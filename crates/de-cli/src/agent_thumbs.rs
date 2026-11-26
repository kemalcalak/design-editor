//! Blur-up thumbnails across parallel `de agent` tasks.
//!
//! The thumbnail registry (`jian_ops_schema::image_thumbs`) is process-wide
//! and belongs to "the open document": loading a document that carries an
//! `imageThumbs` table replaces it, and a save writes the registry entries
//! its images reference. With `--parallel`, task B's load would replace the
//! registry task A saves from, and A's document would lose its thumbnails.
//!
//! Entries are keyed by a hash of the image source (`paint_image_id`), so an
//! entry is right for every document that references that key. Each task
//! therefore keeps its input's own table and adds it back to the registry
//! right before its save; loads and saves hold one lock, so no load can
//! replace the registry between the two. The registry only ever grows by
//! entries some task's document carries, and a save keeps only the entries
//! its document references.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use base64::Engine as _;
use de_editor_core::{EditorState, Locale};
use de_host_services::doc_io::{self, DocIoError};
use serde::Deserialize;

/// Held across every load + table read and every restore + save.
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

/// One task's own thumbnail table: `(paint id, JPEG bytes)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TaskThumbs(Vec<(u64, Vec<u8>)>);

#[derive(Deserialize)]
struct ThumbTable {
    #[serde(rename = "imageThumbs", default)]
    thumbs: BTreeMap<String, String>,
}

/// Load `path` and read the thumbnail table it carries.
pub(crate) fn load_with_thumbs(path: &Path) -> Result<(EditorState, TaskThumbs), DocIoError> {
    let _guard = lock();
    let state = doc_io::load_editor_state_with_report(path, Locale::EnUs)?.state;
    Ok((state, read_table(path)))
}

/// Save `state` to `output` (atomically) with `thumbs` in the registry.
pub(crate) fn save_with_thumbs(
    state: &EditorState,
    output: &Path,
    thumbs: &TaskThumbs,
) -> Result<(), DocIoError> {
    let _guard = lock();
    for (id, bytes) in &thumbs.0 {
        // `store_thumb` ignores payloads over its 4 KiB limit itself.
        jian_ops_schema::image_thumbs::store_thumb(*id, bytes.clone());
    }
    doc_io::save_to_path(state, output)
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    REGISTRY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The document's `imageThumbs` table, the way the loader reads it: entries
/// whose key is not a decimal id or whose value is not base64 are dropped.
/// A file that is not JSON (`.pptx`) has none.
fn read_table(path: &Path) -> TaskThumbs {
    let Ok(text) = std::fs::read_to_string(path) else {
        return TaskThumbs::default();
    };
    let Ok(table) = serde_json::from_str::<ThumbTable>(&text) else {
        return TaskThumbs::default();
    };
    TaskThumbs(
        table
            .thumbs
            .into_iter()
            .filter_map(|(id, encoded)| {
                let id = id.parse::<u64>().ok()?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .ok()?;
                Some((id, bytes))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_read_and_bad_entries_dropped() {
        let dir = std::env::temp_dir().join(format!("de-agent-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.de");
        std::fs::write(
            &path,
            r#"{"version":"1.0.0","children":[],"imageThumbs":{"12":"/9j/2Q==","x":"/9j/2Q==","13":"%%%"}}"#,
        )
        .unwrap();
        assert_eq!(
            read_table(&path),
            TaskThumbs(vec![(12, vec![0xff, 0xd8, 0xff, 0xd9])])
        );
        std::fs::write(&path, "PK\u{3}\u{4} not json").unwrap();
        assert_eq!(read_table(&path), TaskThumbs::default());
        let _ = std::fs::remove_dir_all(dir);
    }
}
