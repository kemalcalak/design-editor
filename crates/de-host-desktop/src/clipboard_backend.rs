//! The flavour-level clipboard backend (`arboard::Clipboard`, a fake in
//! tests) and the paste readers / writers over it.
//!
//! Every reader and writer reports whether the handle itself may have
//! broken, so `system_clipboard::OsClipboard` can drop it and open a new
//! one on the next call: a long-lived X11 / Wayland connection can die
//! with the display server's side of it, and without a reset every later
//! clipboard call would keep failing. Only `Error::Unknown` counts — that
//! is where arboard reports connection and protocol failures. "Nothing in
//! this flavour" (`ContentNotAvailable`), a payload that would not convert
//! (`ConversionFailure`) and a clipboard another party holds for a moment
//! (`ClipboardOccupied`) say nothing about the handle. (Windows reports a
//! missing HTML flavour as `Unknown` too; a reset there only rebuilds a
//! small struct, so the rule stays simple.)

use crate::clipboard::{encode_image, png_to_rgba, ClipboardImage};
use arboard::{Error, ImageData};

/// One clipboard handle, flavour by flavour.
pub(crate) trait ClipboardBackend {
    fn get_text(&mut self) -> Result<String, Error>;
    fn get_html(&mut self) -> Result<String, Error>;
    fn get_image(&mut self) -> Result<ImageData<'static>, Error>;
    fn set_text(&mut self, text: &str) -> Result<(), Error>;
    fn set_image(&mut self, image: ImageData<'static>) -> Result<(), Error>;
    /// HTML with no plain-text flavour, kept out of clipboard history.
    fn set_private_html(&mut self, html: &str) -> Result<(), Error>;
    /// A platform reader that answers a canvas paste without this handle
    /// (the Wayland fast path); `None` falls through to the handle.
    fn canvas_fast_path(&mut self) -> Option<(Option<String>, Option<ClipboardImage>)> {
        None
    }
}

impl ClipboardBackend for arboard::Clipboard {
    fn get_text(&mut self) -> Result<String, Error> {
        arboard::Clipboard::get_text(self)
    }

    fn get_html(&mut self) -> Result<String, Error> {
        self.get().html()
    }

    fn get_image(&mut self) -> Result<ImageData<'static>, Error> {
        arboard::Clipboard::get_image(self)
    }

    fn set_text(&mut self, text: &str) -> Result<(), Error> {
        arboard::Clipboard::set_text(self, text)
    }

    fn set_image(&mut self, image: ImageData<'static>) -> Result<(), Error> {
        arboard::Clipboard::set_image(self, image)
    }

    fn set_private_html(&mut self, html: &str) -> Result<(), Error> {
        exclude_from_history(self.set()).html(html, None)
    }

    #[cfg(target_os = "linux")]
    fn canvas_fast_path(&mut self) -> Option<(Option<String>, Option<ClipboardImage>)> {
        crate::clipboard::read_wayland_canvas_paste()
    }
}

#[cfg(target_os = "macos")]
fn exclude_from_history(set: arboard::Set<'_>) -> arboard::Set<'_> {
    use arboard::SetExtApple;
    set.exclude_from_history()
}

#[cfg(windows)]
fn exclude_from_history(set: arboard::Set<'_>) -> arboard::Set<'_> {
    use arboard::SetExtWindows;
    set.exclude_from_history()
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
))]
fn exclude_from_history(set: arboard::Set<'_>) -> arboard::Set<'_> {
    use arboard::SetExtLinux;
    set.exclude_from_history()
}

#[cfg(not(any(
    target_os = "macos",
    windows,
    all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
    )
)))]
fn exclude_from_history(set: arboard::Set<'_>) -> arboard::Set<'_> {
    set
}

/// What a read found, and whether the handle may have broken on the way.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Read<T> {
    pub(crate) value: T,
    pub(crate) handle_failed: bool,
}

/// Whether `error` may mean the handle itself broke.
pub(crate) fn breaks_handle(error: &Error) -> bool {
    matches!(error, Error::Unknown { .. })
}

/// One flavour: `None` when absent or failed, noting a broken handle.
fn flavour<T>(result: Result<T, Error>, handle_failed: &mut bool) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            *handle_failed |= breaks_handle(&error);
            None
        }
    }
}

/// The text flavour, for a focused editor input. Canvas and chat paste use
/// their own readers so a text paste never transfers and PNG-encodes a
/// clipboard bitmap.
pub(crate) fn read_text_paste(clipboard: &mut impl ClipboardBackend) -> Read<Option<String>> {
    let mut handle_failed = false;
    let value = flavour(clipboard.get_text(), &mut handle_failed);
    Read {
        value,
        handle_failed,
    }
}

/// The flavours relevant to a focused chat input. Images win over text,
/// matching the paste router, so the losing flavour is never transferred.
pub(crate) fn read_chat_paste(
    clipboard: &mut impl ClipboardBackend,
) -> Read<(Option<String>, Option<ClipboardImage>)> {
    let mut handle_failed = false;
    let image = flavour(clipboard.get_image(), &mut handle_failed).and_then(encode_image);
    let text = match image {
        Some(_) => None,
        None => flavour(clipboard.get_text(), &mut handle_failed),
    };
    Read {
        value: (text, image),
        handle_failed,
    }
}

/// The external flavours relevant to a canvas paste. HTML wins over images
/// (the node-copy marker, Figma and editable HTML ingestion); plain text is
/// read separately and only when neither is there.
pub(crate) fn read_canvas_paste(
    clipboard: &mut impl ClipboardBackend,
) -> Read<(Option<String>, Option<ClipboardImage>)> {
    if let Some(value) = clipboard.canvas_fast_path() {
        return Read {
            value,
            handle_failed: false,
        };
    }
    let mut handle_failed = false;
    let html = flavour(clipboard.get_html(), &mut handle_failed);
    if html
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Read {
            value: (html, None),
            handle_failed,
        };
    }
    let image = flavour(clipboard.get_image(), &mut handle_failed).and_then(encode_image);
    Read {
        value: (html, image),
        handle_failed,
    }
}

/// Write `text`.
pub(crate) fn set_text(clipboard: &mut impl ClipboardBackend, text: &str) -> Result<(), Error> {
    clipboard.set_text(text)
}

/// Put a PNG on the clipboard as an image (Copy as PNG, #399). arboard
/// takes raw RGBA, so the PNG is decoded first; one that does not decode
/// is a `ConversionFailure`.
pub(crate) fn set_png_image(
    clipboard: &mut impl ClipboardBackend,
    png: &[u8],
) -> Result<(), Error> {
    let image = png_to_rgba(png).ok_or(Error::ConversionFailure)?;
    clipboard.set_image(image)
}

/// Put the node-copy marker on the clipboard (#436): HTML only, so no
/// plain-text flavour exists for a text field, editor or terminal to paste
/// (Windows stores an empty text alongside), and kept out of
/// clipboard-history tools, since it means nothing outside this editor.
pub(crate) fn set_node_marker(clipboard: &mut impl ClipboardBackend) -> Result<(), Error> {
    clipboard.set_private_html(de_editor_core::node_clipboard_marker::NODE_CLIPBOARD_MARKER_HTML)
}
