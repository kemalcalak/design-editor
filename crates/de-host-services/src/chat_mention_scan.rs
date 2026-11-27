//! The project-folder listing behind the chat's `@` popup (#398).
//!
//! Listing a folder is disk I/O of unknown size, so it never runs on the UI
//! thread: [`ProjectFileScanner`] answers from its cache at once and lists
//! the folder again on a worker thread, handing the result back through
//! [`ProjectFileSource::poll`]. The walk itself is bounded three ways —
//! files, folders read and depth — and a document saved straight into the
//! home folder (or above it) is listed one level deep only, with a warning,
//! instead of walking the whole account.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use de_editor_core::chat_mentions::{ProjectFileList, ProjectFileSource};

/// Files the popup is offered at most.
pub const MAX_PROJECT_FILES: usize = 2_000;
/// Folder levels below the project root the scan descends.
pub const MAX_SCAN_DEPTH: usize = 4;
/// Folders read at most, however shallow they are.
pub const MAX_SCAN_DIRS: usize = 400;
/// A listing younger than this is served without listing again.
const FRESH_FOR: Duration = Duration::from_secs(10);

/// Folders never worth offering: VCS internals, dependency and build
/// output trees.
const SKIPPED_DIRS: &[&str] = &["node_modules", "target", "dist", "build", "__pycache__"];

fn skipped(name: &str, is_dir: bool) -> bool {
    name.starts_with('.') || (is_dir && SKIPPED_DIRS.contains(&name))
}

/// One listing of a project folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectScan {
    /// Project-relative paths with `/` separators, sorted.
    pub files: Vec<String>,
    /// True when the listing stopped short: the folder is the home folder
    /// (or above it), or a file / folder / depth limit cut it.
    pub limited: bool,
}

/// The user's home folder, from the environment.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// List `root` breadth-first within the limits above. `home` is the user's
/// home folder: a root at or above it is listed one level deep only.
/// Hidden entries, symlinks and dependency / build folders are skipped;
/// unreadable folders are passed over silently.
pub fn scan_project_files(root: &Path, home: Option<&Path>) -> ProjectScan {
    let broad = home.is_some_and(|home| home.starts_with(root));
    let max_depth = if broad { 0 } else { MAX_SCAN_DEPTH };
    let mut scan = ProjectScan {
        files: Vec::new(),
        limited: broad,
    };
    let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut next = 0;
    while next < queue.len() {
        if scan.files.len() >= MAX_PROJECT_FILES || next >= MAX_SCAN_DIRS {
            scan.limited = true;
            break;
        }
        let (dir, depth) = queue[next].clone();
        next += 1;
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if skipped(name, kind.is_dir()) || kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if depth < max_depth {
                    queue.push((path, depth + 1));
                } else {
                    scan.limited |= !broad;
                }
            } else if kind.is_file() {
                if scan.files.len() >= MAX_PROJECT_FILES {
                    scan.limited = true;
                    break;
                }
                if let Some(rel) = relative_slash_path(root, &path) {
                    scan.files.push(rel);
                }
            }
        }
    }
    scan.files.sort();
    scan
}

fn relative_slash_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel
        .components()
        .map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    Some(parts?.join("/"))
}

struct Cached {
    root: PathBuf,
    files: Arc<[String]>,
    at: Instant,
}

/// Background, cached [`ProjectFileSource`] for the desktop host.
#[derive(Default)]
pub struct ProjectFileScanner {
    cache: Option<Cached>,
    inflight: Option<(PathBuf, Receiver<ProjectScan>)>,
    /// The roots the "listing was cut" warning was raised for, so it shows
    /// once per folder (also across A → B → A) rather than on every popup.
    warned: HashSet<PathBuf>,
}

impl ProjectFileScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a listing is running on its worker (the host keeps waking
    /// to collect it).
    pub fn is_pending(&self) -> bool {
        self.inflight.is_some()
    }

    fn spawn(&mut self, root: &Path) {
        let (tx, rx) = mpsc::channel();
        let dir = root.to_path_buf();
        let spawned = thread::Builder::new()
            .name("de-mention-scan".into())
            .spawn(move || {
                let home = home_dir();
                // The receiver may be gone (another folder was opened);
                // the listing is then simply dropped.
                let _ = tx.send(scan_project_files(&dir, home.as_deref()));
            });
        match spawned {
            Ok(_) => self.inflight = Some((root.to_path_buf(), rx)),
            Err(err) => eprintln!("design-editor: spawn mention scan failed: {err}"),
        }
    }
}

impl ProjectFileSource for ProjectFileScanner {
    fn request(&mut self, root: &Path) -> Option<ProjectFileList> {
        let cached = self.cache.as_ref().filter(|c| c.root == root);
        let fresh = cached.is_some_and(|c| c.at.elapsed() < FRESH_FOR);
        let answer = cached.map(|c| ProjectFileList {
            root: c.root.clone(),
            files: Arc::clone(&c.files),
            warn: false,
        });
        let running = self.inflight.as_ref().is_some_and(|(r, _)| r == root);
        if !fresh && !running {
            self.spawn(root);
        }
        answer
    }

    fn poll(&mut self) -> Option<ProjectFileList> {
        let (root, rx) = self.inflight.as_ref()?;
        let scan = match rx.try_recv() {
            Ok(scan) => scan,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                self.inflight = None;
                return None;
            }
        };
        let root = root.clone();
        self.inflight = None;
        let warn = scan.limited && self.warned.insert(root.clone());
        let files: Arc<[String]> = Arc::from(scan.files);
        self.cache = Some(Cached {
            root: root.clone(),
            files: Arc::clone(&files),
            at: Instant::now(),
        });
        Some(ProjectFileList { root, files, warn })
    }
}

#[cfg(test)]
#[path = "chat_mention_scan_tests.rs"]
mod tests;
