//! Path operations on the active selection (#388): booleans (as boolean
//! groups), Flatten, Outline stroke, Offset path, Simplify and Reverse.
//! The geometry and the document write are `de_path_ops`'; this arm only
//! refreshes the scene they read and repaints.

use super::*;

impl WidgetHostNative {
    /// Combine the selection in a boolean group (Union / Subtract /
    /// Intersect / Exclude), or switch the op of a selected boolean
    /// group. Returns true when it applied.
    pub fn apply_boolean_op(&mut self, op: de_editor_core::BooleanOp) -> bool {
        self.apply_path_operation(de_editor_core::path_ops::PathOperation::Boolean(op))
    }

    /// Run a path operation on the selection. True when the document
    /// changed or the op was refused with a notice; either way the
    /// shortcut was handled and a repaint is owed.
    pub fn apply_path_operation(&mut self, op: de_editor_core::path_ops::PathOperation) -> bool {
        // Path ops mutate the document: land a pending variable-row
        // edit first, then read geometry from the current layout.
        self.commit_variable_row_focus_if_any();
        self.refresh_layout_scene();
        let applied = de_path_ops::apply_path_operation(
            &mut self.editor_state,
            &self.layout_scene,
            op,
            &mut self.next_node_id,
            self.now_ms,
        );
        if applied {
            self.mark_dirty();
        }
        applied
    }
}

#[cfg(all(test, feature = "gl-host"))]
#[path = "path_ops_tests.rs"]
mod tests;
