//! MCP `set_screen` (#223): mark a top-level frame as a Preview screen,
//! make it the start screen, or remove its marker — the property panel's
//! screen rows, through `de_editor_core::screen_marker_edit`.

use std::collections::BTreeMap;

use de_editor_core::screen_marker_edit::{screen_marker_commands, ScreenMarkerEdit};
use de_editor_core::EditorState;
use jian_ops_schema::node::PenNode;

use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

/// First-party `set_screen` tool.
pub struct SetScreen {
    /// The active page's top-level frames, without their subtrees: all a
    /// marker edit reads (ids, names, markers).
    frames: Vec<PenNode>,
}

pub fn set_screen_snapshot(state: &EditorState) -> SetScreen {
    let frames = state
        .active_children()
        .iter()
        .filter_map(|node| match node {
            PenNode::Frame(frame) => {
                let mut shallow = frame.clone();
                shallow.children = None;
                Some(PenNode::Frame(shallow))
            }
            _ => None,
        })
        .collect();
    SetScreen { frames }
}

impl McpTool for SetScreen {
    fn name(&self) -> &str {
        "set_screen"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(node_id) = args
            .get("nodeId")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "nodeId is required".into());
        };
        let edit = match args.get("mode").map(String::as_str).unwrap_or("mark") {
            "mark" => ScreenMarkerEdit::Mark,
            "start" => ScreenMarkerEdit::Start,
            "remove" => ScreenMarkerEdit::Remove,
            other => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("mode must be mark, start or remove, got {other}"),
                )
            }
        };
        let commands = match screen_marker_commands(&self.frames, node_id, edit) {
            Ok(commands) => commands,
            Err(error) => {
                return ToolOutcome::Err(ToolErrorCode::InvalidArgument, error.to_string())
            }
        };
        let mut out = BTreeMap::new();
        out.insert("nodeId".into(), node_id.to_owned());
        out.insert("wrote".into(), (!commands.is_empty()).to_string());
        if commands.is_empty() {
            return ToolOutcome::Ok(out);
        }
        ToolOutcome::OkWithCommand(out, EditorCommand::Batch { commands })
    }
}

#[cfg(test)]
#[path = "screen_tools_tests.rs"]
mod tests;
