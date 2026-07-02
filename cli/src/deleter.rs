//! Parallel directory deletion.
//!
//! `std::fs::remove_dir_all` unlinks a tree one entry at a time on a single
//! thread. For a `node_modules` — routinely tens of thousands of tiny files —
//! that serial syscall storm is the real cost, not the number of folders. This
//! module keeps a small shared pool of worker threads busy unlinking files
//! concurrently, which fills the disk's I/O queue and cuts wall-clock time on
//! SSDs noticeably.
//!
//! The design mirrors [`crate::scanner`]: a single job queue behind a mutex
//! feeds a fixed pool, so total threads stay bounded no matter how many folders
//! the user marked. Folders are orchestrated one at a time — the expensive
//! part (unlinking files) is parallel, while the cheap part (removing the now
//! empty directories, bottom-up) is serial. Each finished folder reports a
//! `Deleted` or `DeleteError` message exactly as the old per-folder threads did,
//! so the UI is unchanged.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::scanner::Msg;

/// Delete every path in `paths` on a background thread, streaming a
/// `Msg::Deleted` / `Msg::DeleteError` back per folder as it completes.
pub fn spawn_delete(paths: Vec<PathBuf>, tx: Sender<Msg>) {
    thread::spawn(move || run(paths, tx));
}

fn run(paths: Vec<PathBuf>, tx: Sender<Msg>) {
    // I/O-bound work: a modest pool keeps the disk queue full without thrashing.
    let workers = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8);

    // Files to unlink; a single receiver shared behind a mutex so any idle
    // worker grabs the next one. Each worker acks its outcome so the
    // orchestrator can tell when a folder is fully drained.
    let (job_tx, job_rx) = mpsc::channel::<PathBuf>();
    let job_rx = Arc::new(Mutex::new(job_rx));
    let (ack_tx, ack_rx) = mpsc::channel::<io::Result<()>>();

    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let job_rx = Arc::clone(&job_rx);
        let ack_tx = ack_tx.clone();
        handles.push(thread::spawn(move || {
            loop {
                let file = {
                    let Ok(guard) = job_rx.lock() else { return };
                    match guard.recv() {
                        Ok(file) => file,
                        Err(_) => return, // Queue closed and drained.
                    }
                };
                // `remove_file` unlinks a plain file or a symlink without
                // following it, matching `remove_dir_all`'s treatment of links.
                if ack_tx.send(fs::remove_file(&file)).is_err() {
                    return; // Orchestrator gone.
                }
            }
        }));
    }
    // The orchestrator owns the sole surviving `ack_tx`; drop this extra clone
    // so the receiver isn't kept alive past its usefulness.
    drop(ack_tx);

    for folder in paths {
        let outcome = delete_one(&folder, &job_tx, &ack_rx);
        let msg = match outcome {
            Ok(()) => Msg::Deleted { path: folder },
            Err(error) => Msg::DeleteError {
                path: folder,
                error,
            },
        };
        if tx.send(msg).is_err() {
            break; // UI gone, stop working.
        }
    }

    // Closing the queue lets workers exit; joining is best-effort tidiness.
    drop(job_tx);
    for handle in handles {
        let _ = handle.join();
    }
}

/// Delete a single folder: stream its files to the unlink pool, wait for them
/// all, then remove the emptied directories bottom-up. Returns the first error
/// encountered (best-effort: it keeps going so as much is reclaimed as possible).
fn delete_one(
    folder: &Path,
    job_tx: &Sender<PathBuf>,
    ack_rx: &mpsc::Receiver<io::Result<()>>,
) -> Result<(), String> {
    let mut first_err: Option<String> = None;
    // Directories recorded in visit order (parent before child). A DFS stack
    // guarantees a parent is popped before any of its children, so reversing
    // this list yields a safe bottom-up removal order.
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    let mut submitted = 0usize;

    while let Some(dir) = stack.pop() {
        dirs.push(dir.clone());
        let read_dir = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                first_err.get_or_insert_with(|| e.to_string());
                continue;
            }
        };
        for entry in read_dir.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            // `file_type` from `read_dir` does not follow symlinks, so a symlink
            // to a directory reports as a symlink (not a dir) and is unlinked as
            // a file below — we never descend through it.
            if file_type.is_dir() {
                stack.push(path);
            } else if job_tx.send(path).is_ok() {
                submitted += 1;
            }
        }
    }

    // Wait for every queued unlink to finish before touching directories: a
    // directory can only be removed once its files are gone. Only this folder's
    // files are in flight, so exactly `submitted` acks belong to us.
    for _ in 0..submitted {
        match ack_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                first_err.get_or_insert_with(|| e.to_string());
            }
            Err(_) => break, // All workers vanished; nothing more will arrive.
        }
    }

    // Remove directories deepest-first. Each is empty by now unless a file under
    // it failed to unlink, in which case the error is already recorded.
    for dir in dirs.iter().rev() {
        if let Err(e) = fs::remove_dir(dir) {
            first_err.get_or_insert_with(|| e.to_string());
        }
    }

    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn deletes_a_nested_tree() {
        let base = std::env::temp_dir().join(format!("nukenpm-del-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let nm = base.join("node_modules");
        fs::create_dir_all(nm.join("pkg/sub")).unwrap();
        fs::write(nm.join("a.js"), b"12345").unwrap();
        fs::write(nm.join("pkg/b.js"), b"67").unwrap();
        fs::write(nm.join("pkg/sub/c.js"), b"890").unwrap();

        let (tx, rx) = mpsc::channel();
        run(vec![nm.clone()], tx);

        let msg = rx.recv().unwrap();
        assert!(matches!(msg, Msg::Deleted { path } if path == nm));
        assert!(!nm.exists(), "tree should be gone");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn deletes_multiple_folders_and_reports_each() {
        let base = std::env::temp_dir().join(format!("nukenpm-delmulti-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let a = base.join("a/node_modules");
        let b = base.join("b/node_modules");
        fs::create_dir_all(a.join("x")).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("x/f.js"), b"data").unwrap();

        let (tx, rx) = mpsc::channel();
        run(vec![a.clone(), b.clone()], tx);

        let mut deleted = Vec::new();
        while let Ok(Msg::Deleted { path }) = rx.recv() {
            deleted.push(path);
            if deleted.len() == 2 {
                break;
            }
        }
        assert_eq!(deleted.len(), 2);
        assert!(!a.exists());
        assert!(!b.exists());

        let _ = fs::remove_dir_all(&base);
    }
}
