//! Chat attachments on the desktop: the rfd file picker and drag-and-drop
//! onto the chat (#398).
//!
//! The headless helpers (reading a file into an attachment, preparing a
//! turn's attachments, base64, temp-file spill, the Claude image-Read flow)
//! live in `de_host_services::chat_attachment*`; this module only picks the
//! file, stages it and says why when it cannot.

use std::path::Path;

use de_editor_core::editor_toast::EditorToastLevel;
use de_host_native::WidgetHostNative;
use de_host_services::chat_attachment_load::{
    load_attachment, AttachmentLoadError, ATTACHABLE_IMAGE_EXTENSIONS, ATTACHABLE_TEXT_EXTENSIONS,
};

/// Drain `chat.pending_attachment_pick` (raised by the "+" menu's Attach
/// row): open a native picker for images and text files and stage the
/// chosen file. Returns true when the chat changed (the caller redraws).
pub fn drain_attachment_pick(host: &mut WidgetHostNative) -> bool {
    if !std::mem::take(&mut host.editor_state_mut().chat.pending_attachment_pick) {
        return false;
    }
    let all: Vec<&str> = ATTACHABLE_IMAGE_EXTENSIONS
        .iter()
        .chain(ATTACHABLE_TEXT_EXTENSIONS)
        .copied()
        .collect();
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Images and text", &all)
        .add_filter("Images", ATTACHABLE_IMAGE_EXTENSIONS)
        .add_filter("Text", ATTACHABLE_TEXT_EXTENSIONS)
        .pick_file()
    else {
        return false;
    };
    stage_attachment_file(host, &path)
}

/// Stage `path` as an attachment of the active chat, or raise a toast
/// naming why not (unsupported type, over the size cap, unreadable, or the
/// per-message count reached). Returns true when the chat changed.
pub fn stage_attachment_file(host: &mut WidgetHostNative, path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let now = host.now_ms();
    let toast = |host: &mut WidgetHostNative, key: &str| {
        host.editor_state_mut().editor_ui.show_toast(
            key,
            vec![("name".to_string(), name.clone())],
            EditorToastLevel::Warn,
            now,
        );
        true
    };
    match load_attachment(path) {
        Ok(attachment) => {
            if host.editor_state_mut().chat.add_attachment(attachment) {
                host.editor_state_mut().chat.focused = true;
                true
            } else {
                toast(host, "chat.attach.full")
            }
        }
        Err(err) => {
            eprintln!("design-editor: chat attachment refused: {err}");
            let key = match err {
                AttachmentLoadError::Unsupported { .. } => "chat.attach.unsupported",
                AttachmentLoadError::TooLarge { .. } => "chat.attach.tooLarge",
                AttachmentLoadError::Io { .. } => "chat.attach.readFailed",
            };
            toast(host, key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("de-chat-attach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_text_file_is_staged_and_focuses_the_chat() {
        let mut host = WidgetHostNative::new();
        let path = temp_file("brief.md", b"# Brief");
        assert!(stage_attachment_file(&mut host, &path));
        let chat = &host.editor_state().chat;
        assert_eq!(chat.pending_attachments.len(), 1);
        assert_eq!(chat.pending_attachments[0].media_type, "text/markdown");
        assert!(chat.focused);
        assert!(host.editor_state().editor_ui.editor_toast.is_none());
    }

    #[test]
    fn a_refused_file_raises_a_toast_naming_it() {
        let mut host = WidgetHostNative::new();
        let path = temp_file("tool.exe", b"MZ");
        assert!(stage_attachment_file(&mut host, &path));
        let toast = host.editor_state().editor_ui.editor_toast.clone().unwrap();
        assert_eq!(toast.i18n_key, "chat.attach.unsupported");
        assert_eq!(
            toast.args,
            vec![("name".to_string(), "tool.exe".to_string())]
        );
        assert!(host.editor_state().chat.pending_attachments.is_empty());
    }

    #[test]
    fn the_fifth_file_is_refused_with_the_count_toast() {
        let mut host = WidgetHostNative::new();
        let path = temp_file("note.txt", b"hi");
        for _ in 0..de_editor_core::chat::MAX_ATTACHMENTS {
            stage_attachment_file(&mut host, &path);
        }
        assert!(host.editor_state().editor_ui.editor_toast.is_none());
        stage_attachment_file(&mut host, &path);
        let toast = host.editor_state().editor_ui.editor_toast.clone().unwrap();
        assert_eq!(toast.i18n_key, "chat.attach.full");
    }
}
