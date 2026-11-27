//! The shape-create drag (#389): the press dropped a node, each move
//! (and each Shift / Alt change) reshapes it through
//! `de_editor_core::draw_constraints`. Only the platform tail lives here.

use super::input::rect_to_doc_rect;
use super::WidgetHostNative;
use de_editor_core::draw_constraints::{
    drawn_shape, snap_line, DrawKind, DrawModifiers, DrawnShape,
};
use de_editor_core::Tool;
use de_editor_ui::{Point2D, Rect};

impl WidgetHostNative {
    /// Reshape the node a create drag is drawing to end at `cursor`
    /// (doc px), under the modifiers held now.
    pub(in crate::widget_host) fn apply_create_drag(&mut self, cursor: Point2D) {
        let Some(drag) = self.create_drag.as_mut() else {
            return;
        };
        drag.cursor_doc = Some(cursor);
        let snap = self.editor_state.pixel_snap_active();
        let mut start = (f64::from(drag.start_doc_x), f64::from(drag.start_doc_y));
        if snap {
            // Snap the start first, so the constrained shape built from it
            // stays square / on 45° once its far edges round too.
            start = (start.0.round(), start.1.round());
        }
        let tool = self.editor_state.tool;
        if tool.keeps_default_size() {
            // A note keeps the size it was dropped at.
            return;
        }
        let mods = DrawModifiers {
            shift: self.shift_held,
            alt: self.alt_held,
        };
        let shape = drawn_shape(
            DrawKind::for_tool(tool),
            start,
            (f64::from(cursor.x), f64::from(cursor.y)),
            mods,
        );
        // A reversed line's scene data is built from its direction, which
        // a bounds patch does not carry, so it rebuilds. The patch check
        // reads the line as it was, so the new end decides here.
        let reversed = matches!(shape, DrawnShape::Line { dx, dy, .. } if dx < 0.0 || dy < 0.0);
        let patch_scene = !reversed && self.prepare_live_bounds_update();
        match shape {
            DrawnShape::Line { x, y, dx, dy } => {
                let (x, y, dx, dy) = if snap {
                    snap_line(x, y, dx, dy, mods.shift)
                } else {
                    (x, y, dx, dy)
                };
                self.editor_state.set_selected_line(x, y, dx, dy);
                let bounds = Rect::xywh(
                    x.min(x + dx) as f32,
                    y.min(y + dy) as f32,
                    dx.abs() as f32,
                    dy.abs() as f32,
                );
                self.finish_live_bounds_update(bounds, patch_scene);
            }
            DrawnShape::Box { x, y, w, h } => {
                // Text needs room for placeholder glyphs.
                let (min_w, min_h) = match tool {
                    Tool::Text => (
                        de_editor_core::DEFAULT_TEXT_NODE_WIDTH as f64,
                        de_editor_core::DEFAULT_TEXT_NODE_HEIGHT as f64,
                    ),
                    _ => (1.0, 1.0),
                };
                let (mut x, mut y, mut w, mut h) = (x, y, w.max(min_w), h.max(min_h));
                if snap {
                    (x, y, w, h) = de_editor_core::pixel_grid::snap_rect_edges(x, y, w, h);
                }
                let bounds = Rect::xywh(x as f32, y as f32, w as f32, h as f32);
                self.editor_state
                    .set_selected_bounds(rect_to_doc_rect(bounds));
                self.finish_live_bounds_update(bounds, patch_scene);
            }
        }
    }

    /// Redraw an in-flight create drag after a Shift / Alt change; true
    /// when there was one to redraw.
    pub(in crate::widget_host) fn refresh_create_drag(&mut self) -> bool {
        let Some(cursor) = self.create_drag.and_then(|drag| drag.cursor_doc) else {
            return false;
        };
        self.apply_create_drag(cursor);
        self.mark_dirty();
        true
    }
}
