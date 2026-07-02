//! Application state and the logic that mutates it in response to events.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::SystemTime;

use crate::deleter;
use crate::scanner::{self, Msg};

/// Lifecycle of a single discovered directory.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntryStatus {
    Found,
    Deleting,
    Deleted,
    Error,
}

/// One discovered target directory (e.g. a `node_modules`).
pub struct Entry {
    pub path: PathBuf,
    pub size: u64,
    pub files: u64,
    pub modified: Option<SystemTime>,
    pub status: EntryStatus,
    pub error: Option<String>,
}

/// How the result list is ordered. The cycle mirrors the prototype:
/// size → modified → path.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Size,
    Modified,
    Path,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Size => "size",
            SortMode::Modified => "modified",
            SortMode::Path => "path",
        }
    }

    fn next(self) -> SortMode {
        match self {
            SortMode::Size => SortMode::Modified,
            SortMode::Modified => SortMode::Path,
            SortMode::Path => SortMode::Size,
        }
    }
}

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct App {
    pub root: PathBuf,
    pub target: String,
    pub entries: Vec<Entry>,
    /// Index into the *visible* (non-deleted) list.
    pub cursor: usize,
    pub scanning: bool,
    pub current_path: Option<PathBuf>,
    pub dirs_scanned: u64,
    pub freed: u64,
    pub sort: SortMode,
    /// Paths the user has multi-selected for deletion.
    marks: HashSet<PathBuf>,
    /// When `Some`, a confirmation dialog is open for these paths.
    pub confirm: Option<Vec<PathBuf>>,
    /// Whether the end-of-session summary screen is showing.
    pub summary: bool,
    /// Whether deletions ask for confirmation first.
    confirm_before_delete: bool,
    spinner_frame: usize,
    /// Generation of the current scan; messages stamped with an older value
    /// come from a superseded scan and are dropped.
    scan_generation: u64,
    /// Cancellation flag handed to the running scanner thread.
    scan_cancel: Arc<AtomicBool>,
    tx: Sender<Msg>,
}

impl App {
    pub fn new(root: PathBuf, target: String, tx: Sender<Msg>, confirm_before_delete: bool) -> Self {
        Self {
            root,
            target,
            entries: Vec::new(),
            cursor: 0,
            scanning: true,
            current_path: None,
            dirs_scanned: 0,
            freed: 0,
            sort: SortMode::Size,
            marks: HashSet::new(),
            confirm: None,
            summary: false,
            confirm_before_delete,
            spinner_frame: 0,
            scan_generation: 0,
            scan_cancel: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }

    /// Reset all session state and kick off a fresh scan on a worker thread.
    pub fn start_scan(&mut self) {
        self.entries.clear();
        self.marks.clear();
        self.cursor = 0;
        self.scanning = true;
        self.current_path = None;
        self.dirs_scanned = 0;
        self.freed = 0;
        self.confirm = None;
        self.summary = false;

        // A previous scan may still be running (the summary screen is reachable
        // mid-scan): tell it to stop and bump the generation so any of its
        // messages still in flight are discarded.
        self.scan_cancel.store(true, atomic::Ordering::Relaxed);
        self.scan_cancel = Arc::new(AtomicBool::new(false));
        self.scan_generation += 1;

        let tx = self.tx.clone();
        let root = self.root.clone();
        let target = self.target.clone();
        let generation = self.scan_generation;
        let cancel = Arc::clone(&self.scan_cancel);
        thread::spawn(move || scanner::scan(root, target, tx, generation, cancel));
    }

    /// Handle one message coming from a worker thread.
    pub fn handle_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Scanning {
                generation,
                path,
                count,
            } => {
                if generation != self.scan_generation {
                    return;
                }
                self.current_path = Some(path);
                self.dirs_scanned = count;
            }
            Msg::Found {
                generation,
                path,
                size,
                files,
                modified,
            } => {
                if generation != self.scan_generation {
                    return;
                }
                self.entries.push(Entry {
                    path,
                    size,
                    files,
                    modified,
                    status: EntryStatus::Found,
                    error: None,
                });
                self.resort();
            }
            Msg::Done { generation } => {
                if generation != self.scan_generation {
                    return;
                }
                self.scanning = false;
                self.current_path = None;
            }
            Msg::Deleted { path } => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
                    entry.status = EntryStatus::Deleted;
                    self.freed += entry.size;
                }
                self.marks.remove(&path);
                self.clamp_cursor();
                // Everything reclaimed — jump straight to the summary.
                if !self.scanning && self.visible().is_empty() && self.deleted_count() > 0 {
                    self.summary = true;
                }
            }
            Msg::DeleteError { path, error } => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
                    entry.status = EntryStatus::Error;
                    entry.error = Some(error);
                }
                self.marks.remove(&path);
            }
        }
    }

    /// Directories still present on disk (everything except already-deleted).
    pub fn visible(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.status != EntryStatus::Deleted)
            .collect()
    }

    fn cursor_path(&self) -> Option<PathBuf> {
        self.visible().get(self.cursor).map(|e| e.path.clone())
    }

    fn clamp_cursor(&mut self) {
        let n = self.visible().len();
        self.cursor = if n == 0 { 0 } else { self.cursor.min(n - 1) };
    }

    fn sort_entries(&mut self) {
        match self.sort {
            SortMode::Size => self.entries.sort_by(|a, b| b.size.cmp(&a.size)),
            SortMode::Path => self.entries.sort_by(|a, b| a.path.cmp(&b.path)),
            // Oldest first, with unknown mtimes sinking to the bottom.
            SortMode::Modified => self.entries.sort_by(|a, b| match (a.modified, b.modified) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }),
        }
    }

    /// Re-sort while keeping the same row under the cursor.
    fn resort(&mut self) {
        let keep = self.cursor_path();
        self.sort_entries();
        if let Some(path) = keep
            && let Some(idx) = self.visible().iter().position(|e| e.path == path)
        {
            self.cursor = idx;
        }
        self.clamp_cursor();
    }

    pub fn toggle_sort(&mut self) {
        self.sort = self.sort.next();
        self.sort_entries();
        self.cursor = 0;
    }

    pub fn next(&mut self) {
        let n = self.visible().len();
        if n > 0 {
            self.cursor = (self.cursor + 1).min(n - 1);
        }
    }

    pub fn previous(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn is_marked(&self, path: &PathBuf) -> bool {
        self.marks.contains(path)
    }

    /// Toggle the mark on the entry under the cursor.
    pub fn toggle_mark(&mut self) {
        let path = match self.visible().get(self.cursor) {
            Some(e) if e.status == EntryStatus::Found => e.path.clone(),
            _ => return,
        };
        if !self.marks.remove(&path) {
            self.marks.insert(path);
        }
    }

    fn found_paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Found)
            .map(|e| e.path.clone())
            .collect()
    }

    /// Mark every deletable row, or clear them all if everything is marked.
    pub fn toggle_all(&mut self) {
        let paths = self.found_paths();
        if self.all_marked() {
            for p in &paths {
                self.marks.remove(p);
            }
        } else {
            for p in paths {
                self.marks.insert(p);
            }
        }
    }

    /// Whether every deletable row is currently marked.
    pub fn all_marked(&self) -> bool {
        let paths = self.found_paths();
        !paths.is_empty() && paths.iter().all(|p| self.marks.contains(p))
    }

    pub fn marked_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Found && self.marks.contains(&e.path))
            .count()
    }

    pub fn marked_size(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Found && self.marks.contains(&e.path))
            .map(|e| e.size)
            .sum()
    }

    /// Begin a deletion: gather the marked rows (or the cursor row) and either
    /// open the confirmation dialog or delete immediately.
    pub fn request_delete(&mut self) {
        let paths: Vec<PathBuf> = if !self.marks.is_empty() {
            self.entries
                .iter()
                .filter(|e| e.status == EntryStatus::Found && self.marks.contains(&e.path))
                .map(|e| e.path.clone())
                .collect()
        } else {
            match self.visible().get(self.cursor) {
                Some(e) if e.status == EntryStatus::Found => vec![e.path.clone()],
                _ => Vec::new(),
            }
        };
        if paths.is_empty() {
            return;
        }
        if self.confirm_before_delete {
            self.confirm = Some(paths);
        } else {
            self.perform_delete(paths);
        }
    }

    /// Entries referenced by the open confirmation dialog.
    pub fn confirm_entries(&self) -> Vec<&Entry> {
        match &self.confirm {
            Some(paths) => paths
                .iter()
                .filter_map(|p| self.entries.iter().find(|e| &e.path == p))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn confirm_delete(&mut self) {
        if let Some(paths) = self.confirm.take() {
            self.perform_delete(paths);
        }
    }

    pub fn cancel_confirm(&mut self) {
        self.confirm = None;
    }

    /// Mark the paths as in-flight and hand the whole batch to the shared
    /// deletion pool, which unlinks files across folders concurrently.
    fn perform_delete(&mut self, paths: Vec<PathBuf>) {
        let mut targets = Vec::with_capacity(paths.len());
        for path in &paths {
            self.marks.remove(path);
            let Some(entry) = self
                .entries
                .iter_mut()
                .find(|e| &e.path == path && e.status == EntryStatus::Found)
            else {
                continue;
            };
            entry.status = EntryStatus::Deleting;
            targets.push(entry.path.clone());
        }
        if !targets.is_empty() {
            deleter::spawn_delete(targets, self.tx.clone());
        }
        self.confirm = None;
        self.clamp_cursor();
    }

    pub fn show_summary(&mut self) {
        self.summary = true;
    }

    pub fn deleted_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == EntryStatus::Deleted)
            .count()
    }

    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    /// Raw animation frame counter, used to drive the scanning progress sweep.
    pub fn anim(&self) -> usize {
        self.spinner_frame
    }

    pub fn spinner(&self) -> &'static str {
        SPINNER[(self.spinner_frame / 2) % SPINNER.len()]
    }

    /// Total size of directories still present (not yet deleted).
    pub fn reclaimable(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.status != EntryStatus::Deleted)
            .map(|e| e.size)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn stale_scan_messages_are_ignored_after_restart() {
        let (tx, _rx) = mpsc::channel::<Msg>();
        let root = std::env::temp_dir();
        let mut app = App::new(root, "node_modules".into(), tx, true);

        // Restarting bumps the generation; anything stamped with the old one
        // (a scanner that was still running) must not leak into the session.
        app.start_scan();
        app.handle_msg(Msg::Found {
            generation: 0,
            path: PathBuf::from("/stale/node_modules"),
            size: 1,
            files: 1,
            modified: None,
        });
        assert!(app.entries.is_empty(), "stale Found must be dropped");

        app.handle_msg(Msg::Done { generation: 0 });
        assert!(app.scanning, "stale Done must not end the new scan");
    }
}
