//! [`EditorCommand::CommitBooleanResult`] apply — commit a host-computed
//! path boolean as one undo step.

use std::collections::HashSet;

use super::*;

/// Hands out the one id the command carries. The caller has already
/// checked it is free, so `allocate` only reserves it. The mutator asks
/// for exactly one id; a second request has no id left to give, which
/// `IdAllocError` can only express as exhaustion.
struct CarriedId(Option<NodeId>);

impl IdAllocator for CarriedId {
    fn allocate(&mut self, taken: &mut HashSet<NodeId>) -> Result<NodeId, IdAllocError> {
        let Some(id) = self.0.take() else {
            return Err(IdAllocError::CounterExhausted);
        };
        taken.insert(id.clone());
        Ok(id)
    }
}

impl EditorState {
    /// Apply [`EditorCommand::CommitBooleanResult`]. Validates before any
    /// write: the document must still be at `revision`, `new_id` must be
    /// free and every source must exist on the active page. A live
    /// document that moved on since the MCP snapshot (a source moved or
    /// restyled on the canvas) rejects instead of committing contours
    /// computed from stale geometry.
    pub(super) fn commit_boolean_result(
        &mut self,
        source_ids: &[NodeId],
        contours: &[Vec<(f64, f64)>],
        new_id: NodeId,
        revision: u64,
    ) -> Result<bool, IdAllocError> {
        if self.revision != revision
            || !new_id.is_real()
            || source_ids.is_empty()
            || self.collect_node_ids().contains(&new_id)
            || source_ids
                .iter()
                .any(|id| find_node(self.active_children(), id).is_none())
        {
            return Ok(false);
        }
        let snap = self.snapshot_for_history();
        let mut allocator = CarriedId(Some(new_id));
        let Some(id) =
            self.replace_paths_with_polyline_with_allocator(source_ids, contours, &mut allocator)?
        else {
            return Ok(false);
        };
        self.history_push_past(snap);
        self.set_single_selection(id);
        Ok(true)
    }
}
