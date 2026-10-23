//! MCP tool `boolean_op` — union / subtract / intersect / exclude over
//! shapes on the active page, the agent-side twin of the canvas context
//! menu and Ctrl+Alt+U/S/I/X.
//!
//! Input: `{ "nodeIds": [String], "operation": "union"|"subtract"|
//!           "intersect"|"exclude" }` — at least two ids; order matters
//!           for `subtract` (the first shape minus the rest).
//!
//! Output: `{ "wrote": "true", "id": String }` — the new path's id.
//!
//! The skia math is the desktop canvas's own `compute_boolean_op`, run
//! at dispatch against the layout-resolved snapshot (the web canvas has
//! its own axis-aligned fallback; MCP always uses skia). The result rides
//! home as `EditorCommand::CommitBooleanResult`, which commits through
//! the same `EditorState` mutator the canvas uses, as one undo step. The
//! id is predicted with the sequential allocator `EditorState::apply`
//! uses and carried in the command together with the snapshot's
//! document revision, so a live document that moved on since the
//! snapshot rejects instead of committing stale geometry.
//!
//! A source with children (a frame holding content) is refused: the
//! commit removes each source with its subtree, and an agent cannot see
//! that loss happen.

use std::collections::BTreeMap;

use de_editor_core::NodeId;
use de_editor_core::{BooleanOp, DocumentIdAllocator, EditorCommand, EditorState, IdAllocator};
use de_editor_ui::layout_scene::{LayoutScene, NodeKind};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};

use super::export_tool::parse_string_array;

/// Snapshot of what one call needs: the layout-resolved active page for
/// the skia math and the argument checks, the id the commit will mint,
/// and the document revision the result was computed against.
pub struct BooleanOpTool {
    scene: LayoutScene,
    next_id: Option<NodeId>,
    revision: u64,
}

pub fn boolean_op_snapshot(state: &EditorState) -> BooleanOpTool {
    let next_id = DocumentIdAllocator::sequential_for_document(&state.doc)
        .and_then(|mut allocator| allocator.allocate(&mut state.collect_node_ids()))
        .ok();
    BooleanOpTool {
        scene: de_pen_loader::editor_state_to_active_page_layout_scene(state),
        next_id,
        revision: state.document_revision(),
    }
}

fn parse_operation(raw: &str) -> Option<BooleanOp> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "union" => Some(BooleanOp::Union),
        "subtract" => Some(BooleanOp::Subtract),
        "intersect" => Some(BooleanOp::Intersect),
        "exclude" => Some(BooleanOp::Exclude),
        _ => None,
    }
}

impl McpTool for BooleanOpTool {
    fn name(&self) -> &str {
        "boolean_op"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(raw_ids) = args.get("nodeIds").filter(|v| !v.trim().is_empty()) else {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "nodeIds is required (JSON array of at least two node ids)".into(),
            );
        };
        let node_ids = parse_string_array(raw_ids);
        if node_ids.len() < 2 {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("nodeIds needs at least two ids, got {}", node_ids.len()),
            );
        }
        let Some(raw_op) = args.get("operation") else {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "operation is required (union|subtract|intersect|exclude)".into(),
            );
        };
        let Some(op) = parse_operation(raw_op) else {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!(
                    "unknown operation {raw_op:?}: must be one of union, subtract, intersect, exclude"
                ),
            );
        };
        let page = self.scene.active_page();
        for (index, id) in node_ids.iter().enumerate() {
            let Some(node) = page.and_then(|page| page.find(id)) else {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("node {id:?} is not on the active page"),
                );
            };
            if matches!(
                node.kind,
                NodeKind::Group | NodeKind::Text | NodeKind::Other(_)
            ) {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!(
                        "node {id:?} is not a shape: use rectangles, frames, ellipses, \
                         polygons, lines or paths"
                    ),
                );
            }
            if !node.children.is_empty() {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!(
                        "node {id:?} has children, which the result would delete: \
                         move them out first"
                    ),
                );
            }
            if node_ids[..index].contains(id) {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("node {id:?} is listed twice"),
                );
            }
        }
        let Some(result) =
            de_host_native::boolean_ops::compute_boolean_op(&self.scene, &node_ids, op)
        else {
            return ToolOutcome::Err(
                ToolErrorCode::ToolFailed,
                "the result is empty (the shapes do not overlap for this operation)".into(),
            );
        };
        let Some(new_id) = self.next_id.clone() else {
            return ToolOutcome::Err(
                ToolErrorCode::Internal,
                "document id counter exhausted".into(),
            );
        };
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        out.insert("id".into(), new_id.as_str().to_string());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::CommitBooleanResult {
                source_ids: result.source_ids.iter().map(NodeId::new).collect(),
                contours: result.contours,
                new_id,
                revision: self.revision,
            },
        )
    }
}

#[cfg(test)]
#[path = "boolean_op_tool_tests.rs"]
mod tests;
