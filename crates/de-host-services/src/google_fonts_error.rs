//! Typed failures for the on-demand Google Fonts download
//! (`google_fonts.rs`).
//!
//! Style follows `de_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. The split
//! that matters to callers is [`GoogleFontsError::NotOnGoogle`] (the family
//! does not exist there, retrying is pointless) versus every other variant
//! (transport or our own guards; offline is the usual cause).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoogleFontsError {
    /// The family name holds characters no Google Fonts family uses, so it
    /// is refused before any request is built.
    InvalidFamily { family: String },
    /// Google Fonts answered that it has no such family.
    NotOnGoogle { family: String },
    /// The CSS answer listed no downloadable face.
    NoFaces { family: String },
    /// The HTTP client could not be built. `reqwest`'s message, verbatim.
    HttpClient(String),
    /// The request never produced a response (DNS, TLS, timeout, offline).
    Request { url: String, detail: String },
    /// The server answered, but not with success.
    HttpStatus { url: String, status: u16 },
    /// The response body exceeded the byte cap.
    TooLarge { url: String },
}

impl GoogleFontsError {
    /// Whether the family is simply not a Google font, as opposed to a
    /// failure that may succeed later (e.g. once back online).
    pub fn is_not_on_google(&self) -> bool {
        matches!(
            self,
            Self::NotOnGoogle { .. } | Self::InvalidFamily { .. } | Self::NoFaces { .. }
        )
    }
}

impl fmt::Display for GoogleFontsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFamily { family } => {
                write!(f, "\"{family}\" is not a valid Google Fonts family name")
            }
            Self::NotOnGoogle { family } => write!(f, "Google Fonts has no family \"{family}\""),
            Self::NoFaces { family } => {
                write!(f, "Google Fonts listed no font files for \"{family}\"")
            }
            Self::HttpClient(detail) => write!(f, "could not build the HTTP client: {detail}"),
            Self::Request { url, detail } => write!(f, "request to {url} failed: {detail}"),
            Self::HttpStatus { url, status } => write!(f, "{url} answered HTTP {status}"),
            Self::TooLarge { url } => write!(f, "{url} is larger than the font size cap"),
        }
    }
}

impl std::error::Error for GoogleFontsError {}
