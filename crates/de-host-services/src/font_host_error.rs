//! [`PageFaceError`]: the failure of [`crate::font_host::FontHost::register_page_faces`].

/// Why a page's `@font-face` files could not be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageFaceError {
    /// None of the files decoded to a font with a family name.
    Unreadable { files: usize },
}

impl std::fmt::Display for PageFaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { files } => write!(
                f,
                "none of the page's {files} @font-face file(s) is a font this editor can read"
            ),
        }
    }
}

impl std::error::Error for PageFaceError {}
