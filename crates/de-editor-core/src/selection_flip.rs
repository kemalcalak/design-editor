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
/// writing `false`, so a round trip leaves the document as it was. That
/// is why this does not go through `EditorCommand::SetNodeFlip`, which
/// stores `Some(false)`.
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

impl FlipAxis {
    /// The axis a Shift+letter shortcut flips (`H` / `V`, any case), so
    /// both hosts share one mapping.
    pub fn from_shortcut_key(key: &str) -> Option<Self> {
        if key.eq_ignore_ascii_case("h") {
            Some(Self::Horizontal)
        } else if key.eq_ignore_ascii_case("v") {
            Some(Self::Vertical)
        } else {
            None
        }
    }
}

/// Shift+H / Shift+V — flip the selection as one undo step. Runs inside
/// the anchor's instance-write scope, like the panel dispatch, so a
/// selected instance child flips through its Ref's overrides.
pub fn flip_selection(state: &mut EditorState, axis: FlipAxis) -> bool {
    if state.selection.is_empty() {
        return false;
    }
    let snap = state.snapshot_for_history();
    let scope = state.begin_instance_write_for_anchor();
    state.flip_selected(axis);
    if let Some(scope) = scope {
        state.finish_instance_write(scope);
    }
    if state.snapshot_for_history() != snap {
        state.history_push_past(snap);
        return true;
    }
    false
}

#[cfg(test)]
#[path = "selection_flip_tests.rs"]
mod tests;
