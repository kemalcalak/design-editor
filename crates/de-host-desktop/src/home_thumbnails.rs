//! Home's Recent thumbnails: a small PNG of a document's active page,
//! written after each save and installed into the image cache while Home
//! is up, so a Recent card shows the design instead of a file glyph.
//!
//! Files live in `~/.design-editor/thumbs/`, one per document path (a new
//! save overwrites it). The cache id folds in the save time, so a re-saved
//! document is read again rather than served from the previous pixels.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use de_editor_core::home_thumbnail::{home_thumbnail_file_name, home_thumbnail_image_id};
use de_editor_core::EditorState;
use de_editor_ui::widgets::canvas_viewport_image::{
    has_cached_image_bytes, store_remote_image_bytes,
};
use de_editor_ui::widgets::home_surface::RECENT_MAX;

/// Render scale for a thumbnail: the exporter's floor, which keeps a long
/// landing page to a modest file while staying sharp in a 240 px card.
const THUMBNAIL_SCALE: f32 = 0.5;

fn thumbs_dir() -> Option<PathBuf> {
    // Test binaries write under a scratch root, never the real one.
    crate::test_config_root::guard_user_config();
    de_config_store::design_editor_dir()
        .ok()
        .map(|dir| dir.join("thumbs"))
}

/// Write the thumbnail for the document just saved at `doc_path`. The
/// layout scene is built here (it reads the live state); rasterising and
/// encoding run on a worker so a save never waits on them. Best-effort:
/// a failure only means the card keeps its glyph.
pub(crate) fn write_after_save(state: &EditorState, doc_path: &Path) {
    let Some(dir) = thumbs_dir() else {
        return;
    };
    let target = dir.join(home_thumbnail_file_name(&doc_path.to_string_lossy()));
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
    let spawned = std::thread::Builder::new()
        .name("home-thumbnail".into())
        .spawn(move || {
            if let Err(error) = std::fs::create_dir_all(&dir) {
                eprintln!("[home-thumbnail] {}: {error}", dir.display());
                return;
            }
            if let Err(error) = de_host_services::export::export_raster(
                &scene,
                &target,
                de_host_services::export::RasterFormat::Png,
                THUMBNAIL_SCALE,
            ) {
                eprintln!("[home-thumbnail] {}: {error}", target.display());
            }
        });
    if let Err(error) = spawned {
        eprintln!("[home-thumbnail] worker: {error}");
    }
}

/// Loads the saved thumbnails of Home's Recent files into the image cache.
#[derive(Default)]
pub(crate) struct HomeThumbnails {
    /// Ids already looked up while Home has been up; a miss is retried the
    /// next time Home shows (its file may have been written since).
    tried: HashSet<u64>,
    home_was_visible: bool,
}

impl HomeThumbnails {
    /// Install any thumbnail Home's Recent list can show and the cache
    /// lacks. Returns whether one was installed (a repaint is due).
    pub(crate) fn poll(&mut self, state: &EditorState) -> bool {
        let visible = state.editor_ui.home.visible;
        if visible && !self.home_was_visible {
            self.tried.clear();
        }
        self.home_was_visible = visible;
        if !visible {
            return false;
        }
        let Some(dir) = thumbs_dir() else {
            return false;
        };
        let mut installed = false;
        for file in state.editor_ui.recent_files.iter().take(RECENT_MAX) {
            let id = home_thumbnail_image_id(&file.path, file.modified_at);
            if has_cached_image_bytes(id) || !self.tried.insert(id) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(dir.join(home_thumbnail_file_name(&file.path))) {
                store_remote_image_bytes(id, bytes);
                installed = true;
            }
        }
        installed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_home_loads_nothing_and_a_reshow_retries_misses() {
        let mut thumbs = HomeThumbnails::default();
        let mut state = EditorState::default();
        state
            .editor_ui
            .recent_files
            .push(de_editor_core::RecentFile {
                path: "/nowhere/never-saved.op".into(),
                modified_at: 7,
            });
        assert!(!thumbs.poll(&state), "Home is hidden");
        assert!(thumbs.tried.is_empty());

        state.editor_ui.home.visible = true;
        assert!(!thumbs.poll(&state), "no file was ever written");
        assert_eq!(thumbs.tried.len(), 1, "the miss is remembered");

        state.editor_ui.home.visible = false;
        thumbs.poll(&state);
        state.editor_ui.home.visible = true;
        thumbs.poll(&state);
        assert_eq!(thumbs.tried.len(), 1, "a reshow retried it once");
    }
}
