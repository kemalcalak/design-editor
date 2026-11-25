//! Vector edit mode (#387): entering and leaving the mode, and the keys
//! it owns (Enter / Esc, Delete / Backspace, arrows, Cmd+A, Cmd+J, Q).
//! The press / drag arms live in `vector_edit_press.rs`.

use super::WidgetHostNative;
use de_editor_core::editor_toast::EditorToastLevel;
use de_editor_core::vector_edit::PointRef;
use de_editor_core::{NodeId, Tool};
use de_editor_ui::widgets::vector_edit_view::{VectorEditView, VectorHit};

impl WidgetHostNative {
    /// Whether vector edit mode runs (the desktop key table routes
    /// Cmd+J and Q by it).
    pub fn vector_edit_active(&self) -> bool {
        self.editor_state.vector_edit_active()
    }

    /// Push the Cmd (macOS) / Ctrl state: held, it suspends snapping.
    pub fn set_modifier_cmd(&mut self, held: bool) {
        self.cmd_held = held;
    }

    /// Enter: leave the mode, or enter it on the single selected path
    /// (Select tool, nothing else owning the key).
    pub(in crate::widget_host) fn apply_vector_edit_enter(&mut self) -> bool {
        self.editor_state.prune_vector_edit();
        // A focused chat composer keeps Enter for sending.
        if self.editor_state.chat.focused {
            return false;
        }
        if self.editor_state.vector_edit_active() {
            self.editor_state.exit_vector_edit();
            self.mark_dirty();
            return true;
        }
        if !matches!(self.editor_state.tool, Tool::Select)
            || self.editor_state.selection_count() != 1
        {
            return false;
        }
        let id = self.editor_state.selection.anchor.clone();
        if !self.editor_state.enter_vector_edit(&id) {
            return false;
        }
        self.mark_dirty();
        true
    }

    /// Escape: stop the pen first, then leave the mode.
    pub(in crate::widget_host) fn apply_vector_edit_escape(&mut self) -> bool {
        self.editor_state.prune_vector_edit();
        if !self.editor_state.vector_edit_active() {
            return false;
        }
        let drawing = self
            .editor_state
            .ui
            .vector_edit
            .as_ref()
            .is_some_and(|s| s.pen_continue.is_some());
        if drawing {
            if let Some(s) = self.editor_state.ui.vector_edit.as_mut() {
                s.pen_continue = None;
                s.pen_cursor = None;
            }
        } else {
            self.editor_state.exit_vector_edit();
        }
        self.mark_dirty();
        true
    }

    /// Delete / Backspace inside the mode: remove the selected points
    /// (healing the curve). Swallowed even with nothing selected so the
    /// key never deletes the whole path. `None` outside the mode.
    pub(in crate::widget_host) fn apply_vector_edit_delete(&mut self) -> Option<bool> {
        self.editor_state.prune_vector_edit();
        if !self.editor_state.vector_edit_active() {
            return None;
        }
        if self.editor_state.vector_delete_selected() {
            self.mark_dirty();
        }
        Some(true)
    }

    /// Arrow nudge of the selected points. `None` outside the mode.
    pub(in crate::widget_host) fn apply_vector_edit_nudge(
        &mut self,
        dx: f32,
        dy: f32,
    ) -> Option<bool> {
        self.editor_state.prune_vector_edit();
        if !self.editor_state.vector_edit_active() {
            return None;
        }
        if self.editor_state.vector_nudge(dx as f64, dy as f64) {
            self.mark_dirty();
        }
        Some(true)
    }

    /// Cmd+A inside the mode selects every point. `None` outside it.
    pub(in crate::widget_host) fn apply_vector_edit_select_all(&mut self) -> Option<bool> {
        self.editor_state.prune_vector_edit();
        if !self.editor_state.vector_edit_active() {
            return None;
        }
        self.editor_state.vector_select_all();
        self.mark_dirty();
        Some(true)
    }

    /// Cmd+J — join the two selected end points. False outside the mode
    /// (the chord keeps its chat toggle there).
    pub fn apply_vector_join(&mut self) -> bool {
        self.editor_state.prune_vector_edit();
        if !self.editor_state.vector_edit_active() {
            return false;
        }
        if self.editor_state.vector_join_selected() {
            self.mark_dirty();
        }
        true
    }

    /// Q — toggle lasso selection for the next drag. False outside the
    /// mode.
    pub fn apply_vector_lasso_toggle(&mut self) -> bool {
        self.editor_state.prune_vector_edit();
        if !self.editor_state.vector_edit_active() {
            return false;
        }
        if let Some(s) = self.editor_state.ui.vector_edit.as_mut() {
            s.lasso = !s.lasso;
        }
        self.mark_dirty();
        true
    }

    /// View ▸ Snap to Pixel Grid.
    pub fn apply_toggle_pixel_grid_snap(&mut self) -> bool {
        let ui = &mut self.editor_state.ui;
        ui.pixel_grid_snap_off = !ui.pixel_grid_snap_off;
        let key = if ui.pixel_grid_snap_off {
            "vectorEdit.pixelSnapOff"
        } else {
            "vectorEdit.pixelSnapOn"
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

    /// Tool switch: Select and Pen keep the mode (the pen stops drawing
    /// when leaving it); any other tool leaves it.
    pub(in crate::widget_host) fn vector_edit_on_tool_switch(&mut self, tool: Tool) {
        if !self.editor_state.vector_edit_active() {
            return;
        }
        match tool {
            Tool::Pen => {}
            Tool::Select => {
                if let Some(s) = self.editor_state.ui.vector_edit.as_mut() {
                    s.pen_continue = None;
                    s.pen_cursor = None;
                }
            }
            _ => {
                self.editor_state.exit_vector_edit();
            }
        }
    }

    /// Double-click on a path (the press's primary target): enter the
    /// mode on it.
    pub(in crate::widget_host) fn try_enter_vector_edit_on_double(&mut self, id: &NodeId) -> bool {
        let is_path = matches!(
            de_editor_core::walkers::find_node(self.editor_state.active_children(), id),
            Some(jian_ops_schema::node::PenNode::Path(_))
        );
        if !is_path || !self.editor_state.enter_vector_edit(id) {
            return false;
        }
        self.mark_dirty();
        true
    }

    /// Pen press on an open end point of the selected path, outside the
    /// mode: enter it and keep drawing from that end. False (and no
    /// state change) otherwise.
    pub(in crate::widget_host) fn try_pen_continue_selected_path(
        &mut self,
        x: f32,
        y: f32,
    ) -> bool {
        if self.editor_state.selection_count() != 1 || self.editor_state.vector_edit_active() {
            return false;
        }
        let id = self.editor_state.selection.anchor.clone();
        let is_open_path = matches!(
            de_editor_core::walkers::find_node(self.editor_state.active_children(), &id),
            Some(jian_ops_schema::node::PenNode::Path(_))
        );
        if !is_open_path || !self.editor_state.enter_vector_edit(&id) {
            return false;
        }
        self.refresh_layout_scene();
        let hit = VectorEditView::build(&self.editor_state, &self.layout_scene).and_then(|view| {
            let doc = de_editor_ui::widgets::host_canvas_geometry::canvas_doc_point_unclamped(
                &self.editor_state,
                x,
                y,
            );
            match view.hit(view.unrotate(doc), self.editor_state.viewport.zoom) {
                Some(VectorHit::Point(p))
                    if view.subs.get(p.sub).is_some_and(|s| s.is_endpoint(p.idx)) =>
                {
                    Some(p)
                }
                _ => None,
            }
        });
        match hit {
            Some(p) => {
                self.editor_state
                    .vector_pen_continue_from(PointRef::new(p.sub, p.idx));
                self.mark_dirty();
                true
            }
            None => {
                self.editor_state.exit_vector_edit();
                false
            }
        }
    }
}
