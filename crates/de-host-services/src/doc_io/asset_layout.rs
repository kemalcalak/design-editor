//! The `.de` folder layout on disk: images in `<stem>.assets/`, the JSON
//! naming them by relative path (see `de_pen_loader::asset_folder`).

use super::atomic_file::create_sibling_temp;
use super::{commit_staged_document, DocIoError};

/// Write a serialized single-file document at `path` in the folder layout
/// (`de_pen_loader::asset_folder::write_with` owns the crash-safe order),
/// publishing the JSON through this crate's atomic writer.
pub(super) fn write_document_with_asset_folder(
    source: Vec<u8>,
    path: &std::path::Path,
) -> Result<(), DocIoError> {
    de_pen_loader::asset_folder::write_with(path, source, |bytes| write_document_bytes(bytes, path))
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
