//! Path boolean ops on the active selection (`apply_boolean_op`).
//!
//! Carved out of the `widget_host.rs` spine as pure code motion to keep
//! the spine under the repo's 800-line cap.

use super::*;

impl WidgetHostNative {
    /// Run a path boolean op on the active selection (Union /
    /// Subtract / Intersect / Exclude). Backed by skia's `Path::op`.
    /// Returns true when the op committed (≥ 2 Path nodes were
    /// selected + the result yielded a non-empty polyline) or was refused
    /// with a notice (a source holds layers): either way the shortcut was
    /// handled and a repaint is owed.
    #[cfg(feature = "gl-host")]
    pub fn apply_boolean_op(&mut self, op: de_editor_core::BooleanOp) -> bool {
        // Codex stop-gate: boolean op shortcuts (Cmd+Alt+U/S/I/X)
        // mutate the document — commit any pending variable-row
        // edit first so the dirty draft lands before this op runs.
        self.commit_variable_row_focus_if_any();
        // The skia `Path::op` math runs against the layout-resolved
        // `LayoutScene` + the editor selection.
        self.refresh_layout_scene();
        // A frame holding layers can't be a source (the commit would drop
        // its subtree): say so rather than doing nothing under the click.
        let source_ids = self.editor_state.selection.set.clone();
        if self
            .editor_state
            .refuse_boolean_on_containers(&source_ids, self.now_ms)
        {
            self.mark_dirty();
            return true;
        }
        let selected: Vec<String> = source_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect();
        let outcome = crate::boolean_ops::compute_boolean_op(&self.layout_scene, &selected, op);
        let Some(result) = outcome else {
            return false;
        };
        // Scene ids are the canonical `.de` ids — wrap straight into
        // `de_editor_core::NodeId`. The commit runs through
        // `EditorCommand::CommitBooleanResult` (the MCP `boolean_op`
        // path), minting the id from this host's counter.
        let source_ids: Vec<de_editor_core::NodeId> = result
            .source_ids
            .iter()
            .map(de_editor_core::NodeId::new)
            .collect();
        let committed = self.editor_state.commit_canvas_boolean(
            source_ids,
            result.contours,
            &mut self.next_node_id,
        );
        if committed {
            self.mark_dirty();
        }
        committed
    }
}

#[cfg(all(test, feature = "gl-host"))]
#[path = "boolean_op_tests.rs"]
mod tests;
