//! Application state and the logic that mutates it in response to events.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::SystemTime;

use crate::scanner::Msg;

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
    pub modified: Option<SystemTime>,
    pub status: EntryStatus,
    pub error: Option<String>,
}

/// How the result list is ordered.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Size,
    Path,
    Modified,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Size => "size",
            SortMode::Path => "path",
            SortMode::Modified => "age",
        }
    }

    fn next(self) -> SortMode {
        match self {
            SortMode::Size => SortMode::Path,
            SortMode::Path => SortMode::Modified,
            SortMode::Modified => SortMode::Size,
        }
    }
}

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct App {
    pub root: PathBuf,
    pub target: String,
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub scanning: bool,
    pub current_path: Option<PathBuf>,
    pub dirs_scanned: u64,
    pub freed: u64,
    pub sort: SortMode,
    spinner_frame: usize,
    tx: Sender<Msg>,
}

impl App {
    pub fn new(root: PathBuf, target: String, tx: Sender<Msg>) -> Self {
        Self {
            root,
            target,
            entries: Vec::new(),
            selected: 0,
            scanning: true,
            current_path: None,
            dirs_scanned: 0,
            freed: 0,
            sort: SortMode::Size,
            spinner_frame: 0,
            tx,
        }
    }

    /// Handle one message coming from a worker thread.
    pub fn handle_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Scanning { path, count } => {
                self.current_path = Some(path);
                self.dirs_scanned = count;
            }
            Msg::Found {
                path,
                size,
                modified,
            } => {
                self.entries.push(Entry {
                    path,
                    size,
                    modified,
                    status: EntryStatus::Found,
                    error: None,
                });
                self.resort();
            }
            Msg::Done => {
                self.scanning = false;
                self.current_path = None;
            }
            Msg::Deleted { path } => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
                    entry.status = EntryStatus::Deleted;
                    self.freed += entry.size;
                }
            }
            Msg::DeleteError { path, error } => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
                    entry.status = EntryStatus::Error;
                    entry.error = Some(error);
                }
            }
        }
    }

    /// Re-sort entries while keeping the same row selected.
    fn resort(&mut self) {
        let selected_path = self.entries.get(self.selected).map(|e| e.path.clone());
        match self.sort {
            SortMode::Size => self.entries.sort_by(|a, b| b.size.cmp(&a.size)),
            SortMode::Path => self.entries.sort_by(|a, b| a.path.cmp(&b.path)),
            SortMode::Modified => self.entries.sort_by(|a, b| b.modified.cmp(&a.modified)),
        }
        if let Some(path) = selected_path
            && let Some(idx) = self.entries.iter().position(|e| e.path == path)
        {
            self.selected = idx;
        }
    }

    pub fn toggle_sort(&mut self) {
        self.sort = self.sort.next();
        self.resort();
    }

    pub fn next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1).min(self.entries.len() - 1);
        }
    }

    pub fn previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Delete the currently selected directory on a background thread.
    pub fn delete_selected(&mut self) {
        let Some(entry) = self.entries.get_mut(self.selected) else {
            return;
        };
        if entry.status != EntryStatus::Found {
            return; // already deleting/deleted/errored.
        }
        entry.status = EntryStatus::Deleting;
        let path = entry.path.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let msg = match fs::remove_dir_all(&path) {
                Ok(()) => Msg::Deleted { path },
                Err(e) => Msg::DeleteError {
                    path,
                    error: e.to_string(),
                },
            };
            let _ = tx.send(msg);
        });
    }

    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
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
