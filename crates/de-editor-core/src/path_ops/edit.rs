//! Simplify and Reverse direction: in-place edits of a path's geometry,
//! no skia needed. Both read the path as subpaths in its parent frame
//! (`vector_edit::node_subpaths`, the same view vector edit mode uses)
//! and write it back through the same writer.

use crate::node_id::NodeId;
use crate::state::EditorState;
use crate::vector_edit::{
    anchor_count, node_subpaths, reverse_subpath, simplify_subpaths, write_subpaths, Subpath,
};
use crate::walkers::{find_node, find_node_mut};

impl EditorState {
    /// Simplify the path `node_id` within `tolerance` doc px. False when
    /// it is not a path or no anchor could go.
    pub(super) fn simplify_path(&mut self, node_id: &NodeId, tolerance: f64) -> bool {
        let Some(subs) = find_node(self.active_children(), node_id).and_then(node_subpaths) else {
            return false;
        };
        let simplified = simplify_subpaths(&subs, tolerance);
        if anchor_count(&simplified) == anchor_count(&subs) {
            return false;
        }
        self.write_path_geometry(node_id, &simplified)
    }

    /// Reverse every subpath of the path `node_id`. False when it is not
    /// a path.
    pub(super) fn reverse_path(&mut self, node_id: &NodeId) -> bool {
        let Some(mut subs) = find_node(self.active_children(), node_id).and_then(node_subpaths)
        else {
            return false;
        };
        subs.iter_mut().for_each(reverse_subpath);
        self.write_path_geometry(node_id, &subs)
    }

    /// One undo step writing `subs` (parent frame) into the path.
    fn write_path_geometry(&mut self, node_id: &NodeId, subs: &[Subpath]) -> bool {
        let snap = self.snapshot_for_history();
        let written = find_node_mut(self.active_children_mut(), node_id)
            .is_some_and(|node| write_subpaths(node, subs));
        if written {
            self.history_push_past(snap);
        }
        written
    }
}
