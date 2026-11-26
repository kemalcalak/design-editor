//! Typed failure for Copy as PNG / SVG (`clipboard_export`).

use crate::export::ExportError;
use de_editor_ui::svg_export::SvgExportError;

/// Why the selection could not be copied as an image or as SVG.
#[derive(Debug)]
pub enum ClipboardExportError {
    /// No real node is selected.
    NothingSelected,
    /// The document has no active page to render from.
    NoActivePage,
    /// The PNG render failed.
    Raster(ExportError),
    /// The SVG serializer failed.
    Svg(SvgExportError),
}

impl std::fmt::Display for ClipboardExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardExportError::NothingSelected => f.write_str("nothing selected"),
            ClipboardExportError::NoActivePage => f.write_str("no active page"),
            ClipboardExportError::Raster(e) => write!(f, "{e}"),
            ClipboardExportError::Svg(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClipboardExportError {}

impl From<ExportError> for ClipboardExportError {
    fn from(e: ExportError) -> Self {
        ClipboardExportError::Raster(e)
    }
}

impl From<SvgExportError> for ClipboardExportError {
    fn from(e: SvgExportError) -> Self {
        ClipboardExportError::Svg(e)
    }
}
