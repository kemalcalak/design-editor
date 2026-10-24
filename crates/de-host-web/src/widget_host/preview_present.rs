//! Present from any page (#157), the web arm of native's
//! `widget_host/preview_present.rs`: Cmd/Ctrl+Enter presents the active
//! page's top-level frames as slides, whatever the document's scenario.

use super::WidgetHost;

impl WidgetHost {
    /// Start Present, or leave it when it is already running. From the
    /// interactive preview it switches straight to Present. Returns
    /// whether anything changed — `false` on a page with no frame or in a
    /// build without CanvasKit.
    pub fn toggle_present(&mut self) -> bool {
        let (viewport_w, viewport_h) = (self.last_viewport_w, self.last_viewport_h);
        if self.preview_slideshow_active() {
            self.exit_preview(viewport_w, viewport_h);
            return true;
        }
        #[cfg(feature = "canvaskit")]
        {
            if self.preview.is_some() {
                // The slideshow replaces the app preview without a merge
                // animation; the teardown clears the request below, so it
                // finishes first.
                self.finish_exit_teardown();
            }
            if !self.editor_state.request_present() {
                return false;
            }
            let op_ck = self.op_ck.clone();
            let entered = self.enter_preview_from_browser(viewport_w, viewport_h, op_ck.as_ref());
            if !entered {
                self.editor_state.editor_ui.preview.present = false;
            }
            entered
        }
        #[cfg(not(feature = "canvaskit"))]
        {
            false
        }
    }
}
