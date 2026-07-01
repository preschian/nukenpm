//! Background directory scanner.
//!
//! The scanner runs on its own thread and streams results back to the UI over a
//! channel, so the interface stays responsive while a large tree is walked.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::SystemTime;

use crate::fs_utils::dir_stats;

/// Messages sent from worker threads (scanner + deleters) to the UI loop.
pub enum Msg {
    /// Progress heartbeat: directory currently being walked and dirs seen so far.
    Scanning { path: PathBuf, count: u64 },
    /// A target directory was found, together with its size, file count and mtime.
    Found {
        path: PathBuf,
        size: u64,
        files: u64,
        modified: Option<SystemTime>,
    },
    /// The scan finished walking the whole tree.
    Done,
    /// A deletion completed successfully.
    Deleted { path: PathBuf },
    /// A deletion failed.
    DeleteError { path: PathBuf, error: String },
}

/// Walk `root` looking for directories named `target` (e.g. `node_modules`).
///
/// When a match is found we record its size but do **not** descend into it,
/// which is both faster and avoids reporting nested `node_modules`.
pub fn scan(root: PathBuf, target: String, tx: Sender<Msg>) {
    let mut stack = vec![root];
    let mut count: u64 = 0;

    while let Some(dir) = stack.pop() {
        count += 1;
        // Throttle progress updates so we don't flood the channel.
        if count.is_multiple_of(24) {
            let _ = tx.send(Msg::Scanning {
                path: dir.clone(),
                count,
            });
        }

        let Ok(read_dir) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in read_dir.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }

            let path = entry.path();
            if entry.file_name() == *target.as_str() {
                let stats = dir_stats(&path);
                let modified = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                if tx
                    .send(Msg::Found {
                        path,
                        size: stats.size,
                        files: stats.files,
                        modified,
                    })
                    .is_err()
                {
                    return; // UI gone, stop working.
                }
                // Do not descend into a matched directory.
            } else {
                stack.push(path);
            }
        }
    }

    let _ = tx.send(Msg::Done);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::mpsc;

    #[test]
    fn finds_targets_without_descending_into_them() {
        // Build a fixture tree under a unique temp dir.
        let base = std::env::temp_dir().join(format!("nukenpm-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        // project-a/node_modules/pkg/index.js  (nested node_modules must be ignored)
        let nm_a = base.join("project-a/node_modules");
        fs::create_dir_all(nm_a.join("pkg/node_modules")).unwrap();
        fs::write(nm_a.join("pkg/index.js"), b"hello").unwrap();
        // project-b/node_modules  (empty-ish)
        let nm_b = base.join("project-b/node_modules");
        fs::create_dir_all(&nm_b).unwrap();

        let (tx, rx) = mpsc::channel();
        scan(base.clone(), "node_modules".to_string(), tx);

        let mut found = Vec::new();
        let mut done = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Found { path, .. } => found.push(path),
                Msg::Done => done = true,
                _ => {}
            }
        }

        assert!(done, "scan should emit Done");
        // Exactly the two top-level node_modules, not the nested one.
        assert_eq!(found.len(), 2, "found: {found:?}");
        assert!(found.iter().any(|p| p.ends_with("project-a/node_modules")));
        assert!(found.iter().any(|p| p.ends_with("project-b/node_modules")));

        let _ = fs::remove_dir_all(&base);
    }
}
