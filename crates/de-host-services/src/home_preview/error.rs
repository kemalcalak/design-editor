//! Typed failures for Home's Recent previews (`home_preview.rs`). Every one
//! is best-effort: the card keeps its glyph (or its previous preview), and
//! the document is rendered again once it changes.

use std::fmt;

use crate::doc_io::DocIoError;
use crate::export::ExportError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomePreviewError {
    /// The `.op` file could not be loaded.
    Load(DocIoError),
    /// The page could not be rasterised.
    Render(ExportError),
    /// The preview's temporary file could not be written.
    Write { path: String, detail: String },
    /// The written preview could not be moved into place.
    Install(DocIoError),
}

impl fmt::Display for HomePreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(error) => write!(f, "load: {error}"),
            Self::Render(error) => write!(f, "render: {error}"),
            Self::Write { path, detail } => write!(f, "write {path}: {detail}"),
            Self::Install(error) => write!(f, "install: {error}"),
        }
    }
}

impl std::error::Error for HomePreviewError {}

impl From<DocIoError> for HomePreviewError {
    fn from(error: DocIoError) -> Self {
        Self::Load(error)
    }
}

impl From<ExportError> for HomePreviewError {
    fn from(error: ExportError) -> Self {
        Self::Render(error)
    }
}
