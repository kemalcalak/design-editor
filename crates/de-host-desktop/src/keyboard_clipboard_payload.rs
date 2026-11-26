//! Injectable system-clipboard snapshot used by desktop paste routing.

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
        // SVG markup copied as text pastes as vectors (#399). Only read
        // when no richer flavour won, and only kept when it is SVG.
        let text = if html.as_deref().is_none_or(|h| h.trim().is_empty()) && image.is_none() {
            crate::clipboard::read_text_paste().filter(|t| crate::clipboard::looks_like_svg(t))
        } else {
            None
        };
        Self { text, html, image }
    }
}
