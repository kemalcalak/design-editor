//! Shared plumbing for the HTML / web-snapshot import tools.
//!
//! `import_html`, `import_web_snapshot` (this crate), and
//! `import_html_url` (`de-host-services`) all place an imported
//! `PenNode` subtree onto the canvas with identical placement args and
//! insert-subtree outcome shape; the shared pieces live here.

use std::collections::BTreeMap;

use de_editor_core::NodeId;
use de_html::HtmlImportResult;
use jian_ops_schema::node::PenNode;

use super::write_tools::{parse_opt_i32, root_or_node_id};
use super::{EditorCommand, ToolErrorCode, ToolOutcome};

/// Placement args shared by every import tool: optional x/y offset for
/// the root frame, target parent, and target page.
pub struct ImportPlacement {
    pub x: i32,
    pub y: i32,
    pub parent_id: NodeId,
    pub page_id: Option<String>,
}

/// Parse the optional `viewportWidth` import arg (CSS pixels). Absent or
/// blank keeps the importer's default width.
pub fn parse_viewport_width(
    args: &BTreeMap<String, String>,
) -> Result<Option<f64>, (ToolErrorCode, String)> {
    parse_viewport_axis(args, "viewportWidth", "viewport_width")
}

/// Parse the optional `viewportHeight` import arg (CSS pixels). Absent or
/// blank keeps the importer's aspect-derived default.
pub fn parse_viewport_height(
    args: &BTreeMap<String, String>,
) -> Result<Option<f64>, (ToolErrorCode, String)> {
    parse_viewport_axis(args, "viewportHeight", "viewport_height")
}

fn parse_viewport_axis(
    args: &BTreeMap<String, String>,
    key: &str,
    snake_key: &str,
) -> Result<Option<f64>, (ToolErrorCode, String)> {
    let Some(raw) = args
        .get(key)
        .or_else(|| args.get(snake_key))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    raw.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(Some)
        .ok_or_else(|| {
            (
                ToolErrorCode::InvalidArgument,
                format!("{key}: expected a positive number of CSS pixels"),
            )
        })
}

/// Place the imported roots. A page import has one root, which lands at
/// the placement's x/y; a slide deck has one board per slide, already laid
/// out in a grid, which moves by x/y as a whole.
pub fn offset_roots(nodes: &mut [PenNode], placement: &ImportPlacement) {
    if placement.x == 0 && placement.y == 0 {
        return;
    }
    let single = nodes.len() == 1;
    for node in nodes {
        if let PenNode::Frame(frame) = node {
            let base = |value: Option<f64>| if single { 0.0 } else { value.unwrap_or(0.0) };
            frame.base.x = Some(base(frame.base.x) + placement.x as f64);
            frame.base.y = Some(base(frame.base.y) + placement.y as f64);
        }
    }
}

/// Parse the shared placement args. `Err` carries the typed tool error
/// to return to the caller.
pub fn parse_import_placement(
    args: &BTreeMap<String, String>,
) -> Result<ImportPlacement, (ToolErrorCode, String)> {
    let x = parse_opt_i32(args, "x")
        .map_err(|error| (ToolErrorCode::InvalidArgument, format!("x: {error}")))?
        .unwrap_or(0);
    let y = parse_opt_i32(args, "y")
        .map_err(|error| (ToolErrorCode::InvalidArgument, format!("y: {error}")))?
        .unwrap_or(0);
    let parent_id = args
        .get("parent")
        .or_else(|| args.get("parent_id"))
        .or_else(|| args.get("target_parent_id"))
        .map(|value| root_or_node_id(value))
        .unwrap_or(NodeId::NONE);
    let page_id = args
        .get("pageId")
        .or_else(|| args.get("page_id"))
        .or_else(|| args.get("page"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(ImportPlacement {
        x,
        y,
        parent_id,
        page_id,
    })
}

/// Total node count of an imported subtree (containers recurse through
/// `children`). Distinct from the `count_nodes` MCP read tool, which
/// counts the open document.
pub fn count_subtree_nodes(nodes: &[PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            1 + match node {
                PenNode::Frame(node) => node
                    .children
                    .as_deref()
                    .map(count_subtree_nodes)
                    .unwrap_or_default(),
                PenNode::Group(node) => node
                    .children
                    .as_deref()
                    .map(count_subtree_nodes)
                    .unwrap_or_default(),
                PenNode::Rectangle(node) => node
                    .children
                    .as_deref()
                    .map(count_subtree_nodes)
                    .unwrap_or_default(),
                _ => 0,
            }
        })
        .sum()
}

/// Turn an import result into the shared insert-subtree outcome:
/// reject empty imports, apply the x/y offset to the root frames, and
/// report `wrote` / `nodeCount` / `slides` (a deck) / `warnings`. Every root
/// goes in one `InsertSubtree`, so a deck's boards keep their order.
pub fn import_result_to_outcome(
    result: HtmlImportResult,
    placement: ImportPlacement,
) -> ToolOutcome {
    if result.nodes.is_empty() {
        let detail = result
            .warnings
            .first()
            .map(String::as_str)
            .unwrap_or("input produced no nodes");
        return ToolOutcome::Err(
            ToolErrorCode::InvalidArgument,
            format!("no importable content: {detail}"),
        );
    }
    let mut nodes = result.nodes;
    offset_roots(&mut nodes, &placement);
    let mut output = BTreeMap::new();
    output.insert("wrote".into(), "true".into());
    output.insert("nodeCount".into(), count_subtree_nodes(&nodes).to_string());
    if nodes.len() > 1 {
        output.insert("slides".into(), nodes.len().to_string());
    }
    if !result.warnings.is_empty() {
        output.insert("warnings".into(), result.warnings.join("\n"));
    }
    ToolOutcome::OkWithCommand(
        output,
        EditorCommand::InsertSubtree {
            nodes,
            parent_id: placement.parent_id,
            page_id: placement.page_id,
        },
    )
}
