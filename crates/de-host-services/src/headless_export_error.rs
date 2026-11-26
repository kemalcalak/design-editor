//! Typed failures of [`crate::headless_export::export_document`].

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadlessExportError {
    /// The export directory could not be created (or resolved).
    CreateDir { path: String, detail: String },
    /// A per-frame format found no top-level frame on the active page.
    NoFrames,
    /// The exporter itself failed.
    Render {
        format: &'static str,
        detail: String,
    },
}

impl fmt::Display for HeadlessExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, detail } => write!(f, "cannot create {path}: {detail}"),
            Self::NoFrames => f.write_str("the active page has no exportable top-level frames"),
            Self::Render { format, detail } => write!(f, "{format} export failed: {detail}"),
        }
    }
}

impl std::error::Error for HeadlessExportError {}
