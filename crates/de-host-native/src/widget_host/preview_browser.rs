//! Native arm of the APP MODE browser bar and website-style browsing
//! (#153): back / forward from the bar, `Cmd/Ctrl+[` / `]` and the mouse
//! buttons, and section links that scroll the page.
//!
//! The bar's paint / hit-test / press contract is shared with the web
//! host through `de_preview_core::browse_chrome`; this file only maps it
//! onto the native host's canvas rect, theme and dirty flag.

use de_editor_ui::{Point2D, RenderBackend};
use de_preview_core::browse_chrome;

impl super::WidgetHostNative {
    pub(crate) fn paint_preview_browser_bar(
        &self,
        backend: &mut dyn RenderBackend,
        canvas: de_editor_ui::Rect,
    ) {
        browse_chrome::paint(
            self.preview.as_ref(),
            &self.editor_state.editor_ui.preview,
            backend,
            canvas,
            &self.theme,
        );
    }

    pub(crate) fn over_preview_browser_bar(
        &self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> bool {
        if self.preview_slideshow_active() {
            return false;
        }
        let canvas = self.preview_canvas_rect(viewport_w, viewport_h);
        browse_chrome::contains(self.preview.as_ref(), canvas, Point2D::new(x, y))
    }

    pub(crate) fn preview_browser_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> bool {
        let canvas = self.preview_canvas_rect(viewport_w, viewport_h);
        let hit = browse_chrome::press(
            self.preview.as_ref(),
            &mut self.editor_state.editor_ui.preview,
            canvas,
            Point2D::new(x, y),
        );
        if hit {
            self.mark_dirty();
        }
        hit
    }

    pub(crate) fn preview_browser_release(&mut self) -> bool {
        let consumed = browse_chrome::release(
            self.preview.as_mut(),
            &mut self.editor_state.editor_ui.preview,
        );
        if consumed {
            self.mark_dirty();
        }
        consumed
    }

    pub(crate) fn preview_browser_hover(
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

    /// One history step from a keyboard chord or a mouse back / forward
    /// button. Returns whether it navigated (the caller then repaints).
    pub fn preview_history_step(&mut self, forward: bool) -> bool {
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

    /// Apply a section link's page-scroll request (#153). Runs once per
    /// frame after `reconcile`: a framed preview scrolls its page, Canvas
    /// mode pans the viewport so the section's top sits just below the
    /// floating preview chrome. The math is shared with the web host
    /// (`browse_chrome`).
    pub(crate) fn apply_preview_page_scroll_request(&mut self, viewport_w: f32, viewport_h: f32) {
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
            let delta = browse_chrome::device_scroll_delta(&request, self.preview_scroll_y, fit);
            self.apply_device_scroll(delta);
        } else if let Some(session) = self.preview.as_ref() {
            let canvas = self.preview_canvas_rect(viewport_w, viewport_h);
            let zoom = self.editor_state.viewport.zoom;
            self.editor_state.viewport.pan_y =
                browse_chrome::canvas_pan_y_for_section(session, &request, canvas, zoom);
        }
        self.mark_dirty();
    }

    /// Canvas mode after a page switch: show the new page from its top
    /// when it is taller than the canvas, centred otherwise (#153).
    pub(crate) fn frame_canvas_on_switched_page(
        &mut self,
        page: de_editor_ui::Rect,
        viewport_w: f32,
        viewport_h: f32,
    ) {
        let Some(session) = self.preview.as_ref() else {
            return;
        };
        let canvas = self.preview_canvas_rect(viewport_w, viewport_h);
        let zoom = self.editor_state.viewport.zoom;
        let (pan_x, pan_y) = browse_chrome::canvas_pan_for_page(session, page, canvas, zoom);
        self.editor_state.viewport.pan_x = pan_x;
        self.editor_state.viewport.pan_y = pan_y;
        self.mark_dirty();
    }
}
