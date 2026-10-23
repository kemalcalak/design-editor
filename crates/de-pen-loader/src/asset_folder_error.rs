//! Typed failures of the `.de` asset folder writer (`asset_folder.rs`).

use std::fmt;

#[derive(Debug)]
pub enum AssetFolderError {
    /// The serialized document could not be parsed or re-serialized.
    Json(serde_json::Error),
    /// Writing an image file, the document, or its temp file failed.
    Io(std::io::Error),
}

impl fmt::Display for AssetFolderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetFolderError::Json(error) => error.fmt(f),
            AssetFolderError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for AssetFolderError {}

impl From<serde_json::Error> for AssetFolderError {
    fn from(error: serde_json::Error) -> Self {
        AssetFolderError::Json(error)
    }
}

impl From<std::io::Error> for AssetFolderError {
    fn from(error: std::io::Error) -> Self {
        AssetFolderError::Io(error)
    }
}
