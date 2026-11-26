//! Copy as PNG / SVG (#399) — the desktop arm. The render is
//! `de_host_services::clipboard_export`; this writes the OS clipboard and
//! says how it went in a toast.

use crate::system_clipboard::ClipboardWrite;
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
            CopyAsFormat::Png => selection_png(state).map(ClipboardWrite::PngImage),
            CopyAsFormat::Svg => selection_svg(state).map(ClipboardWrite::Text),
        };
        let (key, level) = match written {
            Ok(write) => {
                self.clipboard.write(write);
                (format.copied_key(), EditorToastLevel::Info)
            }
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
