//! Native arm of a browser node's interaction mode (#324, phase 4). The
//! routing is `de_editor_ui::widgets::browser_interact_flow`; this module
//! hands it the host's state and scene, repaints when the mode ends, and
//! lets the desktop app drain the queued page input. The double-click
//! that enters the mode is in `canvas_select_drag.rs`.

use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_ui::widgets::browser_interact_flow::{ButtonOutcome, KeyInput, KeyOutcome};

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Whether a browser node is in interaction mode.
    pub fn browser_interaction_active(&self) -> bool {
        self.editor_state.interactive_browser().is_some()
    }

    /// Whether the pointer is over the interactive node's page.
    pub fn browser_over_page(&mut self, x: f32, y: f32, viewport_w: f32, viewport_h: f32) -> bool {
        if !self.browser_interaction_active() {
            return false;
        }
        self.refresh_layout_scene();
        self.browser_interaction.over_page(
            &self.editor_state,
            &self.layout_scene,
            x,
            y,
            viewport_w,
            viewport_h,
        )
    }

    /// A pointer move; true when the page took it (the canvas must not
    /// hover or drag under it).
    pub fn browser_pointer_move(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> bool {
        if !self.browser_interaction_active() {
            return false;
        }
        self.refresh_layout_scene();
        self.browser_interaction.pointer_move(
            &self.editor_state,
            &self.layout_scene,
            x,
            y,
            viewport_w,
            viewport_h,
            modifiers,
        )
    }

    /// A mouse button. `Left` and `Editor` let the press go on to the
    /// editor's own handling.
    #[allow(clippy::too_many_arguments)]
    pub fn browser_pointer_button(
        &mut self,
        pressed: bool,
        button: MouseButton,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> ButtonOutcome {
        if !self.browser_interaction_active() {
            return ButtonOutcome::Editor;
        }
        self.refresh_layout_scene();
        let outcome = self.browser_interaction.pointer_button(
            &mut self.editor_state,
            &self.layout_scene,
            pressed,
            button,
            x,
            y,
            viewport_w,
            viewport_h,
            modifiers,
            self.now_ms,
        );
        if outcome == ButtonOutcome::Left {
            self.mark_dirty();
        }
        outcome
    }

    /// A wheel or trackpad scroll (`dx` / `dy` as a canvas pan); true
    /// when the page took it instead of the canvas.
    #[allow(clippy::too_many_arguments)]
    pub fn browser_wheel(
        &mut self,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> bool {
        if !self.browser_interaction_active() {
            return false;
        }
        self.refresh_layout_scene();
        self.browser_interaction.wheel(
            &self.editor_state,
            &self.layout_scene,
            x,
            y,
            dx,
            dy,
            viewport_w,
            viewport_h,
            modifiers,
        )
    }

    /// A key press or release while a node may be interactive.
    pub fn browser_key(&mut self, input: KeyInput, pressed: bool) -> KeyOutcome {
        let outcome = self
            .browser_interaction
            .key(&mut self.editor_state, input, pressed);
        if outcome == KeyOutcome::Left {
            self.mark_dirty();
        }
        outcome
    }

    /// Committed IME text; true when the page took it.
    pub fn browser_insert_text(&mut self, text: &str) -> bool {
        self.browser_interaction
            .insert_text(&self.editor_state, text)
    }

    /// The page input queued since the last call, with its node.
    pub fn take_browser_input(&mut self) -> Option<(String, Vec<BrowserInputEvent>)> {
        self.browser_interaction.take()
    }

    /// End interaction mode when its node lost the selection or its page
    /// (once per frame). Returns whether the canvas must repaint.
    pub fn sync_browser_interaction(&mut self) -> bool {
        if !self.browser_interaction.sync(&mut self.editor_state) {
            return false;
        }
        self.mark_dirty();
        true
    }

    /// The window lost focus: release what the page holds.
    pub fn browser_release_pointer(&mut self) {
        self.browser_interaction.release_pointer();
    }

    /// Double-click on a browser node: enter interaction mode. True when
    /// it did (the press is then done).
    pub(in crate::widget_host) fn enter_browser_interaction_at(
        &mut self,
        hit_path: &[de_editor_core::NodeId],
    ) -> bool {
        if !self
            .browser_interaction
            .enter_at(&mut self.editor_state, hit_path)
        {
            return false;
        }
        self.last_hover_probe = None;
        self.mark_dirty();
        true
    }
}
