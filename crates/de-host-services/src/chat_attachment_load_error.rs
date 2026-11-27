//! Typed failures for staging a picked or dropped file as a chat
//! attachment (`chat_attachment_load.rs`, #398).

use std::fmt;
use std::io;

use de_editor_core::chat::MAX_ATTACHMENT_BYTES;

#[derive(Debug)]
pub enum AttachmentLoadError {
    /// Not an image or text format the chat takes.
    Unsupported { path: String },
    /// A folder or another non-regular file with an attachable name.
    NotAFile { path: String },
    /// Over the per-file cap (`MAX_ATTACHMENT_BYTES`).
    TooLarge { path: String, bytes: u64 },
    /// Reading the file failed.
    Io { path: String, source: io::Error },
}

impl fmt::Display for AttachmentLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttachmentLoadError::Unsupported { path } => {
                write!(f, "{path} is not an image or text file the chat can attach")
            }
            AttachmentLoadError::NotAFile { path } => write!(f, "{path} is not a file"),
            AttachmentLoadError::TooLarge { path, bytes } => write!(
                f,
                "{path} is {bytes} bytes, over the {MAX_ATTACHMENT_BYTES}-byte attachment limit"
            ),
            AttachmentLoadError::Io { path, source } => write!(f, "{path}: {source}"),
        }
    }
}

impl std::error::Error for AttachmentLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AttachmentLoadError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
