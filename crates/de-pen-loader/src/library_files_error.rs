//! Failure type for reading a linked library file (#15).

use de_editor_core::LibraryStatus;

/// Why a library file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryReadError {
    /// No file at the resolved path.
    Missing,
    /// The file exists but reading it failed.
    Unreadable(String),
    /// The file is not a `.de` / pen.dev document.
    Invalid(String),
}

impl std::fmt::Display for LibraryReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => f.write_str("library file not found"),
            Self::Unreadable(error) => write!(f, "read library: {error}"),
            Self::Invalid(error) => write!(f, "load library: {error}"),
        }
    }
}

impl std::error::Error for LibraryReadError {}

impl LibraryReadError {
    /// The panel / MCP status for this failure.
    pub fn status(&self) -> LibraryStatus {
        match self {
            Self::Missing => LibraryStatus::Missing,
            other => LibraryStatus::Invalid(other.to_string()),
        }
    }
}
