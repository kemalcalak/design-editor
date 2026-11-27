//! Reading a file the user picked, dropped or pasted onto the chat into a
//! staged `ChatAttachment` (#398). Shared by the attach picker, drag-and-
//! drop and paste so all three apply the same size cap and type rules.
//!
//! [`ATTACHMENT_TYPES`] is the one table of what the chat takes and the
//! MIME type each extension carries; the picker's filters, the type check
//! and `chat_attachment::media_type_for_path` all read it.

use std::fs;
use std::path::Path;

use de_editor_core::chat::{ChatAttachment, MAX_ATTACHMENT_BYTES};

pub use crate::chat_attachment_load_error::AttachmentLoadError;

/// How the chat uses an attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentClass {
    /// A raster image, sent to vision models.
    Image,
    /// A text format, quoted into the message (SVG included: its MIME type
    /// is an image one, but the model reads it as markup).
    Text,
}

/// Every extension the chat accepts, with its MIME type and class.
pub const ATTACHMENT_TYPES: &[(&str, &str, AttachmentClass)] = &[
    ("png", "image/png", AttachmentClass::Image),
    ("jpg", "image/jpeg", AttachmentClass::Image),
    ("jpeg", "image/jpeg", AttachmentClass::Image),
    ("gif", "image/gif", AttachmentClass::Image),
    ("webp", "image/webp", AttachmentClass::Image),
    ("svg", "image/svg+xml", AttachmentClass::Text),
    ("txt", "text/plain", AttachmentClass::Text),
    ("md", "text/markdown", AttachmentClass::Text),
    ("markdown", "text/markdown", AttachmentClass::Text),
    ("json", "application/json", AttachmentClass::Text),
    ("csv", "text/csv", AttachmentClass::Text),
    ("tsv", "text/tab-separated-values", AttachmentClass::Text),
    ("html", "text/html", AttachmentClass::Text),
    ("htm", "text/html", AttachmentClass::Text),
    ("css", "text/css", AttachmentClass::Text),
    ("js", "text/javascript", AttachmentClass::Text),
    ("jsx", "text/javascript", AttachmentClass::Text),
    ("ts", "text/typescript", AttachmentClass::Text),
    ("tsx", "text/typescript", AttachmentClass::Text),
    ("xml", "application/xml", AttachmentClass::Text),
    ("yaml", "application/yaml", AttachmentClass::Text),
    ("yml", "application/yaml", AttachmentClass::Text),
    ("toml", "application/toml", AttachmentClass::Text),
];

/// The extensions of one class (the picker's filters).
pub fn attachable_extensions(class: AttachmentClass) -> Vec<&'static str> {
    ATTACHMENT_TYPES
        .iter()
        .filter(|(_, _, c)| *c == class)
        .map(|(ext, _, _)| *ext)
        .collect()
}

/// The table row for `path`'s extension (case-insensitive).
fn attachment_type(path: &Path) -> Option<(&'static str, AttachmentClass)> {
    let ext = path.extension()?.to_str()?;
    ATTACHMENT_TYPES
        .iter()
        .find(|(e, _, _)| e.eq_ignore_ascii_case(ext))
        .map(|(_, media, class)| (*media, *class))
}

/// The MIME type `path`'s extension carries, when the chat knows it.
pub fn known_media_type(path: &Path) -> Option<&'static str> {
    attachment_type(path).map(|(media, _)| media)
}

/// Whether `path` names a file type the chat can take as an attachment.
pub fn is_attachable(path: &Path) -> bool {
    attachment_type(path).is_some()
}

/// Read `path` into an attachment, refusing unsupported types, anything
/// that is not a regular file and files over [`MAX_ATTACHMENT_BYTES`]
/// before reading them whole. Disk I/O: hosts call it off the UI thread.
pub fn load_attachment(path: &Path) -> Result<ChatAttachment, AttachmentLoadError> {
    let display = path.display().to_string();
    let Some((media_type, _)) = attachment_type(path) else {
        return Err(AttachmentLoadError::Unsupported { path: display });
    };
    let io = |source| AttachmentLoadError::Io {
        path: path.display().to_string(),
        source,
    };
    let meta = fs::metadata(path).map_err(io)?;
    if !meta.is_file() {
        return Err(AttachmentLoadError::NotAFile { path: display });
    }
    if meta.len() > MAX_ATTACHMENT_BYTES as u64 {
        return Err(AttachmentLoadError::TooLarge {
            path: display,
            bytes: meta.len(),
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
        media_type: media_type.to_string(),
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_answers_type_class_and_filters() {
        assert_eq!(known_media_type(Path::new("a.PNG")), Some("image/png"));
        assert_eq!(
            known_media_type(Path::new("notes.md")),
            Some("text/markdown")
        );
        assert_eq!(known_media_type(Path::new("tool.exe")), None);
        assert!(attachable_extensions(AttachmentClass::Image).contains(&"webp"));
        assert!(attachable_extensions(AttachmentClass::Text).contains(&"svg"));
        assert!(!attachable_extensions(AttachmentClass::Image).contains(&"svg"));
    }

    #[test]
    fn a_folder_named_like_a_file_is_refused() {
        let dir = std::env::temp_dir().join(format!("de-attach-dir-{}.md", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            load_attachment(&dir),
            Err(AttachmentLoadError::NotAFile { .. })
        ));
        fs::remove_dir_all(&dir).ok();
    }
}
