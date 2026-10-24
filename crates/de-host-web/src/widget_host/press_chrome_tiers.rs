//! Web `apply_press` tiers 4-6 — the rail-overlay band, the floating
//! VariablesPanel band, and the floating chrome (file pill + canvas action
//! group).
//!
//! Tier 4 covers the image-fill popover, the StatusBar controls, and the
//! slice of the chat model picker that can lift over the chrome. Tier 5
//! is gated on `over_chat_model_picker` so the picker card keeps its
//! overlap with the panels beneath it.

use super::press_ctx::PressCtx;
use super::WidgetHost;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::widgets::press_flow::{self, ChromePress};
use de_editor_ui::widgets::PropertyPanel;
use de_editor_ui::Point2D;

impl WidgetHost {
    /// `None` — no rail overlay claimed the press.
    pub(in crate::widget_host) fn press_rail_overlay_tiers(
        &mut self,
        ctx: &PressCtx,
    ) -> Option<bool> {
        let (x, y) = (ctx.x, ctx.y);
        let viewport_width = ctx.viewport_width;
        let viewport_height = ctx.viewport_height;
        // 0a0. Shader source editor (#164): it floats over the canvas left
        // of the properties card, above every rail surface.
        if let Some(consumed) =
            self.press_shader_source_editor(x, y, viewport_width, viewport_height)
        {
            return Some(consumed);
        }
        // 0a1. Image-fill popover. It is painted in the late property-overlay
        // pass, above VariablesPanel, chat, StatusBar, marquee, and ordinary
        // rail content. The modal/menu overlays above already had first
        // refusal, while an outside press still commits and closes the popup.
        if self.editor_state.editor_ui.image_fill_popover_open {
            if let Some(panel) = PropertyPanel::for_selection(&self.editor_state) {
                let property_rect =
                    de_editor_ui::widgets::host_canvas_geometry::property_panel_rect(
                        &self.editor_state,
                        viewport_width,
                        viewport_height,
                    );
                let point = Point2D::new(x, y);
                if panel.image_fill_popover_contains(property_rect, point) {
                    if let Some(focus) = panel.image_fill_popover_input_at(property_rect, point) {
                        return Some(self.focus_property_input_from_press(
                            focus,
                            property_rect,
                            point,
                        ));
                    }
                    if let Some(action) = panel.hit_test_action(property_rect, point) {
                        self.apply_property_action(action);
                    }
                    return Some(true);
                }
            }
            self.commit_image_tile_scale_focus_if_any();
            self.editor_state.editor_ui.image_fill_popover_open = false;
            self.mark_dirty();
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

        // Chat paints after the floating chrome. In a short maximized
        // viewport the upward model dropdown can cover it, so route that
        // visible slice before the lower chrome surface. Higher
        // dropdowns/modals already ran.
        if canvas_geometry::over_chrome(
            &self.editor_state,
            viewport_width,
            viewport_height,
            Point2D::new(x, y),
        ) && self.apply_chat_model_picker_overlay_press(x, y, viewport_width, viewport_height)
        {
            return Some(true);
        }
        None
    }

    /// Theme-preset dropdown + floating VariablesPanel.
    /// `None` — neither claimed the press.
    pub(in crate::widget_host) fn press_variables_tiers(&mut self, ctx: &PressCtx) -> Option<bool> {
        let (x, y) = (ctx.x, ctx.y);
        let viewport_width = ctx.viewport_width;
        let viewport_height = ctx.viewport_height;
        let over_chat_model_picker = ctx.over_chat_model_picker;
        // 0aa. Theme-preset dropdown (#20) — runs BEFORE the panel dispatch so
        //      the functional menu rows win over the panel's stub
        //      TogglePresetMenu mapping (native parity).
        if !over_chat_model_picker
            && self.dispatch_variables_preset_press(x, y, viewport_width, viewport_height)
        {
            return Some(true);
        }
        // 0ab. Floating VariablesPanel — full interactive grid
        //      mirroring the native host (#21). A press inside the
        //      panel rect dispatches; outside presses fall through to
        //      the normal layers (the panel floats, it isn't modal).
        if !over_chat_model_picker
            && self.dispatch_variables_panel_press(x, y, viewport_width, viewport_height)
        {
            return Some(true);
        }
        None
    }

    /// Floating chrome (file pill + canvas action group) and the
    /// blank-press fall-through for their padding. `None` — the point is
    /// off the chrome entirely. Mirrors the native host.
    pub(in crate::widget_host) fn press_chrome_tier(&mut self, ctx: &PressCtx) -> Option<bool> {
        let point = Point2D::new(ctx.x, ctx.y);
        let viewport_width = ctx.viewport_width;
        let viewport_height = ctx.viewport_height;
        if let Some(hit) = canvas_geometry::chrome_hit_at(
            &self.editor_state,
            viewport_width,
            viewport_height,
            point,
        ) {
            self.close_image_popovers_for_higher_overlay();
            self.commit_property_family_focus_if_any();
            self.editor_state.editor_ui.pressed_button =
                Some(de_editor_core::ButtonPressTarget::Chrome(hit.button()));
            // Arms whose behaviour is identical on both hosts live in the
            // shared flow; only Preview falls through.
            match press_flow::apply_shared_chrome_hit(&mut self.editor_state, hit, self.now_ms) {
                ChromePress::Handled => {}
                // The web host routes keys through the focused DOM input,
                // so there is no keyboard owner to release (cf. native).
                ChromePress::SettingsOpened => {}
                ChromePress::FileMenuToggled => {
                    self.clear_layer_panel_hover();
                }
                ChromePress::Platform => {
                    self.toggle_preview_from_chrome(viewport_width, viewport_height)
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

    /// Enter / exit Preview from the canvas action group's Play button.
    fn toggle_preview_from_chrome(&mut self, viewport_width: f32, viewport_height: f32) {
        if self.preview.is_some() {
            // Exit preview mode. Track M-1: defer the teardown — only close
            // the runtime if there is no device frame.
            self.exit_preview(viewport_width, viewport_height);
            return;
        }
        #[cfg(feature = "canvaskit")]
        {
            let op_ck = self.op_ck.clone();
            let _ =
                self.enter_preview_from_browser(viewport_width, viewport_height, op_ck.as_ref());
        }
        #[cfg(not(feature = "canvaskit"))]
        {
            self.editor_state.editor_ui.preview.mode = false;
            self.editor_state.editor_ui.preview.warnings =
                vec!["preview: not available in this build".to_string()];
        }
    }

    /// Toggle the browser's Fullscreen API (File ▸ Fullscreen). The web host
    /// runs in WASM, so it reaches the document directly — no runner
    /// round-trip and no intent flag (unlike native, where the host can't
    /// reach the winit window).
    pub(in crate::widget_host) fn toggle_browser_fullscreen(&self) {
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if doc.fullscreen_element().is_some() {
                doc.exit_fullscreen();
            } else if let Some(el) = doc.document_element() {
                let _ = el.request_fullscreen();
            }
        }
    }
}
