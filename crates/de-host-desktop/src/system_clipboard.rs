//! The desktop's system clipboard: one long-lived handle behind a small
//! trait, so tests swap in a recorder, and the one place that writes the
//! clipboard requests the shared state queues.
//!
//! Every read and write in the desktop goes through
//! [`DesktopApp::clipboard`](crate::DesktopApp). [`OsClipboard`] opens its
//! `arboard` handle on first use and keeps it until a call says it broke:
//! a handle per call opened a fresh X11 connection each time, and dropping
//! one blocks while the contents are handed to a clipboard manager.

use crate::clipboard::ClipboardImage;
use crate::clipboard_backend::{self, ClipboardBackend, Read};
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

/// The OS clipboard through one handle, opened on first use and kept.
/// A call whose handle may have broken (`clipboard_backend::breaks_handle`)
/// drops it, so the next call opens a new one: a dead X11 / Wayland
/// connection recovers on its own instead of failing every later call.
pub(crate) struct OsClipboard<B: ClipboardBackend = arboard::Clipboard> {
    handle: Option<B>,
    open: Box<dyn FnMut() -> Option<B>>,
}

impl Default for OsClipboard {
    fn default() -> Self {
        Self::with_opener(|| arboard::Clipboard::new().ok())
    }
}

impl<B: ClipboardBackend> OsClipboard<B> {
    /// A clipboard that opens its handles with `open` (a fake in tests).
    pub(crate) fn with_opener(open: impl FnMut() -> Option<B> + 'static) -> Self {
        Self {
            handle: None,
            open: Box::new(open),
        }
    }

    fn handle(&mut self) -> Option<&mut B> {
        if self.handle.is_none() {
            self.handle = (self.open)();
        }
        self.handle.as_mut()
    }

    /// Drop the handle when the call reported it may have broken.
    fn settle<T>(&mut self, read: Read<T>) -> T {
        if read.handle_failed {
            self.handle = None;
        }
        read.value
    }
}

impl<B: ClipboardBackend> SystemClipboard for OsClipboard<B> {
    fn write(&mut self, write: ClipboardWrite) {
        let Some(handle) = self.handle() else {
            return;
        };
        let result = match write {
            ClipboardWrite::Text(text) => clipboard_backend::set_text(handle, &text),
            ClipboardWrite::PngImage(png) => clipboard_backend::set_png_image(handle, &png),
            ClipboardWrite::NodeMarker => clipboard_backend::set_node_marker(handle),
        };
        if let Err(error) = result {
            if clipboard_backend::breaks_handle(&error) {
                self.handle = None;
            }
        }
    }

    fn read_text(&mut self) -> Option<String> {
        let read = clipboard_backend::read_text_paste(self.handle()?);
        self.settle(read)
    }

    fn read_chat(&mut self) -> (Option<String>, Option<ClipboardImage>) {
        let Some(handle) = self.handle() else {
            return (None, None);
        };
        let read = clipboard_backend::read_chat_paste(handle);
        self.settle(read)
    }

    fn read_canvas(&mut self) -> (Option<String>, Option<ClipboardImage>) {
        let Some(handle) = self.handle() else {
            return (None, None);
        };
        let read = clipboard_backend::read_canvas_paste(handle);
        self.settle(read)
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

#[cfg(test)]
#[path = "system_clipboard_tests.rs"]
mod tests;
