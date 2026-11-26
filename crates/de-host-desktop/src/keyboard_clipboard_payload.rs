//! Injectable system-clipboard snapshot used by desktop paste routing.

use crate::system_clipboard::SystemClipboard;
use de_editor_core::node_clipboard_marker::is_node_clipboard_marker;

/// Every clipboard flavor relevant to Cmd/Ctrl+V.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClipboardPayload {
    pub(crate) text: Option<String>,
    pub(crate) html: Option<String>,
    pub(crate) image: Option<crate::clipboard::ClipboardImage>,
}

impl ClipboardPayload {
    pub(super) fn read_text_system(clipboard: &mut dyn SystemClipboard) -> Self {
        Self {
            text: clipboard.read_text(),
            ..Self::default()
        }
    }

    pub(super) fn read_chat_system(clipboard: &mut dyn SystemClipboard) -> Self {
        let (text, image) = clipboard.read_chat();
        Self {
            text,
            image,
            ..Self::default()
        }
    }

    pub(super) fn read_canvas_system(clipboard: &mut dyn SystemClipboard) -> Self {
        let (html, image) = clipboard.read_canvas();
        // SVG markup copied as text pastes as vectors (#399). Only read
        // when no richer flavour won, and only kept when it is SVG.
        let text = if html.as_deref().is_none_or(|h| h.trim().is_empty()) && image.is_none() {
            clipboard
                .read_text()
                .filter(|t| crate::clipboard::looks_like_svg(t))
        } else {
            None
        };
        Self { text, html, image }
    }

    /// The system clipboard's HTML is the editor's node-copy marker: the
    /// last copy was nodes in the editor, so a canvas paste uses the node
    /// clipboard (#436).
    pub(crate) fn holds_node_marker(&self) -> bool {
        self.html.as_deref().is_some_and(is_node_clipboard_marker)
    }
}

#[cfg(test)]
#[path = "keyboard_clipboard_marker_tests.rs"]
mod tests;
