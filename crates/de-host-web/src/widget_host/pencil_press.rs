//! Pencil (freehand) tool arms for the web host (#88) — the native twin is
//! `de-host-native/src/widget_host/pencil_press.rs`. The stroke state and
//! the curve fit live in `de_editor_core::pencil`; these arms only feed it
//! canvas points.

use super::WidgetHost;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::Point2D;

impl WidgetHost {
    /// Canvas press with the Pencil active — always consumes.
    pub(in crate::widget_host) fn apply_pencil_press(&mut self, doc_point: Point2D) -> bool {
        self.editor_state.begin_pencil_stroke(doc_point);
        self.mark_dirty();
        true
    }

    /// Cursor move while a stroke is being drawn. `None` when no stroke
    /// runs, so the move falls through to the other drag paths.
    pub(in crate::widget_host) fn apply_pencil_cursor_move(
        &mut self,
        x: f32,
        y: f32,
    ) -> Option<bool> {
        if !self.editor_state.pencil_stroke_active() {
            return None;
        }
        let doc = canvas_geometry::canvas_doc_point_unclamped(&self.editor_state, x, y);
        if self.editor_state.extend_pencil_stroke(doc) {
            self.mark_dirty();
        }
        Some(true)
    }

    /// Release ends the stroke and commits the fitted path (one undo
    /// step). False when no stroke was running.
    pub(in crate::widget_host) fn apply_pencil_release(&mut self) -> bool {
        if !self.editor_state.pencil_stroke_active() {
            return false;
        }
        let _ = self
            .editor_state
            .finish_pencil_stroke(&mut self.next_node_id);
        self.mark_dirty();
        true
    }
}
