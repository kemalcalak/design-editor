//! Linked component libraries on the desktop (#15).
//!
//! Three jobs: drain the Libraries section's Link / Reload requests (the
//! picker lives here, the reading in `de_pen_loader::library_files`), keep
//! the watcher's path list in step with the open document's links, and
//! re-sync when the watcher reports that a linked file changed on disk.
//!
//! The watcher is one background thread that stats the linked files once a
//! second and wakes the event loop when a file's size or modification time
//! moves. Polling keeps it dependency-free and works the same on every OS;
//! an idle editor with no links never wakes for it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use de_editor_core::{LibraryCommand, LibraryRequest};

use crate::DesktopApp;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// What identifies one version of a file for the watcher.
type Stamp = Option<(SystemTime, u64)>;

/// Watches the open document's library files.
#[derive(Default)]
pub(crate) struct LibraryWatcher {
    paths: Arc<Mutex<Vec<PathBuf>>>,
    started: bool,
}

impl LibraryWatcher {
    /// Start the polling thread; `notify` runs on it when a file changed.
    pub(crate) fn start(&mut self, notify: impl Fn() + Send + 'static) {
        if self.started {
            return;
        }
        self.started = true;
        let paths = Arc::clone(&self.paths);
        let spawned = std::thread::Builder::new()
            .name("library-watch".into())
            .spawn(move || watch_loop(&paths, notify));
        if let Err(error) = spawned {
            eprintln!("design-editor: library watcher did not start: {error}");
        }
    }

    /// Replace the watched files (no-op when unchanged).
    fn set_paths(&self, next: Vec<PathBuf>) {
        if let Ok(mut paths) = self.paths.lock() {
            if *paths != next {
                *paths = next;
            }
        }
    }
}

fn stamp(path: &PathBuf) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

fn watch_loop(paths: &Mutex<Vec<PathBuf>>, notify: impl Fn()) {
    let mut seen: HashMap<PathBuf, Stamp> = HashMap::new();
    loop {
        std::thread::sleep(POLL_INTERVAL);
        let current = match paths.lock() {
            Ok(paths) => paths.clone(),
            Err(_) => return,
        };
        seen.retain(|path, _| current.contains(path));
        let mut changed = false;
        for path in current {
            let now = stamp(&path);
            match seen.insert(path, now) {
                // First sight of a path: the open already read it.
                None => {}
                Some(before) => changed |= before != now,
            }
        }
        if changed {
            notify();
        }
    }
}

impl DesktopApp {
    /// Point the watcher at the open document's linked files.
    pub(crate) fn refresh_library_watch(&mut self) {
        let libraries = &self.host.editor_state().editor_ui.libraries;
        let base = libraries.base_dir.clone().map(PathBuf::from);
        let paths = libraries
            .entries
            .iter()
            .map(|entry| de_pen_loader::resolve_library_path(base.as_deref(), &entry.link.path))
            .collect();
        self.library_watcher.set_paths(paths);
    }

    /// Re-read every linked library (a file changed, or Reload).
    pub(crate) fn resync_libraries(&mut self) -> bool {
        if self
            .host
            .editor_state()
            .editor_ui
            .libraries
            .entries
            .is_empty()
        {
            return false;
        }
        for problem in de_pen_loader::sync_libraries(self.host.editor_state_mut()) {
            eprintln!("design-editor: library not available: {problem}");
        }
        self.host.mark_editor_state_dirty();
        true
    }

    /// Run a Link / Reload request queued by the Libraries section.
    /// Returns whether the canvas needs a repaint.
    pub(crate) fn drain_library_request(&mut self) -> bool {
        let request = self
            .host
            .editor_state_mut()
            .editor_ui
            .libraries
            .request
            .take();
        let repaint = match request {
            Some(LibraryRequest::Link) => self.link_library_file(),
            Some(LibraryRequest::Reload) => self.resync_libraries(),
            None => false,
        };
        self.refresh_library_watch();
        repaint
    }

    fn link_library_file(&mut self) -> bool {
        let locale = self.host.editor_state().editor_ui.locale;
        let picked = rfd::FileDialog::new()
            .set_title(de_i18n::translate(locale, "componentBrowser.linkLibrary"))
            .add_filter(
                de_editor_ui::PRODUCT_NAME,
                &[
                    de_util::document_file::EXTENSION,
                    de_util::document_file::PEN_DEV_EXTENSION,
                ],
            )
            .pick_file();
        let Some(file) = picked else {
            return false;
        };
        let state = self.host.editor_state_mut();
        let base = state
            .editor_ui
            .libraries
            .base_dir
            .clone()
            .map(PathBuf::from);
        let link_path = de_pen_loader::library_link_path(base.as_deref(), &file);
        let command = de_pen_loader::library_command(base.as_deref(), &link_path, None);
        if let LibraryCommand::Unavailable { status, .. } = &command {
            // Not a library we can read: say so, link nothing.
            eprintln!("design-editor: cannot link {}: {status:?}", file.display());
            crate::message_dialog::alert(
                de_i18n::translate(locale, "componentBrowser.linkLibrary"),
                de_i18n::translate(locale, "componentBrowser.libraryInvalid"),
                rfd::MessageLevel::Warning,
            );
            return false;
        }
        state.apply_library_command(command);
        self.host.mark_editor_state_dirty();
        true
    }
}
