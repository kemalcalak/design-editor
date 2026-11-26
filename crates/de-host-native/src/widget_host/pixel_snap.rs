//! Pixel grid host arms (#399): View ▸ Pixel grid and the node-drag half
//! of Snap to pixel grid. The rounding rules are
//! `de_editor_core::pixel_grid`; resizes and drawing snap in
//! `cursor_move_drags.rs`, vector points in `vector_edit_press.rs`.

use super::WidgetHostNative;
use de_editor_core::editor_toast::EditorToastLevel;
use de_editor_core::pixel_grid::snap_drag_axis;

impl WidgetHostNative {
    /// View ▸ Pixel grid (Shift+'): show or hide the grid at zoom >= 400 %.
    pub fn apply_toggle_pixel_grid(&mut self) -> bool {
        let view = &mut self.editor_state.editor_ui.canvas_view;
        view.pixel_grid_hidden = !view.pixel_grid_hidden;
        let key = if view.pixel_grid_hidden {
            "canvas.pixelGridOff"
        } else {
            "canvas.pixelGridOn"
        };
        self.editor_state.editor_ui.show_toast(
            key,
            Vec::new(),
            EditorToastLevel::Info,
            self.now_ms,
        );
        self.mark_dirty();
        true
    }

    /// After one node-drag step, round the anchor's position to whole
    /// pixels on each axis a smart guide did not just snap (a guide owns
    /// that axis). The whole selection moves by the correction, which is
    /// returned so the caller patches the scene by the same amount.
    pub(in crate::widget_host) fn pixel_snap_node_drag(
        &mut self,
        guided_x: bool,
        guided_y: bool,
    ) -> (f64, f64) {
        if !self.editor_state.pixel_snap_active() {
            self.node_drag_pixel_residual = (0.0, 0.0);
            return (0.0, 0.0);
        }
        let Some((x, y)) = self.editor_state.anchor_authored_position() else {
            return (0.0, 0.0);
        };
        let (rx, ry) = self.node_drag_pixel_residual;
        let (cx, rx) = if guided_x {
            (0.0, 0.0)
        } else {
            snap_drag_axis(x, rx)
        };
        let (cy, ry) = if guided_y {
            (0.0, 0.0)
        } else {
            snap_drag_axis(y, ry)
        };
        self.node_drag_pixel_residual = (rx, ry);
        if (cx != 0.0 || cy != 0.0) && !self.editor_state.translate_selected(cx, cy) {
            return (0.0, 0.0);
        }
        (cx, cy)
    }
}
