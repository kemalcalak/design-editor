//! The desktop's system clipboard: one long-lived handle behind a small
//! trait, so tests swap in a recorder, and the one place that writes the
//! clipboard requests the shared state queues.
//!
//! Every read and write in the desktop goes through
//! [`DesktopApp::clipboard`](crate::DesktopApp). [`OsClipboard`] opens its
//! `arboard` handle on first use and keeps it: a handle per call opened a
//! fresh X11 connection each time, and dropping one blocks while the
//! contents are handed to a clipboard manager.

use crate::clipboard::ClipboardImage;
use crate::DesktopApp;

/// One write to the system clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardWrite {
    /// Plain text (copied text, code, a commit hash, Copy as SVG).
    Text(String),
    /// A PNG, put on the clipboard as an image (Copy as PNG).
    PngImage(Vec<u8>),
    /// The node-copy marker (#436, `de_editor_core::node_clipboard_marker`).
    NodeMarker,
}

/// The system clipboard as the desktop uses it.
pub(crate) trait SystemClipboard {
    /// Best-effort write; a failure is swallowed.
    fn write(&mut self, write: ClipboardWrite);
    /// The text flavour, for a focused text input.
    fn read_text(&mut self) -> Option<String>;
    /// Image, else text, for the chat input.
    fn read_chat(&mut self) -> (Option<String>, Option<ClipboardImage>);
    /// HTML, else image, for a canvas paste.
    fn read_canvas(&mut self) -> (Option<String>, Option<ClipboardImage>);
}

/// The OS clipboard through one `arboard` handle, opened on first use.
/// A failed open is retried on the next call.
#[derive(Default)]
pub(crate) struct OsClipboard {
    handle: Option<arboard::Clipboard>,
}

impl OsClipboard {
    fn handle(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.handle.is_none() {
            self.handle = arboard::Clipboard::new().ok();
        }
        self.handle.as_mut()
    }
}

impl SystemClipboard for OsClipboard {
    fn write(&mut self, write: ClipboardWrite) {
        let Some(handle) = self.handle() else {
            return;
        };
        match write {
            ClipboardWrite::Text(text) => crate::clipboard::set_text(handle, &text),
            ClipboardWrite::PngImage(png) => crate::clipboard::set_png_image(handle, &png),
            ClipboardWrite::NodeMarker => crate::clipboard::set_node_marker(handle),
        }
    }

    fn read_text(&mut self) -> Option<String> {
        crate::clipboard::read_text_paste(self.handle()?)
    }

    fn read_chat(&mut self) -> (Option<String>, Option<ClipboardImage>) {
        self.handle()
            .map_or((None, None), crate::clipboard::read_chat_paste)
    }

    fn read_canvas(&mut self) -> (Option<String>, Option<ClipboardImage>) {
        self.handle()
            .map_or((None, None), crate::clipboard::read_canvas_paste)
    }
}

impl DesktopApp {
    /// Write `text` to the system clipboard.
    pub(crate) fn write_clipboard_text(&mut self, text: impl Into<String>) {
        self.clipboard.write(ClipboardWrite::Text(text.into()));
    }

    /// Carry out the clipboard writes the editor state asked for: text a
    /// copy affordance or shortcut queued (`chat.pending_copy_text`: code,
    /// transcript, an install snippet) and the node-copy marker a node copy
    /// / cut requested (#436). Called right after Cmd+C / Cmd+X and after a
    /// press, so the next Cmd+V never reads an older clipboard.
    pub(crate) fn flush_clipboard_requests(&mut self) {
        let state = self.host.editor_state_mut();
        let text = state.chat.pending_copy_text.take();
        let marker = std::mem::take(&mut state.editor_ui.canvas_view.pending_node_clipboard_marker);
        if let Some(text) = text {
            self.write_clipboard_text(text);
        }
        if marker {
            self.clipboard.write(ClipboardWrite::NodeMarker);
        }
    }
}

/// Test double: records writes, reads back nothing.
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct RecordingClipboard {
    pub(crate) writes: std::rc::Rc<std::cell::RefCell<Vec<ClipboardWrite>>>,
}

#[cfg(test)]
impl SystemClipboard for RecordingClipboard {
    fn write(&mut self, write: ClipboardWrite) {
        self.writes.borrow_mut().push(write);
    }

    fn read_text(&mut self) -> Option<String> {
        None
    }

    fn read_chat(&mut self) -> (Option<String>, Option<ClipboardImage>) {
        (None, None)
    }

    fn read_canvas(&mut self) -> (Option<String>, Option<ClipboardImage>) {
        (None, None)
    }
}
