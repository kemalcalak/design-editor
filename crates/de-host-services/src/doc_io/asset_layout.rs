//! The `.de` folder layout on disk: images in `<stem>.assets/`, the JSON
//! naming them by relative path (see `de_pen_loader::asset_folder`).

use super::atomic_file::create_sibling_temp;
use super::{commit_staged_document, DocIoError};

/// Write a serialized single-file document at `path` in the folder layout:
/// its images go to `<stem>.assets/` first, then the JSON naming them
/// replaces `path` atomically, then images the document no longer names are
/// pruned. A crash before the rename leaves the previous document and every
/// file it names intact.
pub(super) fn write_document_with_asset_folder(
    source: Vec<u8>,
    path: &std::path::Path,
) -> Result<(), DocIoError> {
    use de_pen_loader::asset_folder;
    let folder = asset_folder::folder_path(path);
    let (bytes, files) = match asset_folder::folder_name(path) {
        Some(name) => asset_folder::externalize_source(source, &name)
            .map_err(|e| DocIoError::Serialize(e.to_string()))?,
        None => (source, Vec::new()),
    };
    if let Some(folder) = folder.as_deref() {
        asset_folder::write_files(folder, &files).map_err(|e| DocIoError::Io(e.to_string()))?;
    }
    write_document_bytes(&bytes, path)?;
    if let Some(folder) = folder.as_deref() {
        // The document is already safe on disk; a failed cleanup only
        // leaves unused images behind.
        if let Err(error) = asset_folder::prune(folder, &files) {
            eprintln!("[save] asset folder cleanup failed: {error}");
        }
    }
    Ok(())
}

/// Write `bytes` through a sibling temp file so a crash mid-write doesn't
/// leave a half-written document on disk.
pub(super) fn write_document_bytes(bytes: &[u8], path: &std::path::Path) -> Result<(), DocIoError> {
    let (tmp, file) = create_sibling_temp(path)?;
    let write_result = (|| {
        let mut writer = std::io::BufWriter::with_capacity(1024 * 1024, file);
        std::io::Write::write_all(&mut writer, bytes).map_err(|e| DocIoError::Io(e.to_string()))?;
        std::io::Write::flush(&mut writer).map_err(|e| DocIoError::Io(e.to_string()))?;
        drop(writer);
        commit_staged_document(&tmp, path)
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    write_result
}

#[cfg(test)]
#[path = "asset_layout_tests.rs"]
mod tests;
