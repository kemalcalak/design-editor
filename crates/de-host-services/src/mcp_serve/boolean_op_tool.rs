//! MCP path operation tools (#388): `boolean_op`, `flatten`,
//! `outline_stroke`, `offset_path`, `simplify_path` and `reverse_path`,
//! the agent-side twins of the canvas shortcuts, menus and panel buttons.
//!
//! - `boolean_op { nodeIds, operation, mode? }`: `mode: "group"` (the
//!   default) wraps the shapes in a non-destructive boolean group whose
//!   children stay editable; `mode: "flatten"` bakes the result into one
//!   path and removes the sources. Order: the bottom-most layer is the
//!   base (subtract removes the others from it), whatever order the ids
//!   come in.
//! - `flatten { nodeIds }`: shapes, groups or a boolean group → one path.
//! - `outline_stroke { nodeId }`: the stroke (cap, join, dash, alignment)
//!   → a filled path; a filled source keeps its fill under it.
//! - `offset_path { nodeId, offset, join? }`: the outline grown (`offset`
//!   > 0) or shrunk, `join` round / miter / bevel.
//! - `simplify_path { nodeId, tolerance? }`, `reverse_path { nodeId }`.
//!
//! Geometry is `de_path_ops` (skia, curves kept), computed at dispatch on
//! the layout-resolved snapshot; the write rides home as one
//! `EditorCommand::Path`, the command the canvas writes through too. New
//! ids are predicted with the sequential allocator `EditorState::apply`
//! uses, and computed results carry the snapshot's document revision, so
//! a live document that moved on rejects instead of writing stale
//! geometry.

use std::collections::BTreeMap;

use de_editor_core::path_ops::{
    OffsetJoin, PathCommand, PathCommitKind, DEFAULT_SIMPLIFY_TOLERANCE,
};
use de_editor_core::NodeId;
use de_editor_core::{BooleanOp, DocumentIdAllocator, EditorCommand, EditorState, IdAllocator};
use de_editor_ui::layout_scene::{LayoutScene, NodeKind, SceneNode};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};

use super::export_tool::parse_string_array;

/// Which of the path tools a snapshot serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTool {
    Boolean,
    Flatten,
    OutlineStroke,
    Offset,
    Simplify,
    Reverse,
}

impl PathTool {
    pub const NAMES: [(&'static str, PathTool); 6] = [
        ("boolean_op", PathTool::Boolean),
        ("flatten", PathTool::Flatten),
        ("outline_stroke", PathTool::OutlineStroke),
        ("offset_path", PathTool::Offset),
        ("simplify_path", PathTool::Simplify),
        ("reverse_path", PathTool::Reverse),
    ];

    fn name(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(_, tool)| *tool == self)
            .map_or("boolean_op", |(name, _)| name)
    }
}

/// What one call needs: the layout-resolved active page, the id a new
/// node will get, and the document revision.
pub struct PathOpTool {
    tool: PathTool,
    scene: LayoutScene,
    next_id: Option<NodeId>,
    revision: u64,
}

pub fn path_op_snapshot(state: &EditorState, tool: PathTool) -> PathOpTool {
    let next_id = DocumentIdAllocator::sequential_for_document(&state.doc)
        .and_then(|mut allocator| allocator.allocate(&mut state.collect_node_ids()))
        .ok();
    PathOpTool {
        tool,
        scene: de_pen_loader::editor_state_to_active_page_layout_scene(state),
        next_id,
        revision: state.document_revision(),
    }
}

/// The `boolean_op` snapshot (kept for the registry's name table).
pub fn boolean_op_snapshot(state: &EditorState) -> PathOpTool {
    path_op_snapshot(state, PathTool::Boolean)
}

type Fail = (ToolErrorCode, String);

fn invalid(msg: String) -> Fail {
    (ToolErrorCode::InvalidArgument, msg)
}

/// Scene node and parent id of `id` on the active page.
fn locate<'a>(
    nodes: &'a [SceneNode],
    id: &str,
    parent: Option<&'a str>,
) -> Option<(&'a SceneNode, Option<&'a str>)> {
    for n in nodes {
        if n.id == id {
            return Some((n, parent));
        }
        if let Some(found) = locate(&n.children, id, Some(n.id.as_str())) {
            return Some(found);
        }
    }
    None
}

impl PathOpTool {
    fn node(&self, id: &str) -> Result<(&SceneNode, Option<&str>), Fail> {
        self.scene
            .active_page()
            .and_then(|page| locate(&page.children, id, None))
            .ok_or_else(|| invalid(format!("node {id:?} is not on the active page")))
    }

    fn new_id(&self) -> Result<NodeId, Fail> {
        self.next_id.clone().ok_or_else(|| {
            (
                ToolErrorCode::Internal,
                "document id counter exhausted".into(),
            )
        })
    }

    fn ids(&self, args: &BTreeMap<String, String>, min: usize) -> Result<Vec<String>, Fail> {
        let raw = args
            .get("nodeIds")
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                (
                    ToolErrorCode::MissingArgument,
                    format!("nodeIds is required (JSON array of at least {min} node id(s))"),
                )
            })?;
        let ids = parse_string_array(raw);
        if ids.len() < min {
            return Err(invalid(format!(
                "nodeIds needs at least {min} id(s), got {}",
                ids.len()
            )));
        }
        for (i, id) in ids.iter().enumerate() {
            if ids[..i].contains(id) {
                return Err(invalid(format!("node {id:?} is listed twice")));
            }
        }
        Ok(ids)
    }

    fn one_id(args: &BTreeMap<String, String>) -> Result<String, Fail> {
        args.get("nodeId")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| (ToolErrorCode::MissingArgument, "nodeId is required".into()))
    }

    /// A shape a boolean or Flatten can read an outline from; `bake`
    /// refuses containers whose content the write would delete.
    fn check_operand(&self, id: &str, bake: bool) -> Result<(), Fail> {
        let (node, _) = self.node(id)?;
        let is_group = node.kind == NodeKind::Group
            || (node.kind == NodeKind::Path && !node.children.is_empty());
        if matches!(node.kind, NodeKind::Text | NodeKind::Other(_)) {
            return Err(invalid(format!(
                "node {id:?} is not a shape: use rectangles, frames, ellipses, polygons, \
                 lines, paths, groups or boolean groups"
            )));
        }
        if bake && !is_group && !node.children.is_empty() {
            return Err(invalid(format!(
                "node {id:?} has children, which the result would delete: move them out first"
            )));
        }
        Ok(())
    }

    fn written(&self, id: &str, extra: &[(&str, &str)], command: PathCommand) -> ToolOutcome {
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        out.insert("id".into(), id.to_string());
        for (k, v) in extra {
            out.insert((*k).into(), (*v).into());
        }
        ToolOutcome::OkWithCommand(out, EditorCommand::Path(command))
    }

    fn commit(
        &self,
        plan: Option<de_path_ops::Planned>,
        kind: PathCommitKind,
        empty: &str,
    ) -> Result<ToolOutcome, Fail> {
        let plan = plan.ok_or_else(|| (ToolErrorCode::ToolFailed, empty.to_string()))?;
        let new_id = self.new_id()?;
        let commit = plan.into_commit(kind, new_id.clone(), self.revision);
        Ok(self.written(new_id.as_str(), &[], PathCommand::Commit(Box::new(commit))))
    }

    fn boolean(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, Fail> {
        let ids = self.ids(args, 2)?;
        let raw_op = args.get("operation").ok_or_else(|| {
            (
                ToolErrorCode::MissingArgument,
                "operation is required (union|subtract|intersect|exclude)".to_string(),
            )
        })?;
        let op = BooleanOp::parse(raw_op).ok_or_else(|| {
            invalid(format!(
                "unknown operation {raw_op:?}: must be one of union, subtract, intersect, exclude"
            ))
        })?;
        let flatten = match args.get("mode").map(|m| m.trim().to_ascii_lowercase()) {
            None => false,
            Some(m) if m.is_empty() || m == "group" => false,
            Some(m) if m == "flatten" => true,
            Some(m) => return Err(invalid(format!("unknown mode {m:?}: use group or flatten"))),
        };
        for id in &ids {
            self.check_operand(id, flatten)?;
        }
        let ids = self.paint_order(ids);
        if flatten {
            return self.commit(
                de_path_ops::plan_boolean(&self.scene, &ids, op),
                PathCommitKind::Boolean,
                "the result is empty (the shapes do not overlap for this operation)",
            );
        }
        let parent = self.node(&ids[0])?.1;
        if ids
            .iter()
            .any(|id| self.node(id).ok().map(|n| n.1) != Some(parent))
        {
            return Err(invalid(
                "a boolean group needs siblings: every node must share one parent".into(),
            ));
        }
        let new_id = self.new_id()?;
        Ok(self.written(
            new_id.as_str(),
            &[("mode", "group")],
            PathCommand::CreateBooleanGroup {
                node_ids: ids.iter().map(NodeId::new).collect(),
                op,
                new_id: new_id.clone(),
            },
        ))
    }

    /// Ids sorted bottom layer first (scene pre-order is paint order).
    fn paint_order(&self, mut ids: Vec<String>) -> Vec<String> {
        fn walk(nodes: &[SceneNode], out: &mut Vec<String>) {
            for n in nodes {
                out.push(n.id.clone());
                walk(&n.children, out);
            }
        }
        let mut order = Vec::new();
        if let Some(page) = self.scene.active_page() {
            walk(&page.children, &mut order);
        }
        ids.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
        ids
    }

    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, Fail> {
        match self.tool {
            PathTool::Boolean => self.boolean(args),
            PathTool::Flatten => {
                let ids = self.ids(args, 1)?;
                for id in &ids {
                    self.check_operand(id, true)?;
                }
                let ids = self.paint_order(ids);
                self.commit(
                    de_path_ops::plan_flatten(&self.scene, &ids),
                    PathCommitKind::Flatten,
                    "nothing to flatten: the nodes have no area",
                )
            }
            PathTool::OutlineStroke => {
                let id = Self::one_id(args)?;
                self.node(&id)?;
                self.commit(
                    de_path_ops::plan_outline_stroke(&self.scene, &id),
                    PathCommitKind::OutlineStroke,
                    "the node has no visible stroke to outline",
                )
            }
            PathTool::Offset => {
                let id = Self::one_id(args)?;
                self.check_operand(&id, true)?;
                let offset = args
                    .get("offset")
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .filter(|v| v.is_finite() && *v != 0.0)
                    .ok_or_else(|| {
                        (
                            ToolErrorCode::MissingArgument,
                            "offset is required: a non-zero number of px (negative insets)"
                                .to_string(),
                        )
                    })?;
                let join = match args.get("join") {
                    Some(j) => OffsetJoin::parse(j).ok_or_else(|| {
                        invalid(format!("unknown join {j:?}: round, miter or bevel"))
                    })?,
                    None => OffsetJoin::Round,
                };
                self.commit(
                    de_path_ops::plan_offset(&self.scene, &id, offset, join),
                    PathCommitKind::Offset,
                    "the offset leaves nothing (an inset past the shape's thickness)",
                )
            }
            PathTool::Simplify | PathTool::Reverse => {
                let id = Self::one_id(args)?;
                let (node, _) = self.node(&id)?;
                if node.kind != NodeKind::Path || !node.children.is_empty() {
                    return Err(invalid(format!(
                        "node {id:?} is not a path: flatten it first"
                    )));
                }
                let node_id = NodeId::new(&id);
                let command = if self.tool == PathTool::Reverse {
                    PathCommand::Reverse { node_id }
                } else {
                    let tolerance = match args.get("tolerance") {
                        Some(t) => t
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|t| t.is_finite() && *t >= 0.0)
                            .ok_or_else(|| {
                                invalid(format!("tolerance {t:?} must be a number ≥ 0"))
                            })?,
                        None => DEFAULT_SIMPLIFY_TOLERANCE,
                    };
                    PathCommand::Simplify { node_id, tolerance }
                };
                Ok(self.written(&id, &[], command))
            }
        }
    }
}

impl McpTool for PathOpTool {
    fn name(&self) -> &str {
        self.tool.name()
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args)
            .unwrap_or_else(|(code, message)| ToolOutcome::Err(code, message))
    }
}

#[cfg(test)]
#[path = "path_ops_tool_tests.rs"]
mod path_tests;
#[cfg(test)]
#[path = "boolean_op_tool_tests.rs"]
mod tests;
