//! Host-facing entry points: the canvas and the menus commit path
//! operations through the same [`EditorCommand::Path`] the MCP tools
//! send, minting new ids from the host's `next_id` counter (advanced only
//! when the write lands).
//!
//! [`EditorCommand::Path`]: crate::EditorCommand::Path

use super::{PathCommand, PathCommit, PathCommitKind};
use crate::node_id::NodeId;
use crate::state::EditorState;
use crate::vector_edit::Subpath;
use crate::{BooleanOp, EditorCommand, IdAllocator, SequentialIdAllocator};

/// A path the host computed for the canvas, before an id is minted.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPath {
    pub source_ids: Vec<NodeId>,
    /// Document space.
    pub subpaths: Vec<Subpath>,
    pub even_odd: bool,
    /// Document origin of the first source's frame.
    pub origin: (f64, f64),
    pub kind: PathCommitKind,
}

impl EditorState {
    /// Commit a host-computed path as one undo step (sources replaced,
    /// result selected). Built against the live revision, so the stale
    /// check passes by construction.
    pub fn commit_canvas_path(&mut self, path: CanvasPath, next_id: &mut u64) -> bool {
        let revision = self.document_revision();
        self.apply_minting(next_id, move |new_id| {
            PathCommand::Commit(Box::new(PathCommit {
                source_ids: path.source_ids,
                subpaths: path.subpaths,
                even_odd: path.even_odd,
                origin: path.origin,
                new_id,
                revision,
                kind: path.kind,
            }))
        })
    }

    /// Wrap `node_ids` in a new boolean group (one undo step, selected).
    pub fn create_canvas_boolean_group(
        &mut self,
        node_ids: Vec<NodeId>,
        op: BooleanOp,
        next_id: &mut u64,
    ) -> bool {
        self.apply_minting(next_id, move |new_id| PathCommand::CreateBooleanGroup {
            node_ids,
            op,
            new_id,
        })
    }

    /// Apply a path command that mints nothing (op switch, Simplify,
    /// Reverse).
    pub fn apply_canvas_path_command(&mut self, command: PathCommand) -> bool {
        self.apply(EditorCommand::Path(command))
    }

    fn apply_minting(
        &mut self,
        next_id: &mut u64,
        build: impl FnOnce(NodeId) -> PathCommand,
    ) -> bool {
        let Ok(mut allocator) = SequentialIdAllocator::for_document(&self.doc, *next_id) else {
            return false;
        };
        let Ok(new_id) = allocator.allocate(&mut self.collect_node_ids()) else {
            return false;
        };
        // The command carries its own id; the allocator is only the
        // `apply_with_allocator` argument and mints nothing more.
        let applied = self
            .apply_with_allocator(EditorCommand::Path(build(new_id)), &mut allocator)
            .unwrap_or(false);
        if applied {
            *next_id = allocator.next_counter();
        }
        applied
    }
}
