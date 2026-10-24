//! Flip horizontal / vertical (#90): toggle `flip_x` / `flip_y` on the
//! selection. Shared by the Position header buttons and Shift+H /
//! Shift+V on both hosts.

use jian_ops_schema::node::PenNode;

use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::walkers::find_node_mut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlipAxis {
    /// Mirror across the vertical axis (`flip_x`).
    Horizontal,
    /// Mirror across the horizontal axis (`flip_y`).
    Vertical,
}

/// Whether `node` is mirrored along `axis`.
pub fn node_flipped(node: &PenNode, axis: FlipAxis) -> bool {
    let base = node.base();
    match axis {
        FlipAxis::Horizontal => base.flip_x == Some(true),
        FlipAxis::Vertical => base.flip_y == Some(true),
    }
}

/// Toggle `axis` on one node. Un-flipping clears the field rather than
/// writing `false`, so a round trip leaves the document as it was.
fn toggle_flip(node: &mut PenNode, axis: FlipAxis) {
    let next = (!node_flipped(node, axis)).then_some(true);
    let base = node.base_mut();
    match axis {
        FlipAxis::Horizontal => base.flip_x = next,
        FlipAxis::Vertical => base.flip_y = next,
    }
}

impl EditorState {
    /// Toggle `axis` on every editable selected node, each mirrored in
    /// place. No history: the panel dispatch snapshots around it and
    /// [`flip_selection`] wraps it for the keyboard.
    pub fn flip_selected(&mut self, axis: FlipAxis) -> bool {
        let ids: Vec<_> = self
            .selection
            .set
            .iter()
            .filter(|id| id.is_real() && self.is_editable(id))
            .cloned()
            .collect();
        let mut changed = false;
        for id in ids {
            if let Some(node) = find_node_mut(self.active_children_mut(), &id) {
                toggle_flip(node, axis);
                changed = true;
            }
        }
        if changed {
            self.mark_document_changed();
        }
        changed
    }
}

/// Shift+H / Shift+V — flip the selection as one undo step.
pub fn flip_selection(state: &mut EditorState, axis: FlipAxis) -> bool {
    if state.selection.is_empty() {
        return false;
    }
    let snap = state.snapshot_for_history();
    if state.flip_selected(axis) {
        state.history_push_past(snap);
        return true;
    }
    false
}

#[cfg(test)]
#[path = "selection_flip_tests.rs"]
mod tests;
