//! `apply_press` tiers 3-5 — the rail-overlay band, the floating chrome
//! (file pill + canvas action group), and Preview (Play) mode.
//!
//! Tier 3 covers surfaces that float over the rails: the image-fill
//! popover, the StatusBar controls, the panel-resize gutter, and the slice
//! of the chat model picker that can lift over the floating chrome.
//! Several of these are gated on `in_git_panel` / `in_chat_model_picker`
//! because the Git panel and the picker paint above them.
//!
//! Tier 5 must stay AFTER the chrome tier: the Play/Stop button lives in
//! the action group, and every other press while previewing routes into
//! the runtime.

use super::press_ctx::PressCtx;
use super::{PanelResize, PanelResizeKind, WidgetHostNative};
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::widgets::press_flow::{self, ChromePress};
use de_editor_ui::Point2D;

impl WidgetHostNative {
    /// `None` — no rail overlay claimed the press.
    pub(in crate::widget_host) fn press_rail_overlay_tiers(
        &mut self,
        ctx: &PressCtx,
    ) -> Option<bool> {
        let (x, y) = (ctx.x, ctx.y);
        let viewport_width = ctx.viewport_width;
        let viewport_height = ctx.viewport_height;
        let in_git_panel = ctx.in_git_panel;
        let in_chat_model_picker = ctx.in_chat_model_picker;
        // Presenting hides every surface this tier owns — the StatusBar, the
        // rail-resize gutters, the property-panel popovers (`paint.rs`). A
        // hidden widget must not keep claiming presses, or the deck would
        // have dead patches where the chrome used to be; falling through
        // hands the press to the preview tier, which advances the slide.
        if self.preview_slideshow_active() {
            return None;
        }
        // 0a1. Image-fill popover. Property overlays are painted after the
        // VariablesPanel, chat, StatusBar, marquee, and rail chrome, so the
        // visible popup must own their overlap before those lower surfaces.
        // Git and the modal/menu overlays above already had first refusal.
        if !in_git_panel
            && self.dismiss_image_fill_popover_on_press(x, y, viewport_width, viewport_height)
        {
            return Some(true);
        }

        // StatusBar controls — Search frames content, `[-]` / `[+]`
        // step the zoom. The bar paints above the canvas / toolbar but below
        // the late PropertyPanel overlay pass.
        if let Some(r) = self.status_bar_rect(viewport_width, viewport_height) {
            use de_editor_core::StatusBarButton;
            let bar = de_editor_ui::widgets::StatusBar::for_editor(&self.editor_state);
            if let Some(btn) = bar.control_at(r, Point2D::new(x, y)) {
                self.editor_state.editor_ui.pressed_button =
                    Some(de_editor_core::ButtonPressTarget::StatusBar(btn));
                match btn {
                    StatusBarButton::ZoomToFit => self.zoom_to_fit(viewport_width, viewport_height),
                    StatusBarButton::ZoomOut => {
                        self.status_bar_zoom(false, viewport_width, viewport_height)
                    }
                    StatusBarButton::ZoomIn => {
                        self.status_bar_zoom(true, viewport_width, viewport_height)
                    }
                }
                self.mark_dirty();
                return Some(true);
            }
        }

        // 0z. Panel-resize gutter — ±4 px from rail edges. The gutter is
        // lower than the floating image-fill card even where their bounds
        // overlap, and it must stay ABOVE every left-rail surface: half of
        // its band lies over the rail's own content, and a rail that
        // claimed those pixels first could only ever be dragged wider.
        if !in_git_panel && !in_chat_model_picker {
            if let Some(kind) = self.panel_resize_hover(x, y, viewport_width) {
                let start_width = match kind {
                    PanelResizeKind::LayerRight => self.editor_state.editor_ui.layer_panel_width,
                    PanelResizeKind::PropertyLeft => {
                        self.editor_state.editor_ui.property_panel_width
                    }
                };
                self.panel_resize = Some(PanelResize {
                    kind,
                    start_x: x,
                    start_width,
                });
                return Some(true);
            }
        }

        // Chat paints after the floating chrome. A short/top-anchored chat
        // can lift the upward model dropdown across the chrome, so its
        // visible overlap must be routed before the lower chrome surface.
        // Other dropdowns/modals above chat already had first refusal in the
        // blocks above.
        if canvas_geometry::over_chrome(
            &self.editor_state,
            viewport_width,
            viewport_height,
            Point2D::new(x, y),
        ) && !in_git_panel
            && self.apply_chat_model_picker_overlay_press(x, y, viewport_width, viewport_height)
        {
            return Some(true);
        }
        None
    }

    /// Floating chrome (file pill + canvas action group) and the
    /// blank-press fall-through for their padding. `None` — the point is
    /// off the chrome entirely.
    pub(in crate::widget_host) fn press_chrome_tier(&mut self, ctx: &PressCtx) -> Option<bool> {
        let point = Point2D::new(ctx.x, ctx.y);
        let viewport_width = ctx.viewport_width;
        let viewport_height = ctx.viewport_height;
        self.refresh_layout_scene();
        if let Some(hit) = canvas_geometry::chrome_hit_at(
            &self.editor_state,
            viewport_width,
            viewport_height,
            point,
        ) {
            self.close_image_popovers_for_higher_overlay();
            self.editor_state.editor_ui.pressed_button =
                Some(de_editor_core::ButtonPressTarget::Chrome(hit.button()));
            // Arms whose behaviour is identical on both hosts live in the
            // shared flow; only Preview falls through.
            match press_flow::apply_shared_chrome_hit(&mut self.editor_state, hit, self.now_ms) {
                ChromePress::Handled => {}
                ChromePress::SettingsOpened => {
                    // The modal paints above the inspector: release its
                    // whole input family first.
                    self.release_property_keyboard_owner();
                }
                ChromePress::FileMenuToggled => {
                    self.clear_layer_panel_hover();
                }
                ChromePress::Platform => {
                    // Enter / exit canvas Preview (Play) mode. Layout is
                    // solved per-root from the document; the canvas region
                    // is passed only for API compatibility.
                    let (_cx0, _cy0, cw, ch) = self.canvas_region(viewport_width, viewport_height);
                    self.toggle_preview((cw, ch));
                    return Some(true);
                }
            }
            self.mark_dirty();
            return Some(true);
        }
        if canvas_geometry::over_chrome(&self.editor_state, viewport_width, viewport_height, point)
        {
            // The chrome's padding eats clicks but doesn't act — still a
            // blank press, so every text input blurs.
            let image_closed = self.close_image_popovers_for_higher_overlay();
            let blurred = self.blur_text_inputs_on_blank_press();
            return Some(
                image_closed || blurred || ctx.rename_committed || ctx.text_edit_committed,
            );
        }
        None
    }

    /// Open / close the Git panel (File ▸ Git). Mirrors `main.rs`
    /// `A::ToggleGitPanel` bookkeeping; the binary's per-frame `if
    /// git_panel.open { refresh }` performs the actual repo scan.
    pub(in crate::widget_host) fn toggle_git_panel(&mut self) {
        let panel = &mut self.editor_state.editor_ui.git_panel;
        let opening = !panel.open;
        panel.open = opening;
        if opening {
            panel.loading = true;
        } else {
            panel.defocus_commit_input(self.now_ms);
            panel.remote_focused = false;
            panel.https_focused = false;
            panel.diff = None;
            panel.merge_resolve = None;
            // Close the clone wizard synchronously so a rapid close→reopen
            // can't resurface a stale form before the host's
            // `poll_git_clone_job` runs (it then abandons any in-flight job —
            // `cloning` form gone).
            panel.clone_form = None;
        }
    }

    /// Preview (Play) mode swallows every canvas press. `None` — not
    /// previewing.
    pub(in crate::widget_host) fn press_preview_tier(&mut self, ctx: &PressCtx) -> Option<bool> {
        let (x, y) = (ctx.x, ctx.y);
        let viewport_width = ctx.viewport_width;
        let viewport_height = ctx.viewport_height;
        // 0b'. Preview (Play) mode — the canvas belongs to the live
        // runtime, not the editor. The chrome's Play/Stop button was
        // already handled above; any other press routes into the
        // runtime (taps on switches / buttons, caret placement) and is
        // swallowed so no editor selection / node-creation fires.
        if self.preview.is_some() {
            // Presenting a deck owns the whole canvas: the toolbar first,
            // then the board, which advances the deck on release. Neither
            // switcher paints while presenting, and the runtime is not fed —
            // a slide is being shown, not filled in.
            if self.preview_slideshow_active() {
                if !self.slideshow_toolbar_press(x, y, viewport_width, viewport_height) {
                    self.slideshow_board_press(x, y);
                }
                return Some(true);
            }
            if self.screen_switcher_press(x, y, viewport_width, viewport_height) {
                return Some(true);
            }
            if self.preview_switcher_press(x, y, viewport_width, viewport_height) {
                return Some(true);
            }
            self.preview_dispatch_press(x, y, viewport_width, viewport_height);
            return Some(true);
        }
        None
    }
}
