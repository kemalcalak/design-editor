//! Injectable system-clipboard snapshot used by desktop paste routing.

use de_editor_core::node_clipboard_marker::is_node_clipboard_marker;

/// Every clipboard flavor relevant to Cmd/Ctrl+V.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClipboardPayload {
    pub(crate) text: Option<String>,
    pub(crate) html: Option<String>,
    pub(crate) image: Option<crate::clipboard::ClipboardImage>,
}

impl ClipboardPayload {
    pub(super) fn read_text_system() -> Self {
        Self {
            text: crate::clipboard::read_text_paste(),
            ..Self::default()
        }
    }

    pub(super) fn read_chat_system() -> Self {
        let (text, image) = crate::clipboard::read_chat_paste();
        Self {
            text,
            image,
            ..Self::default()
        }
    }

    pub(super) fn read_canvas_system() -> Self {
        let (html, image) = crate::clipboard::read_canvas_paste();
        // SVG markup copied as text pastes as vectors (#399), and the
        // node-copy marker (#436) says the node clipboard is current. Only
        // read when no richer flavour won (writing the marker replaces
        // them all), and only kept when it is one of the two.
        let text = if html.as_deref().is_none_or(|h| h.trim().is_empty()) && image.is_none() {
            crate::clipboard::read_text_paste()
                .filter(|t| crate::clipboard::looks_like_svg(t) || is_node_clipboard_marker(t))
        } else {
            None
        };
        Self { text, html, image }
    }

    /// The system clipboard still holds the editor's node-copy marker: the
    /// last copy was nodes in the editor, so a canvas paste uses the node
    /// clipboard (#436).
    pub(crate) fn holds_node_marker(&self) -> bool {
        self.text.as_deref().is_some_and(is_node_clipboard_marker)
    }

    /// Text for a focused text input. The node-copy marker is not text a
    /// user copied, so it pastes nothing.
    pub(crate) fn input_text(&self) -> Option<&str> {
        self.text
            .as_deref()
            .filter(|text| !is_node_clipboard_marker(text))
    }
}

#[cfg(test)]
#[path = "keyboard_clipboard_marker_tests.rs"]
mod tests;
