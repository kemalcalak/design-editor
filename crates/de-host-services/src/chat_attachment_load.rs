//! Reading a file the user picked or dropped onto the chat into a staged
//! `ChatAttachment` (#398). Shared by the attach picker and drag-and-drop
//! so both apply the same size cap and type rules.

use std::fs;
use std::path::Path;

use de_editor_core::chat::{ChatAttachment, MAX_ATTACHMENT_BYTES};

pub use crate::chat_attachment_load_error::AttachmentLoadError;

/// Extensions the chat accepts: raster images (sent to vision models) and
/// text formats (quoted into the message).
pub const ATTACHABLE_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];
pub const ATTACHABLE_TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "json", "csv", "tsv", "html", "htm", "css", "js", "ts", "tsx", "jsx",
    "svg", "xml", "yaml", "yml", "toml",
];

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
}

/// Whether `path` names a file the chat can take as an attachment.
pub fn is_attachable(path: &Path) -> bool {
    extension(path).is_some_and(|ext| {
        ATTACHABLE_IMAGE_EXTENSIONS.contains(&ext.as_str())
            || ATTACHABLE_TEXT_EXTENSIONS.contains(&ext.as_str())
    })
}

/// The MIME type a staged attachment carries: images by extension, text
/// formats as `text/<subtype>` (SVG keeps `image/svg+xml`; preparation
/// quotes it as text).
pub fn attachment_media_type(path: &Path) -> String {
    let ext = extension(path).unwrap_or_default();
    let media = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "jsx" => "text/javascript",
        "ts" | "tsx" => "text/typescript",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        _ => "text/plain",
    };
    media.to_string()
}

/// Read `path` into an attachment, refusing unsupported types and files
/// over [`MAX_ATTACHMENT_BYTES`] before reading them whole.
pub fn load_attachment(path: &Path) -> Result<ChatAttachment, AttachmentLoadError> {
    let display = path.display().to_string();
    if !is_attachable(path) {
        return Err(AttachmentLoadError::Unsupported { path: display });
    }
    let io = |source| AttachmentLoadError::Io {
        path: path.display().to_string(),
        source,
    };
    let len = fs::metadata(path).map_err(io)?.len();
    if len > MAX_ATTACHMENT_BYTES as u64 {
        return Err(AttachmentLoadError::TooLarge {
            path: display,
            bytes: len,
        });
    }
    let data = fs::read(path).map_err(io)?;
    if data.len() > MAX_ATTACHMENT_BYTES {
        return Err(AttachmentLoadError::TooLarge {
            path: display,
            bytes: data.len() as u64,
        });
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("attachment")
        .to_string();
    Ok(ChatAttachment {
        name,
        media_type: attachment_media_type(path),
        data,
    })
}
