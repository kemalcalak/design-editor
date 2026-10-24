//! Web arms of the shader editing flows (#164): the floating source
//! editor's press / wheel / keys / paint and the uniform slider drag. The
//! flows are shared (`de_editor_ui::widgets::shader_source_editor`); the
//! SkSL compile check runs in the CanvasKit frame (`canvaskit/inner.rs`),
//! which owns the compiler.

use super::WidgetHost;
use de_editor_ui::widgets::shader_source_editor as editor;
use de_editor_ui::widgets::{PaintCx, PropertyPanel};
use de_editor_ui::{Point2D, RenderBackend};

impl WidgetHost {
    fn shader_editor_layout(
        &self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> editor::EditorLayout {
        editor::layout(
            de_editor_ui::widgets::host_canvas_geometry::property_panel_rect(
                &self.editor_state,
                viewport_width,
                viewport_height,
            ),
            viewport_width,
            viewport_height,
        )
    }

    /// The source editor's press tier. `None` when it is closed or the
    /// press is outside it.
    pub(in crate::widget_host) fn press_shader_source_editor(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<bool> {
        self.editor_state.editor_ui.shader_editing.editor.as_ref()?;
        if self.preview_slideshow_active() {
            return None;
        }
        let layout = self.shader_editor_layout(viewport_width, viewport_height);
        let consumed = editor::apply_press(
            &mut self.editor_state,
            &layout,
            Point2D::new(x, y),
            false,
            self.now_ms,
        );
        if consumed.is_some() {
            self.commit_property_focus_if_any();
            self.mark_dirty();
        }
        consumed
    }

    /// Wheel / trackpad pan over the open source editor scrolls its code.
    pub(in crate::widget_host) fn wheel_shader_source_editor(
        &mut self,
        x: f32,
        y: f32,
        dy: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if self.editor_state.editor_ui.shader_editing.editor.is_none() {
            return false;
        }
        let layout = self.shader_editor_layout(viewport_width, viewport_height);
        let consumed = editor::apply_wheel(&mut self.editor_state, &layout, Point2D::new(x, y), dy);
        if consumed {
            self.mark_dirty();
        }
        consumed
    }

    pub(in crate::widget_host) fn paint_shader_source_editor(
        &mut self,
        backend: &mut dyn RenderBackend,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let Some(open) = self.editor_state.editor_ui.shader_editing.editor.as_ref() else {
            return;
        };
        let layout = self.shader_editor_layout(viewport_width, viewport_height);
        let check = self
            .editor_state
            .editor_ui
            .shader_editing
            .checks
            .status(open.input.text());
        let theme =
            de_editor_ui::widgets::editor_state_ext::theme_for(&self.editor_state.editor_ui);
        let mut cx = PaintCx { backend };
        editor::paint(
            &mut cx,
            &theme,
            self.editor_state.editor_ui.locale,
            &layout,
            open,
            &check,
            self.now_ms,
        );
    }

    /// Whether the source editor owns the keyboard.
    pub fn shader_source_editor_focused(&self) -> bool {
        de_editor_core::shader_source_editor::owns_keyboard(&self.editor_state)
    }

    /// Arrows and Tab while the source editor owns the keyboard; always
    /// consumed then, so none reaches the canvas.
    pub fn apply_shader_editor_key(&mut self, key: &str, extend: bool) -> bool {
        use de_editor_core::shader_source_editor as keys;
        let state = &mut self.editor_state;
        let now = self.now_ms;
        let handled = match key {
            "ArrowLeft" => keys::move_caret(state, false, extend, now),
            "ArrowRight" => keys::move_caret(state, true, extend, now),
            "ArrowUp" => keys::move_line(state, false, extend, now),
            "ArrowDown" => keys::move_line(state, true, extend, now),
            "Tab" => keys::text(state, '\t', now),
            _ => None,
        };
        if handled.is_some() {
            self.mark_dirty();
        }
        handled.is_some()
    }

    /// A live uniform slider drag follows the pointer; `probe` is the
    /// cursor event's shared panel snapshot. `true` when it wrote.
    pub(in crate::widget_host) fn drag_shader_uniform(
        &mut self,
        x: f32,
        probe: &mut Option<Option<PropertyPanel>>,
    ) -> bool {
        let Some((target, index)) = self.editor_state.editor_ui.shader_editing.uniform_drag else {
            return false;
        };
        if probe.is_none() {
            *probe = Some(PropertyPanel::for_selection(&self.editor_state));
        }
        let property_rect = de_editor_ui::widgets::host_canvas_geometry::property_panel_rect(
            &self.editor_state,
            self.last_viewport_w,
            self.last_viewport_h,
        );
        let Some(action) = probe
            .as_ref()
            .and_then(Option::as_ref)
            .and_then(|panel| panel.shader_uniform_drag_action(property_rect, target, index, x))
        else {
            return false;
        };
        self.apply_property_action(action);
        true
    }

    /// End a uniform slider drag. `true` when one was running.
    pub(in crate::widget_host) fn end_shader_uniform_drag(&mut self) -> bool {
        self.editor_state
            .editor_ui
            .shader_editing
            .uniform_drag
            .take()
            .is_some()
    }
}
