//! Blur-up thumbnails across parallel `de agent` tasks.
//!
//! The thumbnail registry (`jian_ops_schema::image_thumbs`) is process-wide
//! and belongs to "the open document": loading a document that carries an
//! `imageThumbs` table replaces it, and a save writes the registry entries
//! the saved document references. With `--parallel`, task B's load would
//! replace the registry task A saves from, and A's document would lose its
//! thumbnails.
//!
//! So each task takes a snapshot of the registry right after its own load
//! (the loader has just installed that document's table, whether the file
//! keeps it inline as base64 or in its asset folder), and puts that snapshot
//! back right before its save. One lock covers load + snapshot and
//! restore + save, so no other task's load can land in between. The agent
//! tools a headless run uses add no thumbnails, so the snapshot taken at
//! load is complete for the document the task saves.

use std::path::Path;
use std::sync::Mutex;

use de_editor_core::{EditorState, Locale};
use de_host_services::doc_io::{self, DocIoError};
use jian_ops_schema::image_thumbs::{self, ImageThumbSnapshot};

/// Held across every load + snapshot and every restore + save.
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

/// The registry as it stood right after one task's load.
#[derive(Clone, Default)]
pub(crate) struct TaskThumbs(ImageThumbSnapshot);

/// Load `path` and snapshot the thumbnails its load installed.
pub(crate) fn load_with_thumbs(path: &Path) -> Result<(EditorState, TaskThumbs), DocIoError> {
    let _guard = lock();
    let state = doc_io::load_editor_state_with_report(path, Locale::EnUs)?.state;
    Ok((state, TaskThumbs(image_thumbs::capture_snapshot())))
}

/// Save `state` to `output` (atomically) with `thumbs` as the registry.
pub(crate) fn save_with_thumbs(
    state: &EditorState,
    output: &Path,
    thumbs: &TaskThumbs,
) -> Result<(), DocIoError> {
    let _guard = lock();
    image_thumbs::restore_snapshot(thumbs.0.clone());
    doc_io::save_to_path(state, output)
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    REGISTRY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
