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
/// One batch of reads: the chat tab it was started for and its files' results
/// in the order they were given.
type Batch = (u64, Vec<Loaded>);

/// Files being read for the chat. Reading is disk I/O of up to
/// [`MAX_ATTACHMENT_BYTES`] per file, so it runs on a worker thread and the
/// results are staged when [`AttachmentLoads::drain`] collects them.
///
/// Files are queued first ([`AttachmentLoads::queue`]) and read as one batch
/// per drop or paste on one worker, in the order given. Only as many files as
/// the chat has room for ([`MAX_ATTACHMENTS`] minus what is staged and what is
/// already being read) are queued; the rest are refused with one toast. Each
/// batch remembers the tab it was started for and lands there.
pub struct AttachmentLoads {
    tx: Sender<Batch>,
    rx: Receiver<Batch>,
    /// Files being read on workers.
    pending: usize,
    /// Files queued for the next batch, and the tab they are for.
    queued: Vec<PathBuf>,
    queued_tab: u64,
    /// Files refused for want of room since the last drain: how many, and
    /// the first one's name (for the toast).
    refused: usize,
    refused_name: Option<String>,
}

impl Default for AttachmentLoads {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            pending: 0,
            queued: Vec::new(),
            queued_tab: 0,
            refused: 0,
            refused_name: None,
        }
    }
}

impl AttachmentLoads {
    /// Whether a read is queued or running, or a refusal waits for its toast
    /// (the host keeps waking to collect it).
    pub fn is_pending(&self) -> bool {
        self.pending > 0 || !self.queued.is_empty() || self.refused > 0
    }

    /// Queue `path` for the active chat tab, or refuse it when the tab has no
    /// room left. Nothing touches the disk here.
    pub fn queue(&mut self, host: &WidgetHostNative, path: PathBuf) {
        let chat = host.editor_state().chat.active();
        if chat.tab_id != self.queued_tab {
            self.flush();
            self.queued_tab = chat.tab_id;
        }
        let taken = chat.pending_attachments.len() + self.pending + self.queued.len();
        if taken >= MAX_ATTACHMENTS {
            self.refused += 1;
            self.refused_name.get_or_insert_with(|| file_name(&path));
            return;
        }
        self.queued.push(path);
    }

    /// Start reading the queued files: one worker for the whole batch.
    pub fn flush(&mut self) {
        if self.queued.is_empty() {
            return;
        }
        let paths = std::mem::take(&mut self.queued);
        let count = paths.len();
        let tab = self.queued_tab;
        let tx = self.tx.clone();
        let spawned = thread::Builder::new()
            .name("de-chat-attach".into())
            .spawn(move || {
                let loaded = paths
                    .iter()
                    .map(|path| (file_name(path), load_attachment(path)))
                    .collect();
                let _ = tx.send((tab, loaded));
            });
        match spawned {
            Ok(_) => self.pending += count,
            Err(err) => eprintln!("design-editor: spawn attachment read failed: {err}"),
        }
    }

    /// Start the queued batch, raise the "full" toast for refused files and
    /// stage every finished batch on the tab it was started for (or raise
    /// its toasts). Returns true when the chat changed.
    pub fn drain(&mut self, host: &mut WidgetHostNative) -> bool {
        self.flush();
        let mut changed = false;
        if self.refused > 0 {
            let name = self.refused_name.take().unwrap_or_default();
            self.refused = 0;
            show_full_toast(host, &name);
            changed = true;
        }
        while let Ok((tab, batch)) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(batch.len());
            for (name, loaded) in batch {
                changed |= stage_loaded(host, tab, &name, loaded);
            }
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

fn show_full_toast(host: &mut WidgetHostNative, name: &str) {
    let now = host.now_ms();
    let args = vec![
        ("name".to_string(), name.to_string()),
        ("count".to_string(), MAX_ATTACHMENTS.to_string()),
    ];
    host.editor_state_mut().editor_ui.show_toast(
        "chat.attach.full",
        args,
        EditorToastLevel::Warn,
        now,
    );
}

/// Drain `chat.pending_attachment_pick` (raised by the "+" menu's Attach
/// row): open a native picker for images and text files and queue the
/// chosen file. Returns true when the picker ran.
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
        loads.queue(host, path);
    }
    true
}

/// A pasted text that is one `file://` URL naming an attachable file (what a
/// Linux file manager puts on the clipboard as text). A plain pasted path
/// stays text: it is typed, never looked up on disk here. Whether the file
/// exists is the worker's read to find out.
pub fn pasted_file_url(text: &str) -> Option<PathBuf> {
    file_url_path(text, cfg!(windows))
}

/// [`pasted_file_url`] for a given platform: on Windows `file:///C:/x.md`
/// is `C:/x.md`.
fn file_url_path(text: &str, windows: bool) -> Option<PathBuf> {
    let line = text.trim();
    if line.contains('\n') {
        return None;
    }
    let rest = line.strip_prefix("file://")?;
    // `file://localhost/x` names the same file as `file:///x`.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let mut decoded = percent_decode(rest)?;
    if windows {
        let b = decoded.as_bytes();
        if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
            decoded.remove(0);
        }
    }
    let path = PathBuf::from(decoded);
    let absolute = path.is_absolute() || windows;
    (absolute && is_attachable(&path)).then_some(path)
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

/// Stage a finished read as an attachment of chat tab `tab` (by
/// [`ChatState::tab_id`]; dropped silently when that tab was closed), or
/// raise a toast naming why not (unsupported type, not a file, over the size
/// cap, unreadable, or the per-message count reached). Returns true when the
/// chat changed.
///
/// [`ChatState::tab_id`]: de_editor_core::ChatState::tab_id
pub fn stage_loaded(
    host: &mut WidgetHostNative,
    tab: u64,
    name: &str,
    loaded: Result<ChatAttachment, AttachmentLoadError>,
) -> bool {
    let now = host.now_ms();
    let mut args = vec![("name".to_string(), name.to_string())];
    let key = match loaded {
        Ok(attachment) => {
            let Some(chat) = host.editor_state_mut().chat.tab_by_id_mut(tab) else {
                return false;
            };
            if chat.add_attachment(attachment) {
                chat.focused = true;
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
