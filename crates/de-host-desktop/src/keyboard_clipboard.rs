//! Cmd+C / Cmd+X / Cmd+V dispatch and the paste router. Split out of
//! `keyboard_input.rs` (pure code motion) to keep it under the repo's
//! 800-line-per-file cap.

use crate::keyboard_clipboard_payload::ClipboardPayload;
use crate::{html_import_session, DesktopApp};
use base64::Engine as _;

impl DesktopApp {
    /// Cmd+C dispatch. The chat input copies its whole buffer (its own
    /// selection model); any other focused text field copies its
    /// highlighted slice; with no input focused, the document node
    /// clipboard (or a selected code / transcript slice) is copied via
    /// [`WidgetHostNative::apply_copy`]. The text-input branches always
    /// consume the chord so Cmd+C never falls through to node copy
    /// while a field owns the keyboard.
    ///
    /// `pub(crate)` so the native Edit-menu dispatch
    /// (`menu_action.rs`) routes through the same input-aware path —
    /// on macOS / Windows the menu accelerator owns Cmd+C and the
    /// winit keydown never reaches `handle_key_pressed`.
    pub(crate) fn handle_cmd_copy(&mut self) -> bool {
        if self.host.chat_input_owns_keyboard_pub() {
            let text = self.host.editor_state().chat.input.text().to_owned();
            if !text.is_empty() {
                self.write_clipboard_text(text);
            }
            return true;
        }
        if self.host.input_active_pub() {
            if let Some(text) = self.host.input_copy_text() {
                self.write_clipboard_text(text);
            }
            return true;
        }
        let copied = self.host.apply_copy();
        self.flush_clipboard_requests();
        copied
    }

    /// Cmd+X dispatch. Cuts the focused text input's selection to the OS
    /// clipboard (chat input via its own cut path), else cuts the
    /// document node selection. `pub(crate)` for the Edit-menu path.
    pub(crate) fn handle_cmd_cut(&mut self) -> bool {
        if self.host.chat_input_owns_keyboard_pub() {
            if let Some(text) = self.host.chat_input_cut() {
                self.write_clipboard_text(text);
            }
            return true;
        }
        if self.host.input_active_pub() {
            if let Some(text) = self.host.input_cut_text() {
                self.write_clipboard_text(text);
            }
            return true;
        }
        let cut = self.host.apply_cut();
        self.flush_clipboard_requests();
        cut
    }

    /// Cmd+V dispatch. Pastes OS clipboard text into whichever text
    /// input owns the keyboard (the chat input also accepts a pasted
    /// image, per `ai-chat-input.tsx:85-94`); with no input focused,
    /// pastes Figma clipboard HTML or the document node clipboard onto
    /// the canvas. `pub(crate)` for the Edit-menu path.
    pub(crate) fn handle_cmd_paste(&mut self) -> bool {
        let clipboard = self.clipboard.as_mut();
        let payload = if self.host.non_chat_input_owns_keyboard_pub() {
            ClipboardPayload::read_text_system(clipboard)
        } else if self.host.chat_input_owns_keyboard_pub() {
            ClipboardPayload::read_chat_system(clipboard)
        } else {
            ClipboardPayload::read_canvas_system(clipboard)
        };
        self.handle_paste_payload(payload)
    }

    /// Input-aware paste router over an already-read clipboard snapshot.
    /// Keeping OS access outside this seam makes precedence directly
    /// testable and guarantees one clipboard handle per paste gesture.
    pub(crate) fn handle_paste_payload(&mut self, mut payload: ClipboardPayload) -> bool {
        // Non-chat text inputs (settings, git, rename, etc.) are
        // checked first so that opening a modal (e.g. agent settings
        // via Cmd+,) while the chat input is focused routes the paste
        // to the modal's field instead of the chat.
        if self.host.non_chat_input_owns_keyboard_pub() {
            if let Some(text) = payload.text.as_deref() {
                self.host.apply_input_paste(text);
            }
            return true;
        }
        if self.host.chat_input_owns_keyboard_pub() {
            // Copied files, then clipboard image data, then a pasted path to
            // an attachable file become attachments (#398); other text is
            // typed. The paste is consumed either way.
            if !payload.files.is_empty() {
                for path in payload.files {
                    self.attachment_loads.start(path);
                }
            } else if let Some(image) = payload.image.take() {
                self.paste_image_into_chat(image);
            } else if let Some(text) = payload.text.as_deref() {
                match crate::chat_attachment::pasted_file_path(text) {
                    Some(path) => self.attachment_loads.start(path),
                    None => {
                        self.host.chat_input_paste(text);
                    }
                }
            }
            return true;
        }
        if payload.holds_node_marker() {
            return self.host.apply_paste();
        }
        if let Some(html) = payload.html.take() {
            if let Some(result) = self.try_figma_clipboard_paste(html.clone()) {
                return result;
            }
            if let Some(result) = self.try_html_clipboard_paste(html) {
                return result;
            }
        }
        if let Some(image) = payload.image {
            return self.paste_image_to_canvas(image);
        }
        if let Some(svg) = payload
            .text
            .as_deref()
            .filter(|t| crate::clipboard::looks_like_svg(t))
        {
            return self.paste_svg_text_to_canvas(svg);
        }
        self.host.apply_paste()
    }

    /// SVG markup on the clipboard becomes vectors at the canvas centre
    /// (#399); markup with no shapes falls back to the node clipboard.
    fn paste_svg_text_to_canvas(&mut self, svg: &str) -> bool {
        let centre = de_editor_ui::widgets::host_canvas_geometry::canvas_centre_doc_point(
            self.host.editor_state(),
            self.viewport_width,
            self.viewport_height,
        );
        let mut next_id = 0;
        let pasted = self.host.editor_state_mut().paste_svg_centered(
            &mut next_id,
            svg,
            (f64::from(centre.x), f64::from(centre.y)),
        );
        if pasted == 0 {
            return self.host.apply_paste();
        }
        self.host.mark_editor_state_dirty();
        true
    }

    /// Stage a clipboard image as a chat attachment. Image data wins over
    /// text, and `add_attachment` enforces the shared count/size limits.
    fn paste_image_into_chat(&mut self, image: crate::clipboard::ClipboardImage) {
        self.host
            .editor_state_mut()
            .chat
            .add_attachment(de_editor_core::chat::ChatAttachment {
                name: "pasted-image.png".to_string(),
                media_type: "image/png".to_string(),
                data: image.png,
            });
    }

    fn paste_image_to_canvas(&mut self, image: crate::clipboard::ClipboardImage) -> bool {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&image.png);
        let src = format!("data:image/png;base64,{encoded}");
        let centre = de_editor_ui::widgets::host_canvas_geometry::canvas_centre_doc_point(
            self.host.editor_state(),
            self.viewport_width,
            self.viewport_height,
        );
        let inserted = self
            .host
            .editor_state_mut()
            .insert_image_node_at_doc_point_sized(
                "pasted-image.png",
                &src,
                image.width,
                image.height,
                (centre.x as f64, centre.y as f64),
            )
            .is_some();
        if inserted {
            self.host.mark_editor_state_dirty();
        }
        inserted
    }

    /// Probe the system clipboard for Figma HTML (Cmd+C in Figma) and
    /// kick off the decode on a worker thread — the base64 + kiwi
    /// `.fig` parse can be heavy and must not stall the keyboard
    /// handler. The UI thread only reads the clipboard and sniffs the
    /// marker; `pump_figma_clipboard_paste` applies the parsed nodes
    /// on a later frame. `None` when the clipboard holds no Figma
    /// payload (caller falls back to the internal node clipboard).
    fn try_figma_clipboard_paste(&mut self, html: String) -> Option<bool> {
        if !de_figma::is_figma_clipboard_html(&html) {
            return None;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        // Detached one-shot: a pure CPU decode over an owned `String` with no
        // IO and no loop, so it always returns. The epoch guard below (not a
        // thread signal) is what discards a stale result; a dropped `rx` just
        // makes the send a no-op.
        std::thread::spawn(move || {
            let nodes = de_figma::extract_figma_clipboard_data(&html)
                .map(|data| de_figma::figma_clipboard_to_nodes(&data.buffer, Some(&html)).nodes)
                .unwrap_or_default();
            let _ = tx.send(nodes);
        });
        // Capture the document epoch so a decode that finishes after
        // the user opens / creates / imports another document is
        // dropped instead of landing in the wrong document.
        self.pending_figma_paste = Some((self.host.document_epoch(), rx));
        // Consumed — the paste lands asynchronously (or decodes to
        // nothing and is silently dropped, never raw-HTML-pasted).
        Some(true)
    }

    /// Drain a finished clipboard decode — inserts the nodes centred
    /// on the viewport. Called once per frame by the redraw path.
    pub(crate) fn pump_figma_clipboard_paste(&mut self) -> bool {
        let Some((epoch, rx)) = self.pending_figma_paste.as_ref() else {
            return false;
        };
        let epoch = *epoch;
        match rx.try_recv() {
            Ok(nodes) => {
                self.pending_figma_paste = None;
                // Drop the result if the document was replaced while
                // the worker was decoding.
                epoch == self.host.document_epoch()
                    && !nodes.is_empty()
                    && self
                        .host
                        .paste_figma_nodes(nodes, self.viewport_width, self.viewport_height)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.pending_figma_paste = None;
                false
            }
        }
    }

    /// Probe pasted HTML (any non-Figma `text/html` payload) and
    /// convert it to editable nodes on a worker thread — the CSS
    /// cascade can be heavy and must not stall the keyboard handler.
    /// `pump_html_clipboard_paste` applies the parsed nodes on a
    /// later frame. `None` when the payload is blank (caller falls
    /// back to the image / internal clipboard branches).
    fn try_html_clipboard_paste(&mut self, html: String) -> Option<bool> {
        if !html_paste_should_consume(&html) {
            return None;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        // Detached one-shot; same contract as `try_figma_clipboard_paste` — a
        // self-terminating CPU decode, staleness handled by the epoch guard.
        std::thread::spawn(move || {
            let result = de_html::import_html(&html, &de_html::HtmlImportOptions::default());
            // TYPED diagnostics travel: the overlay localizes each row.
            let _ = tx.send((result.nodes, result.diagnostics));
        });
        // Epoch-guard the decode (see `try_figma_clipboard_paste`).
        self.pending_html_paste = Some((self.host.document_epoch(), rx));
        // Consumed — the paste lands asynchronously (or decodes to
        // nothing and is silently dropped, never raw-HTML-pasted).
        Some(true)
    }

    /// Drain a finished HTML decode — inserts the nodes centred on
    /// the viewport. Called once per frame by the redraw path.
    pub(crate) fn pump_html_clipboard_paste(&mut self) -> bool {
        let Some((epoch, rx)) = self.pending_html_paste.as_ref() else {
            return false;
        };
        let epoch = *epoch;
        match rx.try_recv() {
            Ok((nodes, diagnostics)) => {
                self.pending_html_paste = None;
                for warning in de_html::render_warnings(&diagnostics) {
                    eprintln!("[import-html] warning: {warning}");
                }
                // Drop the result if the document was replaced while
                // the worker was decoding.
                if epoch != self.host.document_epoch() {
                    return false;
                }
                // A pasted page degrades exactly like an imported file, so it
                // gets the same GUI report instead of stderr-only warnings the
                // user never sees; an empty list clears a stale card.
                let rows = html_import_session::diagnostic_rows(&diagnostics);
                let reported = !rows.is_empty();
                self.host.show_html_import_diagnostics(rows);
                let (w, h) = (self.viewport_width, self.viewport_height);
                (!nodes.is_empty() && self.host.paste_figma_nodes(nodes, w, h)) || reported
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.pending_html_paste = None;
                false
            }
        }
    }
}

/// Whether a pasted `text/html` payload should be consumed by the
/// HTML importer (any non-blank markup qualifies; Figma payloads are
/// intercepted by `try_figma_clipboard_paste` first).
pub(crate) fn html_paste_should_consume(html: &str) -> bool {
    !html.trim().is_empty()
}
