//! Home's Recent previews on the desktop. Writing and rendering live in
//! `de_host_services::home_preview` (shared with the MCP transports); this
//! file queues a preview after each UI save and, while Home is up, keeps
//! every Recent card current: a missing or stale preview is generated in
//! the background from the saved file, and a fresh one is installed into
//! the image cache under its stamp (`HomeState::preview_stamps`).
//! (`de_editor_core::home_thumbnail` names the files and cache ids.)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use de_editor_core::home_thumbnail::home_thumbnail_image_id;
use de_editor_core::{EditorState, HOME_RECENT_MAX};
use de_editor_ui::widgets::canvas_viewport_image::{
    has_cached_image_bytes, store_remote_image_bytes,
};
use de_host_services::home_preview::{
    is_stale, modified_time, preview_path, preview_stamp, previews_dir, queue, queue_into,
    written_count,
};

/// How often a visible Home looks again for files changed outside the app
/// (an agent saving over `--mcp`, another tool). An idle Home is woken on
/// this period by `app_handler/scheduling.rs`.
pub(crate) const RESCAN: Duration = Duration::from_secs(2);

/// A document written this recently is left to its writer: the process
/// that saved it (another `--mcp` server, this app) queues its own
/// preview, and Home rendering it too would do the work twice. A writer
/// that exits before its render lands (`--mcp` ending a session) is caught
/// up by the next scan after this.
const SETTLED: Duration = Duration::from_secs(3);

/// Queue the preview of the document a UI save just wrote. It is rendered
/// from the saved file on the preview worker, so a save never waits on it.
pub(crate) fn queue_after_save(doc_path: &Path) {
    crate::test_config_root::guard_user_config();
    queue(doc_path);
}

/// Keeps Home's Recent previews current while Home is visible.
#[derive(Default)]
pub(crate) struct HomePreviews {
    home_was_visible: bool,
    /// When the last scan ran, over which paths, at which worker count; a
    /// scan repeats when any of them moves or [`RESCAN`] passes.
    last_scan: Option<(Instant, Vec<String>, u64)>,
    /// The document version last queued per path, so a file that fails to
    /// render is not queued again until it changes.
    requested: HashMap<String, SystemTime>,
    /// The preview folder, resolved on first use (`Some(None)`: none).
    dir: Option<Option<PathBuf>>,
}

impl HomePreviews {
    /// Queue what is missing and install what landed. Returns whether a
    /// preview was installed (a repaint is due).
    pub(crate) fn poll(&mut self, state: &mut EditorState) -> bool {
        self.poll_at(state, Instant::now(), SystemTime::now())
    }

    fn poll_at(&mut self, state: &mut EditorState, now: Instant, wall: SystemTime) -> bool {
        let visible = state.editor_ui.home.visible;
        if visible && !self.home_was_visible {
            self.last_scan = None;
        }
        self.home_was_visible = visible;
        if !visible {
            return false;
        }
        let recent = state.editor_ui.recent_files.iter().take(HOME_RECENT_MAX);
        let written = written_count();
        if let Some((at, last_paths, last_written)) = &self.last_scan {
            let same_paths = last_paths
                .iter()
                .map(String::as_str)
                .eq(recent.clone().map(|file| file.path.as_str()));
            if same_paths && *last_written == written && now - *at < RESCAN {
                return false;
            }
        }
        let paths: Vec<String> = recent.map(|file| file.path.clone()).collect();
        self.last_scan = Some((now, paths.clone(), written));
        let Some(dir) = self
            .dir
            .get_or_insert_with(|| {
                crate::test_config_root::guard_user_config();
                previews_dir()
            })
            .clone()
        else {
            return false;
        };
        let mut installed = false;
        for path in paths {
            let doc = Path::new(&path);
            let preview = preview_path(&dir, doc);
            if is_stale(doc, &preview) {
                self.request(&dir, &path, wall);
            }
            // A stale preview keeps showing until its replacement lands.
            let Some(stamp) = preview_stamp(&preview) else {
                continue;
            };
            let id = home_thumbnail_image_id(&path, stamp);
            let stamps = &mut state.editor_ui.home.preview_stamps;
            if stamps.get(&path) == Some(&stamp) && has_cached_image_bytes(id) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&preview) else {
                continue;
            };
            store_remote_image_bytes(id, bytes);
            stamps.insert(path, stamp);
            installed = true;
        }
        installed
    }

    /// Queue a render of `path` unless this version was already queued or
    /// its writer is still at work.
    fn request(&mut self, dir: &Path, path: &str, wall: SystemTime) {
        let Some(version) = modified_time(Path::new(path)) else {
            return;
        };
        // A time ahead of the clock (skew, an unpacked archive) counts as
        // settled, or the file would wait until the clock caught up.
        let settled = match wall.duration_since(version) {
            Ok(age) => age >= SETTLED,
            Err(_) => true,
        };
        if !settled || self.requested.get(path) == Some(&version) {
            return;
        }
        self.requested.insert(path.to_string(), version);
        queue_into(dir, Path::new(path));
    }
}

impl crate::DesktopApp {
    /// Run the Home preview scan; on an install, invalidate the editor
    /// state so the next frame paints the new preview.
    pub(crate) fn poll_home_previews(&mut self) -> bool {
        let installed = self.home_previews.poll(self.host.editor_state_mut());
        if installed {
            self.host.mark_editor_state_dirty();
        }
        installed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"{"version":"1.0.0","children":[{"id":"r","type":"rectangle","x":0,"y":0,"width":80,"height":40,"fill":[{"type":"solid","color":"#123456"}]}]}"##;

    fn home_listing(path: &Path) -> EditorState {
        let mut state = EditorState::default();
        state.editor_ui.home.visible = true;
        state
            .editor_ui
            .recent_files
            .push(de_editor_core::RecentFile {
                path: path.to_string_lossy().into_owned(),
                modified_at: 7,
            });
        state
    }

    #[test]
    fn hidden_home_does_nothing() {
        let mut previews = HomePreviews::default();
        let mut state = home_listing(Path::new("/nowhere/never-saved.op"));
        state.editor_ui.home.visible = false;
        assert!(!previews.poll(&mut state));
        assert!(previews.last_scan.is_none());
    }

    #[test]
    fn a_missing_or_stale_preview_is_generated_and_installed() {
        // The preview render takes from the process-wide decode registry
        // the decode tests drive; hold their lock until the worker is idle.
        let _guard = crate::image_decode_host::lock_decode_test_registry();
        let dir = std::env::temp_dir().join(format!("de-home-previews-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let doc = dir.join("agent-made.op");
        std::fs::write(&doc, DOC).expect("doc written outside the app");
        let mut state = home_listing(&doc);
        // An explicit folder: previews stay off in this test binary.
        let mut previews = HomePreviews {
            dir: Some(Some(dir.join("thumbs"))),
            ..HomePreviews::default()
        };
        let start = Instant::now();
        let wall = SystemTime::now() + SETTLED;

        assert!(
            !previews.poll_at(&mut state, start, SystemTime::now()),
            "nothing to show yet"
        );
        assert!(
            previews.requested.is_empty(),
            "a fresh write is left to its writer"
        );
        assert!(!previews.poll_at(&mut state, start + RESCAN, wall));
        assert!(de_host_services::home_preview::wait_idle(
            Duration::from_secs(30)
        ));
        assert!(
            previews.poll_at(&mut state, start + RESCAN, wall),
            "the landed preview is installed at once"
        );
        let path = doc.to_string_lossy().into_owned();
        let stamp = state.editor_ui.home.preview_stamps[&path];
        assert!(has_cached_image_bytes(home_thumbnail_image_id(
            &path, stamp
        )));
        assert!(
            !previews.poll_at(&mut state, start + RESCAN, wall),
            "nothing new"
        );

        // The document changes outside the app: a later scan refreshes it.
        let later = SystemTime::now() + Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&doc)
            .and_then(|file| file.set_modified(later))
            .expect("touch doc");
        let wall = later + SETTLED;
        previews.poll_at(&mut state, start + RESCAN * 2, wall);
        assert!(de_host_services::home_preview::wait_idle(
            Duration::from_secs(30)
        ));
        assert!(previews.poll_at(&mut state, start + RESCAN * 2, wall));
        assert_ne!(state.editor_ui.home.preview_stamps[&path], stamp);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_dated_ahead_of_the_clock_is_still_rendered() {
        let _guard = crate::image_decode_host::lock_decode_test_registry();
        let dir = std::env::temp_dir().join(format!("de-home-future-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let doc = dir.join("from-archive.op");
        std::fs::write(&doc, DOC).expect("doc");
        let ahead = SystemTime::now() + Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&doc)
            .and_then(|file| file.set_modified(ahead))
            .expect("date it ahead");
        let mut state = home_listing(&doc);
        let mut previews = HomePreviews {
            dir: Some(Some(dir.join("thumbs"))),
            ..HomePreviews::default()
        };
        previews.poll_at(&mut state, Instant::now(), SystemTime::now());
        assert!(
            previews
                .requested
                .contains_key(doc.to_string_lossy().as_ref()),
            "skewed time does not hold the render back"
        );
        assert!(de_host_services::home_preview::wait_idle(
            Duration::from_secs(30)
        ));
        let _ = std::fs::remove_dir_all(dir);
    }
}
