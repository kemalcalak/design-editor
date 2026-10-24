//! Web arm of the APP MODE browser bar and website-style browsing
//! (#153). Mirrors native `widget_host/preview_browser.rs`; the bar's
//! paint / hit-test / press contract is shared through
//! `de_preview_core::browse_chrome`.

use de_editor_ui::{Point2D, Rect, RenderBackend};
use de_preview_core::browse_chrome;

impl super::WidgetHost {
    pub(in crate::widget_host) fn paint_preview_browser_bar(
        &self,
        backend: &mut dyn RenderBackend,
        canvas_rect: Rect,
    ) {
        browse_chrome::paint(
            self.preview.as_ref(),
            &self.editor_state.editor_ui.preview,
            backend,
            canvas_rect,
            &self.theme,
        );
    }

    pub(in crate::widget_host) fn preview_browser_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> bool {
        if self.preview_slideshow_active() {
            return false;
        }
        let canvas = self.preview_canvas_rect(viewport_w, viewport_h);
        browse_chrome::press(
            self.preview.as_ref(),
            &mut self.editor_state.editor_ui.preview,
            canvas,
            Point2D::new(x, y),
        )
    }

    pub(in crate::widget_host) fn preview_browser_release(&mut self) -> bool {
        let consumed = browse_chrome::release(
            self.preview.as_mut(),
            &mut self.editor_state.editor_ui.preview,
        );
        if consumed {
            self.mark_dirty();
        }
        consumed
    }

    pub(in crate::widget_host) fn preview_browser_hover(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) {
        let canvas = self.preview_canvas_rect(viewport_w, viewport_h);
        if browse_chrome::hover(
            self.preview.as_ref(),
            &mut self.editor_state.editor_ui.preview,
            canvas,
            Point2D::new(x, y),
        ) {
            self.mark_dirty();
        }
    }

    /// One history step from `Cmd/Ctrl+[` / `]` or a mouse back /
    /// forward button. Returns whether it navigated.
    pub fn preview_history_step(&mut self, forward: bool) -> bool {
        if !self.editor_state.editor_ui.preview.mode {
            return false;
        }
        let Some(session) = self.preview.as_mut() else {
            return false;
        };
        let navigated =
            browse_chrome::step(session, forward) != de_preview_core::InvalidationKind::None;
        if navigated {
            self.mark_dirty();
        }
        navigated
    }

    /// The CSS cursor for the last pointer position while previewing
    /// (`Some("pointer")` over a link, button or bar button, `Some("text")`
    /// over a text field, `Some("")` otherwise); `None` outside Preview,
    /// where the page keeps its default cursor.
    pub fn preview_css_cursor(&self) -> Option<&'static str> {
        let session = self.preview.as_ref()?;
        if !self.editor_state.editor_ui.preview.mode || self.preview_slideshow_active() {
            return None;
        }
        let (x, y) = (self.last_cursor_x, self.last_cursor_y);
        let canvas = self.preview_canvas_rect(self.last_viewport_w, self.last_viewport_h);
        let point = Point2D::new(x, y);
        if browse_chrome::contains(Some(session), canvas, point) {
            let over_button =
                de_editor_ui::widgets::PreviewBrowserBar::hit_test(canvas, point).is_some();
            return Some(if over_button { "pointer" } else { "" });
        }
        Some(match session.preview_cursor() {
            de_preview_core::PreviewCursor::Pointer => "pointer",
            de_preview_core::PreviewCursor::Text => "text",
            de_preview_core::PreviewCursor::Default => "",
        })
    }

    /// Apply a section link's page-scroll request (#153); see the native
    /// twin. Runs once per frame after `reconcile`.
    pub(in crate::widget_host) fn apply_preview_page_scroll_request(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
    ) {
        let Some(request) = self
            .preview
            .as_ref()
            .and_then(|session| session.take_page_scroll_request())
        else {
            return;
        };
        if self.device_mode_active() {
            if self.preview_device_frame.is_none() {
                self.recompute_device_frame(viewport_w, viewport_h);
            }
            let fit = self
                .preview_device_frame
                .as_ref()
                .map_or(1.0, |frame| frame.fit);
            let delta = self.preview_scroll_y - request.offset;
            self.apply_device_scroll(delta * fit);
        } else {
            const TOP_MARGIN: f32 = 24.0;
            let vp = &mut self.editor_state.viewport;
            vp.pan_y = TOP_MARGIN - (request.page_rect.origin.y + request.offset) * vp.zoom;
        }
        self.mark_dirty();
    }
}
