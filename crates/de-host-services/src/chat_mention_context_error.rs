//! Typed failures for reading a `@`-mentioned project file
//! (`chat_mention_context.rs`, #398). The `Display` text is what the model
//! reads in the context block ("unreadable — …"), so it stays short and
//! names the path the user picked.

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum MentionFileError {
    /// The document has never been saved, so there is no project folder to
    /// read from.
    NoProjectFolder,
    /// The path leaves the project folder (`..`, absolute, or a symlink
    /// pointing outside).
    OutsideProject { path: String },
    /// Opening, sizing or reading the file failed.
    Io { path: String, source: io::Error },
}

impl fmt::Display for MentionFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MentionFileError::NoProjectFolder => {
                write!(f, "the document is not saved, so it has no project folder")
            }
            MentionFileError::OutsideProject { path } => {
                write!(f, "{path} is outside the project folder")
            }
            MentionFileError::Io { path, source } => write!(f, "{path}: {source}"),
        }
    }
}

impl std::error::Error for MentionFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MentionFileError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
