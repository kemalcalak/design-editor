//! Typed failure for the disk-backed imported-font store (`fonts.rs`).
//!
//! Style follows `de_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency.
//! [`FontStoreError::TooLarge`] carries the raw byte count and re-formats the
//! sentence from it, so the size text is reproduced byte for byte; the rest
//! are labelled wrappers around the message of the step that failed. These
//! strings reach the user through `font_import_host.rs`'s native dialog
//! ("Could not import <path>:\n\n<detail>"), so the wording is contract.
//!
//! What the enum adds is where in the import transaction the failure
//! happened, which is load-bearing here: `FontStore::import` deliberately
//! persists BEFORE registering and prunes superseded files only after the
//! index is durable, so [`FontStoreError::WriteIndex`] means the new face is
//! on disk but unreferenced, while [`FontStoreError::Register`] means it is
//! both on disk and indexed but not live this session. A `String` could not
//! express that difference.
//!
//! The message-carrying variants adapt `std::io` and `jian-skia`, neither of
//! which this pass owns, with `e.to_string()`.

use std::fmt;

use crate::fonts::MAX_FONT_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FontStoreError {
    /// The file is over [`MAX_FONT_BYTES`]. Refused before anything touches
    /// disk or the process-global registry.
    TooLarge { bytes: usize },
    /// `jian-skia` could not parse the bytes as a font, so no family / style /
    /// weight key exists to store the file under.
    NotAFontFile,
    /// The `<config>/fonts/` directory could not be created.
    CreateDir(String),
    /// Copying the font bytes into the store failed.
    WriteFile(String),
    /// Rewriting `index.json` failed. The font file itself is already on
    /// disk but nothing references it.
    WriteIndex(String),
    /// The live registration failed after the on-disk state was made durable.
    /// Practically unreachable — the same bytes already parsed once — but
    /// propagated rather than unwrapped.
    Register(String),
}

impl fmt::Display for FontStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontStoreError::TooLarge { bytes } => write!(
                f,
                "font file is too large ({:.1} MiB; max {} MiB)",
                *bytes as f64 / (1024.0 * 1024.0),
                MAX_FONT_BYTES / (1024 * 1024)
            ),
            FontStoreError::NotAFontFile => f.write_str("not a valid ttf/otf font file"),
            FontStoreError::CreateDir(message) => write!(f, "create fonts dir: {message}"),
            FontStoreError::WriteFile(message) => write!(f, "write font file: {message}"),
            FontStoreError::WriteIndex(message) => write!(f, "write font index: {message}"),
            FontStoreError::Register(message) => write!(f, "register font: {message}"),
        }
    }
}

impl std::error::Error for FontStoreError {}

/// Why a downloaded face's `name` table could not be rewritten
/// (`google_font_names.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FontNameError {
    /// A collection, WOFF, or anything that is not one sfnt face.
    NotASingleFace,
    /// A table or record points past the end of the file.
    Truncated,
    /// The face has no table with this tag.
    MissingTable(&'static str),
    /// The rebuilt `name` table would overflow its 16-bit offsets.
    NameTooLarge,
}

impl fmt::Display for FontNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontNameError::NotASingleFace => f.write_str("not a single TrueType/OpenType face"),
            FontNameError::Truncated => f.write_str("font file is truncated"),
            FontNameError::MissingTable(tag) => write!(f, "font has no `{tag}` table"),
            FontNameError::NameTooLarge => f.write_str("font name table is too large"),
        }
    }
}

impl std::error::Error for FontNameError {}

/// Why a Google Fonts family could not be made available
/// (`google_font_download.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FontDownloadError {
    /// The network half failed or Google Fonts has no such family.
    Google(de_host_services::google_fonts::GoogleFontsError),
    /// A downloaded face could not be renamed to the requested family.
    Name(FontNameError),
    /// A downloaded face could not be persisted or registered.
    Store(FontStoreError),
    /// The font store under the config dir could not be opened.
    OpenStore(String),
}

impl FontDownloadError {
    /// Google Fonts has no such family; retrying will not help.
    pub(crate) fn is_not_on_google(&self) -> bool {
        matches!(self, FontDownloadError::Google(error) if error.is_not_on_google())
    }
}

impl fmt::Display for FontDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontDownloadError::Google(error) => error.fmt(f),
            FontDownloadError::Name(error) => write!(f, "downloaded font: {error}"),
            FontDownloadError::Store(error) => error.fmt(f),
            FontDownloadError::OpenStore(message) => write!(f, "open font store: {message}"),
        }
    }
}

impl std::error::Error for FontDownloadError {}

impl From<de_host_services::google_fonts::GoogleFontsError> for FontDownloadError {
    fn from(error: de_host_services::google_fonts::GoogleFontsError) -> Self {
        FontDownloadError::Google(error)
    }
}

impl From<FontNameError> for FontDownloadError {
    fn from(error: FontNameError) -> Self {
        FontDownloadError::Name(error)
    }
}

impl From<FontStoreError> for FontDownloadError {
    fn from(error: FontStoreError) -> Self {
        FontDownloadError::Store(error)
    }
}
