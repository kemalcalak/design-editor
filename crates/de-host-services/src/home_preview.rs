//! Home's Recent previews: a small PNG overview of a document's active page
//! (every top-level node, at its real position, fitted together like
//! Pencil's dashboard), written whenever any save path writes the `.de` and
//! regenerated on demand when Home lists a file whose preview is missing or
//! older than the file.
//!
//! Files live in `~/.design-editor/thumbs/`, one per document path. A
//! preview carries its document's modified time as its own, so "stale" is
//! simply "the document is newer than its preview", whoever wrote it: the
//! desktop, `--mcp <file>`, the live MCP server or another tool.
//!
//! Shared by the desktop (UI saves, Home's scan) and the MCP transports, so
//! it lives here rather than in a host. Rendering is the CPU raster export
//! pipeline and runs on one background worker (`worker.rs`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::SystemTime;

use de_editor_core::home_thumbnail::home_thumbnail_file_name;
use de_editor_ui::layout_scene::ScenePage;
use de_editor_ui::Rect;

use crate::export::{page_bounds, paint_nodes, render_raster_bytes, RasterFormat};

mod error;
#[cfg(test)]
mod tests;
mod worker;

pub use error::HomePreviewError;
#[doc(hidden)]
pub use worker::wait_idle;
pub use worker::{queue, queue_into, set_written_notifier, written_count};

/// Longest edge of a preview in pixels: about twice a 240 px card, so a
/// long landing page stays a small file instead of a full-size render.
pub const PREVIEW_MAX_EDGE: f32 = 560.0;

/// Whether this process may write into the user's preview folder.
static ENABLED: AtomicBool = AtomicBool::new(cfg!(test));

/// Turn preview writes on for this process. The app's entry points call it
/// (the GUI, `--mcp`, `--mcp-http`); test binaries of other crates that
/// reach a save path never do, so they cannot touch the real folder.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

/// The folder previews live in, or `None` when there is no user root or
/// previews are not [`enable`]d in this process. Resolved once: every MCP
/// write asks for it.
pub fn previews_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    DIR.get_or_init(|| {
        // Unit tests here must never write into the developer's real folder.
        crate::test_config_root::guard_user_config();
        de_config_store::design_editor_dir()
            .ok()
            .map(|dir| dir.join("thumbs"))
    })
    .clone()
}

/// The preview file for the document at `doc_path` inside `dir`.
pub fn preview_path(dir: &Path, doc_path: &Path) -> PathBuf {
    dir.join(home_thumbnail_file_name(&doc_path.to_string_lossy()))
}

/// A file's modified time, or `None` when it does not exist.
pub fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// A preview's stamp: its modified time in nanoseconds since the epoch.
/// Home keys its image cache on it, so a refreshed preview is read again.
pub fn preview_stamp(preview: &Path) -> Option<u64> {
    let since = modified_time(preview)?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?;
    Some(u64::try_from(since.as_nanos()).unwrap_or(u64::MAX))
}

/// Whether the document needs a new preview: it exists and its preview is
/// missing or carries another modified time (a newer save, or an older copy
/// restored over it).
pub fn is_stale(doc_path: &Path, preview: &Path) -> bool {
    let Some(doc) = modified_time(doc_path) else {
        return false;
    };
    modified_time(preview).is_none_or(|written| written != doc)
}

/// Rasterise a laid-out page: the union of every top-level node, scaled
/// down to [`PREVIEW_MAX_EDGE`]. An empty page gives a blank (transparent)
/// preview, which Home shows as an empty canvas.
fn render_page_png(page: &ScenePage) -> Result<Vec<u8>, HomePreviewError> {
    let Some(bounds) = page_bounds(page) else {
        return blank_png();
    };
    let long_edge = bounds.size.x.max(bounds.size.y).max(1.0);
    let scale = (PREVIEW_MAX_EDGE / long_edge).min(1.0);
    Ok(render_raster_bytes(
        bounds,
        RasterFormat::Png,
        scale,
        0.0,
        |canvas| paint_nodes(canvas, &page.children),
    )?)
}

/// Load the `.de` file and render its saved active page. The file is read
/// into memory, not mapped: other writers (`save_document`, outside tools)
/// rewrite it in place, which would fault a mapping mid-parse.
pub fn render_document_png(doc_path: &Path) -> Result<Vec<u8>, HomePreviewError> {
    let source = std::fs::read_to_string(doc_path).map_err(|error| {
        HomePreviewError::Load(crate::doc_io::DocIoError::Io(error.to_string()))
    })?;
    let state = crate::doc_io::load_editor_state_from_source_beside(
        &source,
        de_editor_core::Locale::EnUs,
        doc_path,
    )?;
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(&state);
    match scene.active_page() {
        Some(page) => render_page_png(page),
        None => blank_png(),
    }
}

fn blank_png() -> Result<Vec<u8>, HomePreviewError> {
    Ok(render_raster_bytes(
        Rect::xywh(0.0, 0.0, 1.0, 1.0),
        RasterFormat::Png,
        1.0,
        0.0,
        |_| {},
    )?)
}

/// Install `bytes` as the preview at `target`, stamped with the document's
/// modified time `doc_modified`. Written to a unique sibling and swapped in
/// with the `.de` save's primitives, so Home never reads a half-written
/// file, two processes never share a temp, and Windows replaces in place.
pub fn write_preview(
    target: &Path,
    bytes: &[u8],
    doc_modified: Option<SystemTime>,
) -> Result<(), HomePreviewError> {
    use crate::doc_io::atomic_file::{create_sibling_temp, replace_file};
    use std::io::Write as _;
    let fail = |detail: std::io::Error| HomePreviewError::Write {
        path: target.display().to_string(),
        detail: detail.to_string(),
    };
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let (temp, mut file) = create_sibling_temp(target).map_err(HomePreviewError::Install)?;
    let written = file.write_all(bytes).and_then(|()| match doc_modified {
        Some(modified) => file.set_modified(modified),
        None => Ok(()),
    });
    drop(file);
    let installed = written
        .map_err(fail)
        .and_then(|()| replace_file(&temp, target).map_err(HomePreviewError::Install));
    if installed.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    installed
}
