//! Typed failures of a `.pptx` import.
//!
//! Only the package itself can fail an import: a slide whose content the
//! importer cannot express still comes in (as a placeholder, with a
//! warning), so every variant here means "this is not a deck we can read".

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PptxError {
    /// The bytes are not a ZIP archive.
    NotZip(String),
    /// A ZIP archive without `ppt/presentation.xml`.
    NotPresentation,
    /// A part the deck needs could not be read from the archive.
    Part { part: String, detail: String },
    /// A part is not well-formed XML.
    Xml { part: String, detail: String },
    /// The presentation lists no slides.
    NoSlides,
    /// The converted boards do not form a valid document (an importer bug).
    Convert(String),
}

impl fmt::Display for PptxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PptxError::NotZip(detail) => write!(f, "not a PowerPoint file (zip: {detail})"),
            PptxError::NotPresentation => {
                write!(f, "not a PowerPoint presentation (no ppt/presentation.xml)")
            }
            PptxError::Part { part, detail } => write!(f, "cannot read {part}: {detail}"),
            PptxError::Xml { part, detail } => write!(f, "{part} is not valid XML: {detail}"),
            PptxError::NoSlides => write!(f, "the presentation has no slides"),
            PptxError::Convert(detail) => write!(f, "converted slides are invalid: {detail}"),
        }
    }
}

impl std::error::Error for PptxError {}
