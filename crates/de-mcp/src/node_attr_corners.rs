//! `set_node_corner_radius` (#389): one radius or four, plus Figma's
//! corner smoothing, written as one `SetNodeCorners` step. The tool checks
//! the write against the node with the core's `check_corners` before it
//! queues the command, so the agent reads why a kind refuses it (four
//! radii on a polygon, smoothing on an ellipse, a text node) instead of a
//! bare host rejection; the command checks again before writing.

use std::collections::BTreeMap;

use de_editor_core::command_node_attrs::corners::{
    check_corner_target, CornerTarget, CornersError,
};
use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::node::PenNode;

use super::write_tools::parse_node_id;
use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

/// First-party `set_node_corner_radius` tool.
///
/// - `radius`: one non-negative doc-px number for every corner, or
///   `[tl, tr, br, bl]` (an array, or the same four numbers separated by
///   commas / spaces). Per-corner radii fit frames, groups, rectangles,
///   images and browsers; polygons, stars, paths and ellipses take one.
/// - `smoothing`: Figma's corner smoothing as a fraction `0..=1`
///   (`0.6` = Figma's "iOS" 60 %), or a percentage string `"0%"..="100%"`.
///   `0` clears it. Frames, groups, rectangles, polygons / stars and paths.
///
/// At least one of the two is required.
pub struct SetNodeCornerRadius {
    /// What the check needs of each node on the active page (its kind and
    /// corner fields), by id: built in one walk per registry instead of a
    /// deep copy of the page, since the registry is rebuilt for every call
    /// and `tools/list`.
    targets: BTreeMap<String, CornerTarget>,
}

impl McpTool for SetNodeCornerRadius {
    fn name(&self) -> &str {
        "set_node_corner_radius"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let node_id = match parse_node_id(args, "node_id") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let radii = match args.get("radius").map(|raw| parse_radii(raw)) {
            None => None,
            Some(Ok(radii)) => Some(radii),
            Some(Err(message)) => return invalid(message),
        };
        let smoothing = match args.get("smoothing").map(|raw| parse_smoothing(raw)) {
            None => None,
            Some(Ok(smoothing)) => Some(smoothing),
            Some(Err(message)) => return invalid(message),
        };
        if radii.is_none() && smoothing.is_none() {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "radius (a number or [tl, tr, br, bl]) or smoothing (0-1 or \"0%\"-\"100%\") \
                 is required"
                    .into(),
            );
        }
        let target = self.targets.get(node_id.as_str()).copied();
        if let Err(error) = check_corner_target(&node_id, target, radii, smoothing) {
            return invalid(corners_refusal(&error));
        }
        let mut out = BTreeMap::new();
        out.insert("wrote".into(), "true".into());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::SetNodeCorners {
                node_id,
                radii,
                smoothing,
            },
        )
    }
}

pub fn set_node_corner_radius_snapshot(state: &EditorState) -> SetNodeCornerRadius {
    let mut targets = BTreeMap::new();
    collect_targets(state.active_children(), &mut targets);
    SetNodeCornerRadius { targets }
}

/// Every node under `nodes` by id, the first in depth-first order winning
/// like `walkers::find_node`, which the command resolves through.
fn collect_targets(nodes: &[PenNode], out: &mut BTreeMap<String, CornerTarget>) {
    for node in nodes {
        out.entry(node.id_str().to_owned())
            .or_insert_with(|| CornerTarget::of(node));
        if let Some(children) = node.children() {
            collect_targets(children, out);
        }
    }
}

/// The refusal the agent reads: the core's typed reason.
fn corners_refusal(error: &CornersError) -> String {
    format!("set_node_corner_radius refused: {error}")
}

fn invalid(message: String) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message)
}

/// One radius for every corner, or four in top-left, top-right,
/// bottom-right, bottom-left order.
pub(crate) fn parse_radii(raw: &str) -> Result<[f32; 4], String> {
    let trimmed = raw.trim();
    let bracketed = trimmed.starts_with('[');
    let inner = if bracketed {
        trimmed
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(|| format!("radius array is not closed: {raw:?}"))?
    } else {
        trimmed
    };
    let values: Vec<&str> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect();
    let parsed = values
        .iter()
        .map(|v| v.parse::<f32>().ok().filter(|r| r.is_finite() && *r >= 0.0))
        .collect::<Option<Vec<f32>>>()
        .ok_or_else(|| {
            format!("radius values must be non-negative finite doc-px numbers, got {raw:?}")
        })?;
    match parsed.as_slice() {
        [r] if !bracketed => Ok([*r; 4]),
        [tl, tr, br, bl] => Ok([*tl, *tr, *br, *bl]),
        _ => Err(format!(
            "radius must be one number or four [top-left, top-right, bottom-right, \
             bottom-left], got {raw:?}"
        )),
    }
}

/// A fraction `0..=1`, or a percentage `"0%"..="100%"`.
pub(crate) fn parse_smoothing(raw: &str) -> Result<f32, String> {
    let trimmed = raw.trim();
    let (number, scale) = match trimmed.strip_suffix('%') {
        Some(percent) => (percent.trim(), 100.0),
        None => (trimmed, 1.0),
    };
    let value = number
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v / scale);
    match value {
        Some(v) if (0.0..=1.0).contains(&v) => Ok(v),
        _ => Err(format!(
            "smoothing must be a fraction from 0 to 1 (0.6 = 60 %) or a percentage string \
             from \"0%\" to \"100%\", got {raw:?}"
        )),
    }
}
