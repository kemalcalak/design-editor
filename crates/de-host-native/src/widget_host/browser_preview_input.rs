//! Native arm of the preview's embedded pages (#324). In Preview every
//! live browser page on screen takes the pointer and keys directly; the
//! routing is `de_editor_ui::widgets::browser_interact_flow`'s `preview_*`
//! methods over the pages the last preview frame showed
//! (`browser_preview`). This module keeps the preview's own chrome in
//! front (the floating cards and chrome, the chat, the device switcher,
//! the browser bar, the screen pills): an event over it is never offered
//! to a page.

use de_editor_core::browser_input::{InputModifiers, MouseButton};
use de_editor_ui::widgets::browser_interact_flow::{KeyInput, KeyOutcome};
use de_editor_ui::widgets::{browser_preview, host_canvas_geometry, ScreenSwitcherPills};
use de_editor_ui::Point2D;

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Whether the preview's pages may take input now: a preview that
    /// is not presenting a deck (a slide is shown, not used).
    pub fn preview_pages_active(&self) -> bool {
        self.preview.is_some() && !self.preview_slideshow_active()
    }

    /// Whether the preview's chrome covers `(x, y)`.
    pub(in crate::widget_host) fn over_preview_chrome(
        &self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> bool {
        let point = Point2D::new(x, y);
        // `over_canvas` is false over the floating cards (the left card
        // paints over the preview too).
        if !self.over_canvas(x, y, viewport_w, viewport_h)
            || host_canvas_geometry::over_floating_chrome(
                &self.editor_state,
                viewport_w,
                viewport_h,
                point,
            )
            || self.over_preview_switcher(x, y, viewport_w, viewport_h)
            || self.over_preview_browser_bar(x, y, viewport_w, viewport_h)
            || self.chat_panel_surface_contains(x, y, viewport_w, viewport_h)
        {
            return true;
        }
        self.preview.as_ref().is_some_and(|session| {
            session.is_app_mode() && {
                let count = session.screen_switcher_entries().len();
                let canvas = self.preview_canvas_rect(viewport_w, viewport_h);
                ScreenSwitcherPills::hit_test(canvas, count, point).is_some()
            }
        })
    }

    /// Whether a page may see a pointer event at `(x, y)`: over a page
    /// and off the chrome, or anywhere while a press on a page holds the
    /// pointer. The page record is checked first: it is empty in most
    /// previews, so the chrome hit tests rarely run per move.
    fn preview_page_may_see(&self, x: f32, y: f32, viewport_w: f32, viewport_h: f32) -> bool {
        self.preview_pages_active()
            && (self.browser_interaction.captures_pointer()
                || (browser_preview::page_at(Point2D::new(x, y)).is_some()
                    && !self.over_preview_chrome(x, y, viewport_w, viewport_h)))
    }

    /// Whether the pointer is over a page the preview drives.
    pub fn preview_over_page(&self, x: f32, y: f32, viewport_w: f32, viewport_h: f32) -> bool {
        self.preview_page_may_see(x, y, viewport_w, viewport_h)
            && self
                .browser_interaction
                .preview_over_page(&self.editor_state, x, y)
    }

    /// A pointer move in Preview; true when a page took it.
    pub fn preview_page_pointer_move(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> bool {
        self.preview_page_may_see(x, y, viewport_w, viewport_h)
            && self
                .browser_interaction
                .preview_pointer_move(&self.editor_state, x, y, modifiers)
    }

    /// A mouse button in Preview; true when a page took it (the preview
    /// must not see it). A press on the chrome takes the keys back.
    #[allow(clippy::too_many_arguments)]
    pub fn preview_page_pointer_button(
        &mut self,
        pressed: bool,
        button: MouseButton,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> bool {
        if !self.preview_pages_active() {
            return false;
        }
        // A press on the chrome is offered nowhere, so it lands off every
        // page and takes the keyboard back.
        let (x, y) = if self.preview_page_may_see(x, y, viewport_w, viewport_h) {
            (x, y)
        } else {
            (f32::NAN, f32::NAN)
        };
        self.browser_interaction.preview_pointer_button(
            &self.editor_state,
            pressed,
            button,
            x,
            y,
            modifiers,
            self.now_ms,
        )
    }

    /// A wheel or trackpad scroll in Preview (`dx` / `dy` as a pan); true
    /// when it scrolled a page instead of the preview.
    #[allow(clippy::too_many_arguments)]
    pub fn preview_page_wheel(
        &mut self,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> bool {
        self.preview_page_may_see(x, y, viewport_w, viewport_h)
            && self
                .browser_interaction
                .preview_wheel(&self.editor_state, x, y, dx, dy, modifiers)
    }

    /// A key in Preview: the page last pressed gets it.
    pub fn preview_page_key(&mut self, input: KeyInput, pressed: bool) -> KeyOutcome {
        if !self.preview_pages_active() {
            return KeyOutcome::Editor;
        }
        self.browser_interaction
            .preview_key(&self.editor_state, input, pressed)
    }

    /// Committed IME text in Preview; true when the focused page took it.
    pub fn preview_page_insert_text(&mut self, text: &str) -> bool {
        self.preview_pages_active()
            && self
                .browser_interaction
                .preview_insert_text(&self.editor_state, text)
    }

    /// Once per frame: the preview closed (or presents a deck), so its
    /// pages let go of the pointer and keys. Returns whether the preview's
    /// hold ended (a release may be queued; the caller flushes).
    pub fn sync_preview_pages(&mut self) -> bool {
        self.browser_interaction.sync_preview(&self.editor_state)
    }
}
