//! Chat attachments on the desktop: the rfd file picker, drag-and-drop
//! and pasted files on the chat (#398).
//!
//! The headless helpers (reading a file into an attachment, preparing a
//! turn's attachments, base64, temp-file spill, the Claude image-Read flow)
//! live in `de_host_services::chat_attachment*`; this module picks the
//! file, reads it on a worker thread ([`AttachmentLoads`]), stages it and
//! says why when it cannot.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use de_editor_core::chat::{ChatAttachment, MAX_ATTACHMENTS, MAX_ATTACHMENT_BYTES};
use de_editor_core::editor_toast::EditorToastLevel;
use de_host_native::WidgetHostNative;
use de_host_services::chat_attachment_load::{
    attachable_extensions, is_attachable, load_attachment, AttachmentClass, AttachmentLoadError,
};

type Loaded = (String, Result<ChatAttachment, AttachmentLoadError>);

/// Files being read for the chat. Reading is disk I/O of up to
/// [`MAX_ATTACHMENT_BYTES`] per file, so it runs on a worker thread and the
/// result is staged when [`AttachmentLoads::drain`] collects it.
pub struct AttachmentLoads {
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    pending: usize,
}

impl Default for AttachmentLoads {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx, pending: 0 }
    }
}

impl AttachmentLoads {
    /// Whether a read is still running (the host keeps waking to collect
    /// it).
    pub fn is_pending(&self) -> bool {
        self.pending > 0
    }

    /// Read `path` on a worker thread.
    pub fn start(&mut self, path: PathBuf) {
        let tx = self.tx.clone();
        let name = file_name(&path);
        let spawned = thread::Builder::new()
            .name("de-chat-attach".into())
            .spawn(move || {
                let _ = tx.send((name, load_attachment(&path)));
            });
        match spawned {
            Ok(_) => self.pending += 1,
            Err(err) => eprintln!("design-editor: spawn attachment read failed: {err}"),
        }
    }

    /// Stage every finished read on the active chat (or raise its toast).
    /// Returns true when the chat changed.
    pub fn drain(&mut self, host: &mut WidgetHostNative) -> bool {
        let mut changed = false;
        while let Ok((name, loaded)) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            changed |= stage_loaded(host, &name, loaded);
        }
        changed
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string()
}

/// Drain `chat.pending_attachment_pick` (raised by the "+" menu's Attach
/// row): open a native picker for images and text files and start reading
/// the chosen file. Returns true when the picker ran.
pub fn drain_attachment_pick(host: &mut WidgetHostNative, loads: &mut AttachmentLoads) -> bool {
    if !std::mem::take(&mut host.editor_state_mut().chat.pending_attachment_pick) {
        return false;
    }
    let images = attachable_extensions(AttachmentClass::Image);
    let text = attachable_extensions(AttachmentClass::Text);
    let all: Vec<&str> = images.iter().chain(&text).copied().collect();
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Images and text", &all)
        .add_filter("Images", &images)
        .add_filter("Text", &text)
        .pick_file()
    {
        loads.start(path);
    }
    true
}

/// A pasted text that names one attachable file on disk (a copied path, or
/// a `file://` URL a Linux file manager puts on the clipboard as text).
pub fn pasted_file_path(text: &str) -> Option<PathBuf> {
    let line = text.trim();
    if line.is_empty() || line.contains('\n') {
        return None;
    }
    let path = match line.strip_prefix("file://") {
        Some(rest) => PathBuf::from(percent_decode(rest)?),
        None => PathBuf::from(line),
    };
    (path.is_absolute() && is_attachable(&path) && path.is_file()).then_some(path)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Stage a finished read as an attachment of the active chat, or raise a
/// toast naming why not (unsupported type, not a file, over the size cap,
/// unreadable, or the per-message count reached). Returns true when the
/// chat changed.
pub fn stage_loaded(
    host: &mut WidgetHostNative,
    name: &str,
    loaded: Result<ChatAttachment, AttachmentLoadError>,
) -> bool {
    let now = host.now_ms();
    let mut args = vec![("name".to_string(), name.to_string())];
    let key = match loaded {
        Ok(attachment) => {
            if host.editor_state_mut().chat.add_attachment(attachment) {
                host.editor_state_mut().chat.focused = true;
                return true;
            }
            args.push(("count".to_string(), MAX_ATTACHMENTS.to_string()));
            "chat.attach.full"
        }
        Err(err) => {
            eprintln!("design-editor: chat attachment refused: {err}");
            match err {
                AttachmentLoadError::Unsupported { .. } => "chat.attach.unsupported",
                AttachmentLoadError::TooLarge { .. } => {
                    let mb = MAX_ATTACHMENT_BYTES / (1024 * 1024);
                    args.push(("size".to_string(), mb.to_string()));
                    "chat.attach.tooLarge"
                }
                AttachmentLoadError::NotAFile { .. } | AttachmentLoadError::Io { .. } => {
                    "chat.attach.readFailed"
                }
            }
        }
    };
    host.editor_state_mut()
        .editor_ui
        .show_toast(key, args, EditorToastLevel::Warn, now);
    true
}

#[cfg(test)]
#[path = "chat_attachment_tests.rs"]
mod tests;
