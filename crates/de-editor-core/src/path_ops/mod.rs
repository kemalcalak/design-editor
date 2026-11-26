//! Path operations on the document (#388): boolean groups, committing a
//! host-computed path (boolean flatten, Flatten, Outline stroke, Offset
//! path), Simplify and Reverse direction.
//!
//! The skia math (booleans, stroking, offsetting) runs in the host layer
//! (`de-path-ops`); this module only writes its result, so the core stays
//! skia-free. Every write is one [`EditorCommand::Path`] and one undo
//! step. Boolean groups are `PenNode::Boolean`: the children stay
//! editable and the scene builder paints their combined outline.
//!
//! [`EditorCommand::Path`]: crate::EditorCommand::Path

mod boolean_group;
mod canvas;
mod commit;
mod edit;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_commit;

pub use boolean_group::boolean_group_name;
pub use canvas::CanvasPath;
pub use commit::{PathCommit, PathCommitKind};

use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::node_id::NodeId;
use crate::state::EditorState;
use crate::BooleanOp;

/// A path operation the canvas, the menus and the property panel run on
/// the selection. Hosts with skia compute the geometry ones and write
/// them through [`PathCommand`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathOperation {
    /// Combine the selection in a boolean group, or switch the op of a
    /// selected boolean group.
    Boolean(BooleanOp),
    /// Bake the selection (shapes, groups, a boolean group) into one path.
    Flatten,
    /// Turn each selected node's stroke into a filled path.
    OutlineStroke,
    /// Grow (`delta > 0`) or shrink each selected node's outline.
    Offset { delta: f32, join: OffsetJoin },
    /// Remove redundant anchors of each selected path.
    Simplify,
    /// Reverse each selected path's direction.
    Reverse,
}

/// Corner style of an offset path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OffsetJoin {
    #[default]
    Round,
    Miter,
    Bevel,
}

impl OffsetJoin {
    /// Parse `"round" | "miter" | "bevel"` (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "round" => Some(Self::Round),
            "miter" => Some(Self::Miter),
            "bevel" => Some(Self::Bevel),
            _ => None,
        }
    }
}

/// The Offset path menu step, doc px.
pub const OFFSET_MENU_STEP: f32 = 1.0;

/// Default Simplify tolerance in doc px.
pub const DEFAULT_SIMPLIFY_TOLERANCE: f64 = 0.5;

/// One path-operation write.
#[derive(Debug, Clone, PartialEq)]
pub enum PathCommand {
    /// Replace sources with one host-computed path.
    Commit(Box<PathCommit>),
    /// Wrap same-parent nodes in a new boolean group `new_id` combining
    /// them with `op` (paint order: the bottom node is the base).
    CreateBooleanGroup {
        node_ids: Vec<NodeId>,
        op: BooleanOp,
        new_id: NodeId,
    },
    /// Switch a boolean group's operation.
    SetBooleanOp { node_id: NodeId, op: BooleanOp },
    /// Remove anchors a path does not need, within `tolerance` doc px.
    Simplify { node_id: NodeId, tolerance: f64 },
    /// Reverse every subpath of a path (arrowheads and the non-zero fill
    /// follow the direction).
    Reverse { node_id: NodeId },
}

impl PathCommand {
    /// Short name for errors and logs.
    pub fn verb(&self) -> &'static str {
        match self {
            PathCommand::Commit(c) => c.kind.verb(),
            PathCommand::CreateBooleanGroup { .. } => "boolean group",
            PathCommand::SetBooleanOp { .. } => "set boolean op",
            PathCommand::Simplify { .. } => "simplify",
            PathCommand::Reverse { .. } => "reverse",
        }
    }
}

impl EditorState {
    /// Apply a [`PathCommand`]. `Ok(false)` when it does not apply (a
    /// missing / wrong-kind node, a stale commit, nothing to change);
    /// nothing is written then.
    pub fn apply_path_command(
        &mut self,
        command: PathCommand,
        _allocator: &mut dyn IdAllocator,
    ) -> Result<bool, IdAllocError> {
        let id = match command {
            PathCommand::Commit(commit) => self.commit_path_result(*commit)?,
            PathCommand::CreateBooleanGroup {
                node_ids,
                op,
                new_id,
            } => self.create_boolean_group(&node_ids, op, new_id),
            PathCommand::SetBooleanOp { node_id, op } => {
                return Ok(self.set_boolean_op(&node_id, op));
            }
            PathCommand::Simplify { node_id, tolerance } => {
                return Ok(self.simplify_path(&node_id, tolerance));
            }
            PathCommand::Reverse { node_id } => return Ok(self.reverse_path(&node_id)),
        };
        Ok(id.is_some())
    }

    /// The selected ids in paint order (bottom first): the order a
    /// boolean combines them in and Flatten bakes them.
    pub fn selection_in_paint_order(&self) -> Vec<NodeId> {
        let mut ordered: Vec<(Vec<usize>, NodeId)> = self
            .selection
            .set
            .iter()
            .filter_map(|id| Some((tree_position(self.active_children(), id)?, id.clone())))
            .collect();
        ordered.sort_by(|a, b| a.0.cmp(&b.0));
        ordered.into_iter().map(|(_, id)| id).collect()
    }
}

/// Index path of `id` from the page root (`[2, 0]` = first child of the
/// third root), for paint-order sorting.
pub(crate) fn tree_position(
    children: &[jian_ops_schema::node::PenNode],
    id: &NodeId,
) -> Option<Vec<usize>> {
    use crate::pen_node_ext::PenNodeExt;
    for (i, child) in children.iter().enumerate() {
        if child.id_str() == id.as_str() {
            return Some(vec![i]);
        }
        if let Some(kids) = child.children() {
            if let Some(mut rest) = tree_position(kids, id) {
                rest.insert(0, i);
                return Some(rest);
            }
        }
    }
    None
}
