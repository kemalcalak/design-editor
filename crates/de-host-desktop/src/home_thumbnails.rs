//! Home's Recent thumbnails: a small PNG of a document's active page,
//! written after each save and installed into the image cache while Home
//! is up, so a Recent card shows the design instead of a file glyph.
//!
//! Files live in `~/.design-editor/thumbs/`, one per document path (a new
//! save overwrites it). The cache id folds in the save time, so a re-saved
//! document is read again rather than served from the previous pixels.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};

use de_editor_core::home_thumbnail::{home_thumbnail_file_name, home_thumbnail_image_id};
use de_editor_core::{EditorState, HOME_RECENT_MAX};
use de_editor_ui::layout_scene::ScenePage;
use de_editor_ui::widgets::canvas_viewport_image::{
    has_cached_image_bytes, store_remote_image_bytes,
};
use de_host_native::WidgetHostNative;
use de_host_services::export::{page_bounds, paint_nodes, render_raster_bytes, RasterFormat};

/// Longest edge of a thumbnail in pixels: about twice a 240 px card, so
/// a long landing page stays a small file instead of a full-size render.
const THUMBNAIL_MAX_EDGE: f32 = 560.0;

/// Bumped each time the worker writes a file, so a Home that is already up
/// looks again for a thumbnail it missed a moment earlier.
static WRITTEN: AtomicU64 = AtomicU64::new(0);

fn thumbs_dir() -> Option<PathBuf> {
    // Test binaries write under a scratch root, never the real one.
    crate::test_config_root::guard_user_config();
    de_config_store::design_editor_dir()
        .ok()
        .map(|dir| dir.join("thumbs"))
}

/// When to write: after a save always, after an open only if the file has
/// no thumbnail yet (one saved by an older build).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThumbnailWrite {
    Always,
    IfMissing,
}

/// Queue the thumbnail for the document at `doc_path`. The active page is
/// taken from the host's cached layout scene; rasterising and encoding
/// run on one background worker, so a save never waits on them.
/// Best-effort: a failure only means the card keeps its glyph.
pub(crate) fn write_after_save(host: &mut WidgetHostNative, doc_path: &Path, when: ThumbnailWrite) {
    let Some(dir) = thumbs_dir() else {
        return;
    };
    let target = dir.join(home_thumbnail_file_name(&doc_path.to_string_lossy()));
    if when == ThumbnailWrite::IfMissing && target.exists() {
        return;
    }
    let Some(page) = host.layout_scene().active_page().cloned() else {
        return;
    };
    if let Err(error) = worker().lock().map(|tx| tx.send((target, page))) {
        eprintln!("[home-thumbnail] worker: {error}");
    }
}

/// The single worker. Saves arriving while it renders are coalesced per
/// file, so a burst of Cmd+S renders each document once.
fn worker() -> &'static Mutex<Sender<(PathBuf, ScenePage)>> {
    static WORKER: OnceLock<Mutex<Sender<(PathBuf, ScenePage)>>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(PathBuf, ScenePage)>();
        let spawned = std::thread::Builder::new()
            .name("home-thumbnail".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    let mut jobs: HashMap<PathBuf, ScenePage> = HashMap::from([first]);
                    jobs.extend(rx.try_iter());
                    for (target, page) in jobs {
                        match render(&page, &target) {
                            Ok(()) => {
                                WRITTEN.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(error) => {
                                eprintln!("[home-thumbnail] {}: {error}", target.display());
                            }
                        }
                    }
                }
            });
        if let Err(error) = spawned {
            eprintln!("[home-thumbnail] worker: {error}");
        }
        Mutex::new(tx)
    })
}

fn render(page: &ScenePage, target: &Path) -> Result<(), String> {
    let Some(bounds) = page_bounds(page) else {
        // An emptied page must not keep showing its old picture.
        let _ = std::fs::remove_file(target);
        return Err("empty page".into());
    };
    let long_edge = bounds.size.x.max(bounds.size.y).max(1.0);
    let scale = (THUMBNAIL_MAX_EDGE / long_edge).min(1.0);
    let bytes = render_raster_bytes(bounds, RasterFormat::Png, scale, 0.0, |canvas| {
        paint_nodes(canvas, &page.children);
    })
    .map_err(|error| error.to_string())?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    // Write aside and rename, so Home never reads a half-written file.
    let partial = target.with_extension("png.partial");
    std::fs::write(&partial, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&partial, target).map_err(|error| error.to_string())
}

/// Loads the saved thumbnails of Home's Recent files into the image cache.
#[derive(Default)]
pub(crate) struct HomeThumbnails {
    /// Ids already looked up; a miss is retried when Home shows again or
    /// the worker writes a new file.
    tried: HashSet<u64>,
    home_was_visible: bool,
    /// The Recent ids and worker count of the last scan; an unchanged pair
    /// means there is nothing new to look for.
    last_scan: Option<(Vec<u64>, u64)>,
    dir: Option<Option<PathBuf>>,
}

impl HomeThumbnails {
    /// Install any thumbnail Home's Recent list can show and the cache
    /// lacks. Returns whether one was installed (a repaint is due).
    pub(crate) fn poll(&mut self, state: &EditorState) -> bool {
        let visible = state.editor_ui.home.visible;
        if visible && !self.home_was_visible {
            self.tried.clear();
            self.last_scan = None;
        }
        self.home_was_visible = visible;
        if !visible {
            return false;
        }
        let recent: Vec<(u64, &str)> = state
            .editor_ui
            .recent_files
            .iter()
            .take(HOME_RECENT_MAX)
            .map(|file| {
                (
                    home_thumbnail_image_id(&file.path, file.modified_at),
                    file.path.as_str(),
                )
            })
            .collect();
        let scan = (
            recent.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            WRITTEN.load(Ordering::Relaxed),
        );
        if self.last_scan.as_ref() == Some(&scan) {
            return false;
        }
        if self.last_scan.as_ref().is_some_and(|last| last.1 != scan.1) {
            self.tried.clear();
        }
        self.last_scan = Some(scan);
        let Some(dir) = self.dir.get_or_insert_with(thumbs_dir).clone() else {
            return false;
        };
        let mut installed = false;
        for (id, path) in recent {
            if has_cached_image_bytes(id) || !self.tried.insert(id) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(dir.join(home_thumbnail_file_name(path))) {
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
        thumbs.tried.clear();
        thumbs.poll(&state);
        assert!(
            thumbs.tried.is_empty(),
            "an unchanged list is not rescanned"
        );

        state.editor_ui.home.visible = false;
        thumbs.poll(&state);
        state.editor_ui.home.visible = true;
        thumbs.poll(&state);
        assert_eq!(thumbs.tried.len(), 1, "a reshow retried it once");
    }

    #[test]
    fn a_thumbnail_is_capped_to_the_card_scale() {
        // Painting touches the process-wide image registry the decode
        // tests drive.
        let _guard = crate::image_decode_host::lock_decode_test_registry();
        let dir = std::env::temp_dir().join(format!("de-thumb-{}", std::process::id()));
        let target = dir.join("long.png");
        let mut node = de_editor_ui::layout_scene::SceneNode::leaf(
            "r",
            de_editor_ui::layout_scene::NodeKind::Rect,
        );
        node.bounds = de_editor_ui::Rect::xywh(0.0, 0.0, 1440.0, 4000.0);
        node.fill = Some(de_editor_ui::Color::WHITE);
        let page = ScenePage {
            id: "p".into(),
            name: "Page".into(),
            children: vec![node],
        };
        render(&page, &target).expect("renders");
        let bytes = std::fs::read(&target).expect("written");
        // PNG IHDR: width then height, big-endian, from byte 16.
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        assert_eq!(height, THUMBNAIL_MAX_EDGE as u32);
        let _ = std::fs::remove_dir_all(dir);
    }
}
