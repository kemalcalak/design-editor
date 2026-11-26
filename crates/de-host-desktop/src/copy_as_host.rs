//! Copy as PNG / SVG (#399) — the desktop arm. The render is
//! `de_host_services::clipboard_export`; this writes the OS clipboard and
//! says how it went in a toast. Also flushes queued copy text.

use crate::DesktopApp;
use de_editor_core::copy_as::CopyAsFormat;
use de_editor_core::editor_toast::EditorToastLevel;
use de_host_services::clipboard_export::{selection_png, selection_svg};

impl DesktopApp {
    /// Edit ▸ Copy as PNG (Cmd+Alt+C) / Copy as SVG, and the layer menu
    /// rows. False with nothing selected (no toast, nothing copied).
    pub(crate) fn copy_selection_as(&mut self, format: CopyAsFormat) -> bool {
        let state = self.host.editor_state();
        if !state.selection.set.iter().any(|id| id.is_real()) {
            return false;
        }
        let written = match format {
            CopyAsFormat::Png => {
                selection_png(state).map(|png| crate::clipboard::set_png_image(&png))
            }
            CopyAsFormat::Svg => selection_svg(state).map(|svg| crate::clipboard::set_text(&svg)),
        };
        let (key, level) = match written {
            Ok(()) => (format.copied_key(), EditorToastLevel::Info),
            Err(err) => {
                eprintln!("[copy-as] {err}");
                (CopyAsFormat::FAILED_KEY, EditorToastLevel::Warn)
            }
        };
        let now = self.host.now_ms();
        self.host
            .editor_state_mut()
            .editor_ui
            .show_toast(key, Vec::new(), level, now);
        self.host.mark_editor_state_dirty();
        true
    }

    /// Write the text a copy affordance or shortcut queued
    /// (`chat.pending_copy_text`: code, transcript, the node-copy marker of
    /// #436) to the OS clipboard. Called right after Cmd+C / Cmd+X as well
    /// as after a press, so the next Cmd+V never reads an older clipboard.
    pub(crate) fn flush_pending_copy_text(&mut self) {
        if let Some(text) = self.host.editor_state_mut().chat.pending_copy_text.take() {
            crate::clipboard::set_text(&text);
        }
    }

    /// Run a Copy as … request a layer-menu row left on the state.
    pub(crate) fn drain_copy_as_request(&mut self) -> bool {
        let pending = self
            .host
            .editor_state_mut()
            .editor_ui
            .canvas_view
            .pending_copy_as
            .take();
        pending.is_some_and(|format| self.copy_selection_as(format))
    }
}
