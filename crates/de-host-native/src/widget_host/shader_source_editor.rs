//! Native arms of the shader editing flows (#164): the floating source
//! editor's press / wheel / paint, the uniform slider drag, and the SkSL
//! compile check the panel shows. The flows themselves are shared
//! (`de_editor_ui::widgets::shader_source_editor`,
//! `de_editor_core::shader_check`); this file only supplies the
//! platform's rects and its compiler.

use super::WidgetHostNative;
use crate::backend::NativeFrameBackend;
use de_editor_ui::widgets::shader_source_editor as editor;
use de_editor_ui::widgets::PaintCx;
use de_editor_ui::Point2D;

impl WidgetHostNative {
    fn shader_editor_layout(
        &self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> editor::EditorLayout {
        editor::layout(
            self.property_rect(viewport_width, viewport_height),
            viewport_width,
            viewport_height,
        )
    }

    /// Compile the sources the panel shows that have no result yet, with
    /// the same compiler the canvas renders them with. Run before paint.
    pub(in crate::widget_host) fn refresh_shader_checks(&mut self) {
        de_editor_core::shader_check::refresh(&mut self.editor_state, |sksl| {
            skia_safe::RuntimeEffect::make_for_shader(sksl, None).err()
        });
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

    /// Wheel over the open source editor scrolls its code.
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
        frame: &mut NativeFrameBackend<'_>,
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
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
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

    /// Left / Right in the focused source editor. `false` when it does not
    /// own the keyboard.
    pub fn apply_shader_source_caret(&mut self, forward: bool, extend: bool) -> bool {
        let moved = de_editor_core::shader_source_editor::move_caret(
            &mut self.editor_state,
            forward,
            extend,
            self.now_ms,
        );
        if moved.is_some() {
            self.mark_dirty();
        }
        moved.is_some()
    }

    /// Up / Down in the focused source editor.
    pub fn apply_shader_source_line(&mut self, down: bool, extend: bool) -> bool {
        let moved = de_editor_core::shader_source_editor::move_line(
            &mut self.editor_state,
            down,
            extend,
            self.now_ms,
        );
        if moved.is_some() {
            self.mark_dirty();
        }
        moved.is_some()
    }

    /// Whether the source editor owns the keyboard (the desktop routes
    /// Tab and the arrows to it then).
    pub fn shader_source_editor_focused(&self) -> bool {
        de_editor_core::shader_source_editor::owns_keyboard(&self.editor_state)
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
