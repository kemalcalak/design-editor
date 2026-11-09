//! Web arm of a browser node's interaction mode (#324, phase 4), the twin
//! of the native host's `widget_host/browser_interact.rs`. The routing is
//! `de_editor_ui::widgets::browser_interact_flow`; this module hands it the
//! host's state and scene and repaints when the mode ends. The DOM
//! listeners that feed it are `canvaskit/mount_browser_interaction.rs`;
//! the queued input goes to the daemon through `browser_input_post.rs`.
//! The double-click that enters the mode is in `node_drag.rs`.

use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_ui::widgets::browser_interact_flow::{ButtonOutcome, KeyInput, KeyOutcome};

use super::WidgetHost;

/// Where an event went while a node may be interactive. Mirrors the
/// shared flow's outcomes for the DOM glue, which may not name widget
/// types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageRoute {
    /// Not the page's: the editor handles it.
    Editor,
    /// Taken by the page: the editor must not see it.
    Page,
    /// The mode ended (Escape, or a press off the page).
    Left,
}

impl From<ButtonOutcome> for PageRoute {
    fn from(outcome: ButtonOutcome) -> Self {
        match outcome {
            ButtonOutcome::Editor => Self::Editor,
            ButtonOutcome::Page => Self::Page,
            ButtonOutcome::Left => Self::Left,
        }
    }
}

impl From<KeyOutcome> for PageRoute {
    fn from(outcome: KeyOutcome) -> Self {
        match outcome {
            KeyOutcome::Editor => Self::Editor,
            KeyOutcome::Page => Self::Page,
            KeyOutcome::Left => Self::Left,
        }
    }
}

impl WidgetHost {
    /// Whether a browser node is in interaction mode.
    pub(crate) fn browser_interaction_active(&self) -> bool {
        self.editor_state.interactive_browser().is_some()
    }

    /// A pointer move; true when the page took it (the canvas must not
    /// hover or drag under it).
    pub(crate) fn browser_pointer_move(
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
    pub(crate) fn browser_pointer_button(
        &mut self,
        pressed: bool,
        button: MouseButton,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> PageRoute {
        if !self.browser_interaction_active() {
            return PageRoute::Editor;
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
        outcome.into()
    }

    /// A wheel or trackpad scroll (`dx` / `dy` as a canvas pan); true
    /// when the page took it instead of the canvas.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn browser_wheel(
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

    /// A key press or release (DOM `key` / `code`, the text it types).
    pub(crate) fn browser_key(
        &mut self,
        key: String,
        code: String,
        text: Option<String>,
        modifiers: InputModifiers,
        pressed: bool,
    ) -> PageRoute {
        let input = KeyInput {
            key,
            code,
            text,
            modifiers,
        };
        let outcome = self
            .browser_interaction
            .key(&mut self.editor_state, input, pressed);
        if outcome == KeyOutcome::Left {
            self.mark_dirty();
        }
        outcome.into()
    }

    /// Committed IME text; true when the page took it.
    pub(crate) fn browser_insert_text(&mut self, text: &str) -> bool {
        self.browser_interaction
            .insert_text(&self.editor_state, text)
    }

    /// The page input queued since the last call, with its node.
    pub(crate) fn take_browser_input(&mut self) -> Option<(String, Vec<BrowserInputEvent>)> {
        self.browser_interaction.take()
    }

    /// End interaction mode when its node lost the selection or its page
    /// (once per frame). Returns whether the canvas must repaint.
    pub(crate) fn sync_browser_interaction(&mut self) -> bool {
        if !self.browser_interaction.sync(&mut self.editor_state) {
            return false;
        }
        self.mark_dirty();
        true
    }

    /// The tab lost focus: release what the page holds.
    pub(crate) fn browser_release_pointer(&mut self) {
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
        self.mark_dirty();
        true
    }
}

#[cfg(test)]
#[path = "browser_interact_tests.rs"]
mod tests;
