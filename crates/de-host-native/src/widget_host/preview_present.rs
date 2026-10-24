//! Present from any page (#157): Cmd/Ctrl+Enter presents the active
//! page's top-level frames as slides, in canvas order, whatever the
//! document's scenario. The slideshow itself is the deck arm in
//! `preview_slideshow.rs`; this only asks for it and toggles it.

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Start Present, or leave it when it is already running. From the
    /// interactive preview it switches straight to Present. Returns
    /// whether anything changed — `false` on a page with no frame.
    pub fn toggle_present(&mut self, canvas_size: (f32, f32)) -> bool {
        if self.preview_slideshow_active() {
            self.exit_preview();
            return true;
        }
        if self.preview.is_some() {
            // No merge animation: the slideshow replaces the app preview
            // on the next frame. The teardown has to finish before the
            // request below, which clears with the rest of the preview
            // state on exit.
            self.finish_exit_teardown();
        }
        if !self.editor_state.request_present() {
            return false;
        }
        let entered = self.enter_preview(canvas_size);
        if !entered {
            self.editor_state.editor_ui.preview.present = false;
        }
        entered
    }

    /// [`Self::toggle_present`] against the last known canvas size — the
    /// keyboard and menu paths have no viewport of their own.
    pub fn toggle_present_with_cached_viewport(&mut self) -> bool {
        let (_x, _y, width, height) =
            self.canvas_region(self.last_viewport_w, self.last_viewport_h);
        self.toggle_present((width, height))
    }
}

#[cfg(all(test, not(target_os = "windows")))]
#[path = "preview_present_tests.rs"]
mod tests;
