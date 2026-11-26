//! MCP `scale_nodes` (#394): the agent-side twin of the Scale tool (K).
//!
//! `scale_nodes { nodeIds, factor | width | height, anchor?, variables? }`
//! scales the nodes and everything inside them about one of the nine
//! points of their combined bounds. The bounds are the layout-resolved
//! ones (a Fill / Hug node has no authored size), read from the snapshot's
//! scene; the write is one `EditorCommand::Scale`, the request the canvas
//! and the property panel build too ([`de_editor_ui::scale_plan`]), so it
//! is one undo step. Everything is checked against the snapshot first and
//! a call that would change nothing is refused.

use std::collections::BTreeMap;

use de_editor_core::selection_scale::factor_for_size;
use de_editor_core::{
    EditorCommand, EditorState, NodeId, ScaleAnchor, ScaleError, ScaleReport, ScaleRequest,
    ScaleTarget, VariableMode,
};
use de_editor_ui::layout_scene::LayoutScene;
use de_editor_ui::scale_plan::ScalePlan;
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};
use serde_json::{json, Value};

use super::export_tool::parse_string_array;

pub struct ScaleNodesTool {
    state: EditorState,
    scene: LayoutScene,
}

pub fn scale_nodes_snapshot(state: &EditorState) -> ScaleNodesTool {
    ScaleNodesTool {
        state: state.clone(),
        scene: de_pen_loader::editor_state_to_active_page_layout_scene(state),
    }
}

type Fail = (ToolErrorCode, String);

fn invalid(message: impl Into<String>) -> Fail {
    (ToolErrorCode::InvalidArgument, message.into())
}

fn scale_error(e: ScaleError) -> Fail {
    invalid(e.to_string())
}

fn number(args: &BTreeMap<String, String>, key: &str) -> Result<Option<f64>, Fail> {
    match args.get(key).map(|v| v.trim()).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(raw) => raw
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(Some)
            .ok_or_else(|| invalid(format!("{key} must be a number, got {raw:?}"))),
    }
}

impl ScaleNodesTool {
    fn ids(&self, args: &BTreeMap<String, String>) -> Result<Vec<NodeId>, Fail> {
        let raw = args
            .get("nodeIds")
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                (
                    ToolErrorCode::MissingArgument,
                    "nodeIds is required (JSON array of node ids)".to_string(),
                )
            })?;
        let ids = parse_string_array(raw);
        if ids.is_empty() {
            return Err(scale_error(ScaleError::NoTargets));
        }
        for (i, id) in ids.iter().enumerate() {
            if ids[..i].contains(id) {
                return Err(invalid(format!("node {id:?} is listed twice")));
            }
        }
        let ids: Vec<NodeId> = ids.iter().map(NodeId::new).collect();
        // Existence, instance children and locks, before any geometry.
        let probe = ScaleRequest {
            targets: ids
                .iter()
                .map(|id| ScaleTarget {
                    node_id: id.clone(),
                    pivot: None,
                })
                .collect(),
            factor: 2.0,
            variables: VariableMode::KeepBound,
        };
        self.state.check_scale(&probe).map_err(scale_error)?;
        Ok(ids)
    }

    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, Fail> {
        let ids = self.ids(args)?;
        let given = [
            ("factor", number(args, "factor")?),
            ("width", number(args, "width")?),
            ("height", number(args, "height")?),
        ];
        let set: Vec<_> = given.iter().filter(|(_, v)| v.is_some()).collect();
        let [(which, Some(value))] = set.as_slice() else {
            return Err((
                ToolErrorCode::MissingArgument,
                "give exactly one of factor, width or height".to_string(),
            ));
        };
        let anchor = match args
            .get("anchor")
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
        {
            None => ScaleAnchor::TopLeft,
            Some(raw) => ScaleAnchor::parse(raw).ok_or_else(|| {
                invalid(format!(
                    "unknown anchor {raw:?}: top-left, top, top-right, left, center, right, \
                     bottom-left, bottom or bottom-right"
                ))
            })?,
        };
        let variables = match args
            .get("variables")
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            None => VariableMode::KeepBound,
            Some(raw) => VariableMode::parse(raw)
                .ok_or_else(|| invalid(format!("unknown variables {raw:?}: keep or detach")))?,
        };
        let plan = ScalePlan::new(&self.state, &self.scene, &ids)
            .ok_or_else(|| scale_error(ScaleError::EmptyBounds))?;
        let factor = match *which {
            "width" => factor_for_size(plan.bounds.w, *value).map_err(scale_error)?,
            "height" => factor_for_size(plan.bounds.h, *value).map_err(scale_error)?,
            _ => *value,
        };
        let request = plan.request(factor, anchor, variables);
        let report = self.state.preview_scale(&request).map_err(scale_error)?;
        let after = plan.scaled_bounds(factor, anchor);
        let result = json!({
            "wrote": true,
            "nodeIds": plan.ids().map(|id| id.as_str()).collect::<Vec<_>>(),
            "factor": factor,
            "anchor": anchor.ident(),
            "bounds": {"x": after.x, "y": after.y, "width": after.w, "height": after.h},
            "warnings": warnings(&report),
        });
        Ok(ToolOutcome::OkJsonWithCommand(
            result.to_string(),
            EditorCommand::Scale(request),
        ))
    }
}

/// One readable line per value the scale left or unbound.
fn warnings(report: &ScaleReport) -> Vec<Value> {
    let bound = report.bound.iter().map(|n| {
        format!(
            "node {}: {} is bound to a variable and was not scaled (variables: detach scales it)",
            n.node_id, n.field
        )
    });
    let detached = report.detached.iter().map(|n| {
        format!(
            "node {}: {} was unbound from its variable",
            n.node_id, n.field
        )
    });
    let lost = report.unresolved_instances.iter().map(|id| {
        format!("instance {id}: its component was not found, so only its position moved")
    });
    bound
        .chain(detached)
        .chain(lost)
        .map(Value::String)
        .collect()
}

impl McpTool for ScaleNodesTool {
    fn name(&self) -> &str {
        "scale_nodes"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args)
            .unwrap_or_else(|(code, message)| ToolOutcome::Err(code, message))
    }
}

#[cfg(test)]
#[path = "scale_nodes_tool_tests.rs"]
mod tests;
