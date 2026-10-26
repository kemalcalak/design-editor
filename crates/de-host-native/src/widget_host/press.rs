//! Mouse-press dispatcher.
//!
//! `EditorState` is the host's source of truth. Canvas hit-tests run
//! against the layout-resolved `LayoutScene` (refreshed at the top
//! of `apply_press`); the resolved-scene node ids are wrapped into
//! de-editor-core `NodeId`s before feeding mutators. Press-helper
//! methods (`create_node_for_active_tool`,
//! `dispatch_agent_settings_press`) live in `press_helpers.rs`.
//!
//! The `apply_press` tier bodies live in the `press_*_tiers.rs` siblings;
//! `press_ctx.rs` carries the per-event state they share.

use super::press_ctx::PressCtx;
use super::WidgetHostNative;
use de_editor_ui::widgets::press_flow::{
    self, LayerContextMenuPress, LayerContextStep, PropertyOverlayPress,
};
use de_editor_ui::Point2D;

impl WidgetHostNative {
    // `pub(in crate::widget_host)` so the instance-panel tests can
    // drive the context-menu rows without simulating menu geometry.
    pub(in crate::widget_host) fn dispatch_layer_context_action(
        &mut self,
        action: de_editor_ui::widgets::layer_context_menu::LayerContextAction,
        target: de_editor_core::ui_draft::LayerContextTarget,
    ) {
        match press_flow::apply_layer_context_action(
            &mut self.editor_state,
            &mut self.next_node_id,
            action,
            target,
            self.now_ms,
        ) {
            LayerContextStep::Done => {}
            LayerContextStep::Group => {
                let _ = self.apply_group();
            }
            LayerContextStep::Boolean(op) => {
                #[cfg(feature = "gl-host")]
                let _ = self.apply_boolean_op(op);
                #[cfg(not(feature = "gl-host"))]
                let _ = op;
            }
            LayerContextStep::Refit => {
                self.fit_active_page_after_switch(self.last_viewport_w, self.last_viewport_h);
            }
        }
        self.mark_dirty();
    }

    /// Platform tail for a press routed to an open property-panel
    /// popover (`press_flow::press_*`). Every outcome consumes the
    /// press.
    pub(in crate::widget_host) fn finish_property_overlay_press(
        &mut self,
        press: PropertyOverlayPress,
    ) -> bool {
        match press {
            PropertyOverlayPress::Action(action) => self.apply_property_action(action),
            PropertyOverlayPress::Swallow => {}
            PropertyOverlayPress::Dismissed => self.mark_dirty(),
        }
        true
    }

    /// Right-click handler — opens the LayerPanel context menu on
    /// a layer row OR page row.
    pub fn apply_right_press(&mut self, x: f32, y: f32, viewport_w: f32, viewport_h: f32) -> bool {
        let cancelled = self.cancel_pointer_gestures();
        if self.editor_state.editor_ui.agent_settings_open {
            return true;
        }
        // Codex stop-gate: right-click outside the variables panel
        // must commit any pending row focus first.
        self.commit_variable_row_focus_if_any();
        self.close_image_popovers_for_higher_overlay();
        // Any top-most floating panel swallows a right-click on its
        // rect — no context menu opens under them.
        if self.over_topmost_panel(x, y, viewport_w, viewport_h) {
            return true;
        }
        // The model picker can extend across the LayerPanel. A secondary
        // press on its visible card belongs to that floating surface and must
        // never open a context menu for the covered layer/page underneath.
        if self
            .chat_model_picker_rect(viewport_w, viewport_h)
            .is_some_and(|rect| rect.contains(Point2D::new(x, y)))
        {
            return true;
        }
        // Select-tool right-click on a path anchor / handle opens the
        // point-type menu (`pen_press.rs`).
        if self.try_open_path_anchor_menu(x, y, viewport_w, viewport_h) {
            return true;
        }
        // Right-click on the tool rail's Rectangle slot opens the shape menu.
        if press_flow::press_toolbar_secondary(
            &mut self.editor_state,
            viewport_w,
            viewport_h,
            Point2D::new(x, y),
        ) {
            self.mark_dirty();
            return true;
        }
        if !self.layers_panel_visible() {
            // No layer panel to hit — the right press is blank chrome;
            // blur inputs like the sidebar-open fall-through below.
            return self.blur_text_inputs_on_blank_press() || cancelled;
        }
        let layer_rect = self.layers_content_rect(viewport_h);
        let panel = self.layer_panel();
        let hit = panel.hit_test(layer_rect, Point2D::new(x, y));
        match press_flow::open_layer_context_menu(&mut self.editor_state, hit, x, y) {
            LayerContextMenuPress::Opened | LayerContextMenuPress::Dismissed => true,
            // Right-press on blank chrome — blur inputs like a left press.
            LayerContextMenuPress::Missed => self.blur_text_inputs_on_blank_press() || cancelled,
        }
    }

    /// Mouse-press handler. Returns whether anything visible changed.
    ///
    /// A strictly ordered hit-test ladder: overlays before panels before
    /// canvas (see `crates/CLAUDE.md`). Each tier helper returns
    /// `Option<bool>` — `None` means "declined, fall through to the next
    /// tier", `Some(dirty)` means "claimed the press, and this is the
    /// repaint signal".
    ///
    /// THE CALL ORDER BELOW *IS* THE BEHAVIOUR. The tier bodies live in
    /// the `press_*_tiers.rs` siblings only to respect the per-file line
    /// cap; reordering these calls changes which surface owns a click.
    pub fn apply_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        self.apply_press_inner(x, y, viewport_width, viewport_height)
    }

    /// Timestamped press variant: `t_ms` is the event's factual
    /// monotonic timestamp. The ordinary tier ladder runs
    /// byte-identically; only the live-preview pointer path
    /// (`preview_dispatch_press` and anything it cancels) stamps
    /// `PointerEvent.t_ms` with `t_ms` instead of the host's global
    /// clock. The global clock is NOT advanced here — callers advance
    /// it separately (monotonically) via [`Self::set_now_ms`], so an
    /// out-of-order event keeps its factual time without regressing
    /// the clock. The scoped context is restored afterwards; a panic
    /// inside the ladder poisons the engine at the ABI boundary, at
    /// which point no further event can ever observe a stale ticket.
    pub fn apply_press_at(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
        t_ms: u64,
    ) -> bool {
        let previous = self.preview_event_time_ms.replace(t_ms);
        let changed = self.apply_press_inner(x, y, viewport_width, viewport_height);
        self.preview_event_time_ms = previous;
        changed
    }

    fn apply_press_inner(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        self.last_viewport_w = viewport_width;
        self.last_viewport_h = viewport_height;
        // A press anywhere takes the keyboard from the shader source editor;
        // a press inside it (its tier) gives it back. Higher tiers (the
        // colour picker's fields, modals) would otherwise share keys with it.
        de_editor_core::shader_source_editor::blur(&mut self.editor_state);
        // Home is a full-surface first-launch entry and owns every pointer
        // event before document chrome or canvas tiers can see it — but the
        // overlays Home itself opens (agent settings, the language menu, the
        // Home-anchored model picker) paint ABOVE the takeover, so their
        // presses must arrive first. The update modal (#111) sits above all
        // of them.
        if let Some(consumed) = self.press_update_modal(x, y, viewport_width, viewport_height) {
            return consumed;
        }
        // "What's new" (#251) opens at launch over Home, under the update modal.
        if let Some(consumed) = self.press_whats_new(x, y, viewport_width, viewport_height) {
            return consumed;
        }
        if let Some(consumed) = self.press_home_overlays(x, y, viewport_width, viewport_height) {
            return consumed;
        }
        if let Some(consumed) = self.press_home(x, y, viewport_width, viewport_height) {
            return consumed;
        }
        // Blur-commit rename + text-edit; track to repaint on click.
        let rename_committed =
            self.editor_state.ui.layer_rename.is_some() && self.editor_state.rename_commit();
        let text_edit_was_active = self.editor_state.ui.text_editing.is_some();
        // EXCEPTION to commit-on-press: a press INSIDE the edited
        // text node places the caret instead of committing (TS
        // textarea parity — a click inside the overlay moves the
        // caret; only an outside click blurs + commits). The probe
        // returns `None` whenever any overlay / modal owns the point,
        // so those presses still commit + route normally.
        let text_edit_caret_press =
            self.text_edit_press_offset(x, y, viewport_width, viewport_height);
        let text_edit_committed =
            text_edit_caret_press.is_none() && self.editor_state.text_edit_commit();
        if rename_committed || text_edit_committed {
            self.mark_dirty();
        }
        if let Some(offset) = text_edit_caret_press {
            self.place_text_edit_caret(offset);
            return true;
        }
        let mut ctx = PressCtx {
            x,
            y,
            viewport_width,
            viewport_height,
            rename_committed,
            text_edit_was_active,
            text_edit_committed,
            // Resolved below, at the exact point the flat ladder did.
            in_git_panel: false,
            in_chat_model_picker: false,
        };
        // Tier 1 — top-most modals / floating panels / context menus.
        if let Some(consumed) = self.press_topmost_overlay_tiers(&ctx) {
            return consumed;
        }
        // 0aa. Commit-on-blur for property-panel inputs +
        // variable-row inline editor.
        self.commit_variable_row_focus_if_any();
        if self.editor_state.ui.property_focus.is_some() {
            // A press anywhere off the properties card commits (#86: the
            // card floats, so "left of its edge" is no longer the test).
            let on_card = self.editor_state.property_panel_visible()
                && self
                    .property_rect(viewport_width, viewport_height)
                    .contains(Point2D::new(x, y));
            if !on_card {
                self.commit_property_focus_if_any();
            }
        }

        // The floating Git panel paints on top of the right-rail
        // panels (see `paint.rs` §8.2) — and in diff mode it widens
        // to 620 px, which overlaps the property / variables rail
        // (and its resize gutter) on a narrow window. Hit-test order
        // must mirror paint Z-order: a click inside the Git-panel
        // rect belongs to the Git panel (tier 9 below), so every
        // rail tier in between skips a click it would otherwise own.
        ctx.in_git_panel = self
            .git_panel_outer_rect(viewport_width, viewport_height)
            .is_some_and(|r| (r).contains(Point2D::new(x, y)));
        ctx.in_chat_model_picker = self
            .chat_model_picker_rect(viewport_width, viewport_height)
            .is_some_and(|r| r.contains(Point2D::new(x, y)));

        // Tier 2 — shape picker, file / export / figma modals, import +
        // locale dropdowns.
        if let Some(consumed) = self.press_menu_modal_tiers(&ctx) {
            return consumed;
        }
        // Tier 3 — image-fill popover, StatusBar, resize gutter, and the
        // model-picker slice that lifts over the floating chrome.
        if let Some(consumed) = self.press_rail_overlay_tiers(&ctx) {
            return consumed;
        }
        // Tier 3c — the generation workspace's chrome (header, toolbar,
        // deck strip, dock handle) claims presses ahead of the floating chrome
        // and rails, after Home and the modal tiers. Presses on the
        // docked canvas and the pinned chat fall through to their
        // ordinary tiers below.
        if !self.preview_slideshow_active() {
            if let Some(consumed) = self.press_workspace(x, y, viewport_width, viewport_height) {
                return consumed;
            }
        }
        // Tier 4 — floating chrome (and its blank-press padding).
        if let Some(consumed) = self.press_chrome_tier(&ctx) {
            return consumed;
        }
        // Tier 5 — Preview (Play) mode swallows everything else.
        if let Some(consumed) = self.press_preview_tier(&ctx) {
            return consumed;
        }
        // Tier 6 — property-panel popovers.
        if let Some(consumed) = self.press_property_overlay_tiers(&ctx) {
            return consumed;
        }
        // Tier 7 — model picker, theme presets, VariablesPanel.
        if let Some(consumed) = self.press_panel_dispatch_tiers(&ctx) {
            return consumed;
        }
        // Tier 8 — PropertyPanel input row.
        if let Some(consumed) = self.press_property_panel_tier(&ctx) {
            return consumed;
        }
        // Tier 9 — floating Git panel, then the AI chat panel.
        if let Some(consumed) = self.press_git_and_chat_tiers(&ctx) {
            return consumed;
        }
        // Tier 10 — toolbar + floating align toolbar.
        if let Some(consumed) = self.press_toolbar_tiers(&ctx) {
            return consumed;
        }
        // Tier 12 — LayerPanel drag peek, then `apply_click`.
        if let Some(consumed) = self.press_layer_and_click_tiers(&ctx) {
            return consumed;
        }
        // Tier 14 — the canvas, branching on the active tool.
        if let Some(consumed) = self.press_canvas_tier(&ctx) {
            return consumed;
        }
        // Final fall-through — the press hit no interactive chrome
        // (panel-rail gaps, property-panel padding, …): blank press.
        let blurred = self.blur_text_inputs_on_blank_press();
        blurred || rename_committed || text_edit_committed
    }
}
