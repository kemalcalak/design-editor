//! First-party MCP read tools (part 2). Carved off `tools.rs` to keep
//! both files under the 800-line cap as the read surface grew.
//!
//! Ported off shell-core's `Document` onto `de_editor_core::
//! EditorState` (canonical `PenDocument`).

use std::collections::BTreeMap;

use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::shared_state::SharedState;
use de_editor_core::EditorState;
use jian_ops_schema::node::PenNode;
use jian_scene::layout_scene::{NodeKind, SceneNode, ScenePage};
use serde_json::{json, Map, Value};

use super::layout_grid::{cell_json, grid_meta, grid_of, tracks_json, GridMeta, GridTracks};
use super::layout_problems::{child_clip, clip_problem, Area};
use super::tools::kind_label;
use super::{McpTool, ToolErrorCode, ToolOutcome};
use de_editor_core::grid_geometry::GridCell;

type ToolCallError = (ToolErrorCode, String);

// --- snapshot_layout -------------------------------------------------

/// First-party `snapshot_layout` tool — bounding box of every
/// node on the active page, optionally filtered by page / parent /
/// depth for TS CLI compatibility.
pub struct SnapshotLayout {
    state: SharedState,
}

#[derive(Clone)]
struct PageLayout {
    records: Vec<LayoutRecord>,
}

#[derive(Clone)]
struct LayoutRecord {
    id: String,
    name: Option<String>,
    type_label: String,
    ancestors: Vec<String>,
    depth: u32,
    // Document-space coords kept as f64 (TS `LayoutEntry` returns raw
    // numbers); truncating to i32 would drop fractional positions/sizes.
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// `"partially clipped"` / `"fully clipped"` when a clipping ancestor
    /// hides part or all of the node (#169, pen.dev `ctx.problems`), and
    /// `"overlaps <id>"` for each sibling it covers (#265), comma-joined.
    problems: Option<String>,
    /// Resolved tracks of a grid container (#379).
    grid: Option<GridTracks>,
    /// This node's cell in its grid parent.
    cell: Option<GridCell>,
}

#[derive(Clone)]
struct NodeMeta {
    name: Option<String>,
    type_label: String,
    grid: GridMeta,
}

impl McpTool for SnapshotLayout {
    fn name(&self) -> &str {
        "snapshot_layout"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        // Only the page the call reads is laid out (#456).
        let page = match page_layout(&self.state, args) {
            Ok(page) => page,
            Err((code, msg)) => return ToolOutcome::Err(code, msg),
        };
        let max_depth = match arg_alias(args, &["maxDepth", "depth"]) {
            Some(raw) => match raw.parse::<u32>() {
                Ok(v) => v,
                Err(_) => {
                    return ToolOutcome::Err(
                        ToolErrorCode::InvalidArgument,
                        format!("maxDepth must be a u32, got {raw:?}"),
                    );
                }
            },
            None => 1,
        };
        let parent_id = arg_alias(args, &["parentId", "parent_id", "parent"]);
        // TS calls `computeLayoutTree(parent.children, …)` with the default
        // `parentX/parentY = 0`, so a `parentId` query returns coords RELATIVE
        // to that parent (not document-absolute). Our records are page-rooted
        // absolute, so subtract the parent's absolute origin to match.
        let (parent_depth, parent_offset) = match parent_id {
            Some(id) => match page.records.iter().find(|r| r.id == id) {
                Some(parent) => (Some(parent.depth), (parent.x, parent.y)),
                None => {
                    // TS snapshot-layout.ts:33 throws `Node not found: ${id}`.
                    return ToolOutcome::Err(
                        ToolErrorCode::ToolFailed,
                        format!("Node not found: {id}"),
                    );
                }
            },
            None => (None, (0.0, 0.0)),
        };
        let selected: Vec<&LayoutRecord> = page
            .records
            .iter()
            .filter(|record| match (parent_id, parent_depth) {
                (Some(parent), Some(depth)) => {
                    record.ancestors.iter().any(|id| id == parent)
                        && record.depth <= depth + max_depth + 1
                }
                _ => record.depth <= max_depth,
            })
            .collect();
        // TS `snapshot_layout` returns `{ layout: LayoutEntry[] }` — a nested
        // tree of {id,name?,type,x,y,width,height,children?} with absolute
        // coords. Roots are the page's top-level nodes (or `parentId`'s direct
        // children); children nest up to maxDepth. The flat `selected` set is
        // already depth-bounded, so we just re-nest it by parent.
        let root_key = parent_id.unwrap_or("");
        let layout = layout_children_values(&selected, root_key, parent_offset);
        ToolOutcome::OkJson(json!({ "layout": layout }).to_string())
    }
}

/// Build the ordered list of `LayoutEntry` JSON objects whose immediate
/// parent (in document order) is `parent_key` (`""` for page-roots), each
/// recursively carrying its own children — mirrors TS `computeLayoutTree`.
/// `offset` is subtracted from every coord so a `parentId` query returns
/// parent-relative coords (TS passes `parentX/Y = 0`); it is `(0, 0)` for a
/// page-level query (coords stay document-absolute).
fn layout_children_values(
    selected: &[&LayoutRecord],
    parent_key: &str,
    offset: (f64, f64),
) -> Vec<Value> {
    selected
        .iter()
        .filter(|record| record.ancestors.last().map(String::as_str).unwrap_or("") == parent_key)
        .map(|record| layout_entry_value(record, selected, offset))
        .collect()
}

/// Serialize a document coordinate the way JS `JSON.stringify` does: a
/// whole number as an integer (`100`, not `100.0`), a fractional value as a
/// decimal — so the wire bytes match TS `LayoutEntry` numbers exactly.
fn js_number(v: f64) -> Value {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 9_007_199_254_740_992.0 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

fn layout_entry_value(
    record: &LayoutRecord,
    selected: &[&LayoutRecord],
    offset: (f64, f64),
) -> Value {
    let mut obj = Map::new();
    obj.insert("id".into(), json!(record.id));
    // TS `LayoutEntry.name` is optional — omit when the node is unnamed
    // (JSON.stringify drops `undefined`).
    if let Some(name) = &record.name {
        obj.insert("name".into(), json!(name));
    }
    obj.insert("type".into(), json!(&record.type_label));
    obj.insert("x".into(), js_number(record.x - offset.0));
    obj.insert("y".into(), js_number(record.y - offset.1));
    obj.insert("width".into(), js_number(record.w));
    obj.insert("height".into(), js_number(record.h));
    if let Some(problems) = &record.problems {
        obj.insert("problems".into(), json!(problems));
    }
    if let Some(grid) = &record.grid {
        obj.insert("grid".into(), tracks_json(grid, offset));
    }
    if let Some(cell) = &record.cell {
        obj.insert("cell".into(), cell_json(cell));
    }
    let children = layout_children_values(selected, &record.id, offset);
    if !children.is_empty() {
        obj.insert("children".into(), Value::Array(children));
    }
    Value::Object(obj)
}

pub fn snapshot_layout_snapshot(state: &EditorState) -> SnapshotLayout {
    snapshot_layout_shared(SharedState::copied(state))
}

/// [`snapshot_layout_snapshot`] over a state the caller already shares:
/// no copy (#456).
pub fn snapshot_layout_shared(state: SharedState) -> SnapshotLayout {
    SnapshotLayout { state }
}

/// The page a read names (`pageId` / `page_id` / `page`, else the active
/// one), by index. A document without pages has the one page `0`.
fn target_page(
    state: &EditorState,
    args: &BTreeMap<String, String>,
) -> Result<usize, ToolCallError> {
    let id = arg_alias(args, &["pageId", "page_id", "page"]);
    let found = match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => match id {
            Some(id) => pages.iter().position(|page| page.id == id),
            None => Some(state.ui.active_page_index.min(pages.len() - 1)),
        },
        _ => match id {
            None | Some("0") => Some(0),
            Some(_) => None,
        },
    };
    found.ok_or_else(|| {
        (
            ToolErrorCode::ToolFailed,
            format!("page not found: {}", id.unwrap_or("0")),
        )
    })
}

/// The authored roots of page `index` (the root children without pages).
fn page_roots(state: &EditorState, index: usize) -> &[PenNode] {
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages
            .get(index)
            .map(|page| page.children.as_slice())
            .unwrap_or_default(),
        _ => state.doc.children.as_slice(),
    }
}

/// The layout records of the page a call reads, laying out that page only.
fn page_layout(
    state: &EditorState,
    args: &BTreeMap<String, String>,
) -> Result<PageLayout, ToolCallError> {
    let index = target_page(state, args)?;
    let scene = de_pen_loader::editor_state_to_page_layout_scene(state, index);
    let mut meta = BTreeMap::new();
    collect_node_meta(page_roots(state, index), &mut meta);
    let records = scene
        .pages
        .get(index)
        .map(|scene_page| scene_layout_records(&scene_page.children, &meta, None))
        .unwrap_or_default();
    Ok(PageLayout { records })
}

// --- find_empty_space -----------------------------------------------

/// First-party `find_empty_space` tool — find a padded position for a
/// new rectangle relative to either the page content or one node.
pub struct FindEmptySpace {
    state: SharedState,
}

#[derive(Clone)]
struct PageSpace {
    roots: Vec<BoundsRecord>,
    all: Vec<BoundsRecord>,
}

#[derive(Clone)]
struct BoundsRecord {
    id: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl McpTool for FindEmptySpace {
    fn name(&self) -> &str {
        "find_empty_space"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let width = match required_i32_arg(args, "width") {
            Ok(v) => v,
            Err((code, msg)) => return ToolOutcome::Err(code, msg),
        };
        let height = match required_i32_arg(args, "height") {
            Ok(v) => v,
            Err((code, msg)) => return ToolOutcome::Err(code, msg),
        };
        let padding = match optional_i32_arg(args, "padding", 50) {
            Ok(v) => v,
            Err((code, msg)) => return ToolOutcome::Err(code, msg),
        };
        let Some(direction) = args.get("direction") else {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "direction is required".into(),
            );
        };
        // Only the page the call reads is laid out (#456).
        let page = match page_space_for(&self.state, args) {
            Ok(page) => page,
            Err((code, msg)) => return ToolOutcome::Err(code, msg),
        };
        let node_id = arg_alias(args, &["nodeId", "node_id", "node"]);
        let nodes: Vec<&BoundsRecord> = match node_id {
            Some(id) => match page.all.iter().find(|record| record.id == id) {
                Some(record) => vec![record],
                None => {
                    return ToolOutcome::Err(
                        ToolErrorCode::ToolFailed,
                        format!("node not found: {id}"),
                    );
                }
            },
            None => page.roots.iter().collect(),
        };
        let (x, y) = match find_padded_position(&nodes, direction, width, height, padding) {
            Ok(pos) => pos,
            Err((code, msg)) => return ToolOutcome::Err(code, msg),
        };
        let mut out = BTreeMap::new();
        out.insert("x".into(), x.to_string());
        out.insert("y".into(), y.to_string());
        ToolOutcome::Ok(out)
    }
}

pub fn find_empty_space_snapshot(state: &EditorState) -> FindEmptySpace {
    find_empty_space_shared(SharedState::copied(state))
}

/// [`find_empty_space_snapshot`] over a state the caller already shares:
/// no copy (#456).
pub fn find_empty_space_shared(state: SharedState) -> FindEmptySpace {
    FindEmptySpace { state }
}

/// The bounds of the page a call reads, laying out that page only.
fn page_space_for(
    state: &EditorState,
    args: &BTreeMap<String, String>,
) -> Result<PageSpace, ToolCallError> {
    let index = target_page(state, args)?;
    let scene = de_pen_loader::editor_state_to_page_layout_scene(state, index);
    let roots = scene
        .pages
        .get(index)
        .map(|page| page.children.as_slice())
        .unwrap_or_default();
    Ok(page_space(roots))
}

fn collect_node_meta(nodes: &[PenNode], out: &mut BTreeMap<String, NodeMeta>) {
    for node in nodes {
        out.insert(
            node.id_str().to_string(),
            NodeMeta {
                name: node.base().name.clone(),
                type_label: kind_label(node).to_string(),
                grid: grid_meta(node),
            },
        );
        if let Some(children) = node.children() {
            collect_node_meta(children, out);
        }
    }
}

/// `clip` is the area the roots' clipping ancestors leave visible (`None` for
/// page roots).
fn scene_layout_records(
    roots: &[SceneNode],
    meta: &BTreeMap<String, NodeMeta>,
    clip: Option<Area>,
) -> Vec<LayoutRecord> {
    #[allow(clippy::too_many_arguments)]
    fn walk(
        nodes: &[SceneNode],
        meta: &BTreeMap<String, NodeMeta>,
        grid_meta: &BTreeMap<String, GridMeta>,
        cells: &mut BTreeMap<String, GridCell>,
        ancestors: &[String],
        clip: Option<Area>,
        depth: u32,
        out: &mut Vec<LayoutRecord>,
    ) {
        // Page roots are placed apart by #37; overlaps are checked inside them.
        let overlaps = if depth > 0 {
            super::layout_overlaps::sibling_overlaps(nodes)
        } else {
            BTreeMap::new()
        };
        for node in nodes {
            let id = node.id.clone();
            let node_meta = meta.get(&id);
            let mut problems: Vec<String> = clip_problem(node, clip)
                .map(str::to_string)
                .into_iter()
                .collect();
            if let Some(others) = overlaps.get(&id) {
                problems.extend(others.iter().map(|other| format!("overlaps {other}")));
            }
            let bounds = node.aggregate_bounds();
            let grid = grid_of(node, grid_meta).map(|(tracks, children)| {
                cells.extend(children);
                tracks
            });
            out.push(LayoutRecord {
                id: id.clone(),
                name: node_meta.and_then(|m| m.name.clone()),
                type_label: node_meta
                    .map(|m| m.type_label.clone())
                    .unwrap_or_else(|| scene_kind_label(&node.kind).to_string()),
                ancestors: ancestors.to_vec(),
                depth,
                x: f64::from(bounds.origin.x),
                y: f64::from(bounds.origin.y),
                w: f64::from(bounds.size.x),
                h: f64::from(bounds.size.y),
                problems: (!problems.is_empty()).then(|| problems.join(", ")),
                grid,
                cell: cells.remove(&id),
            });
            let mut child_ancestors = ancestors.to_vec();
            child_ancestors.push(id);
            let clip = child_clip(node, clip);
            walk(
                &node.children,
                meta,
                grid_meta,
                cells,
                &child_ancestors,
                clip,
                depth + 1,
                out,
            );
        }
    }

    let grid_meta: BTreeMap<String, GridMeta> = meta
        .iter()
        .map(|(id, m)| (id.clone(), m.grid.clone()))
        .collect();
    let mut cells = BTreeMap::new();
    let mut out = Vec::new();
    walk(roots, meta, &grid_meta, &mut cells, &[], clip, 0, &mut out);
    out
}

/// The visible area `id`'s clipping ancestors leave it: `Some(clip)` once
/// `id` is found under `nodes`, `None` when it is not there.
fn clip_above(nodes: &[SceneNode], id: &str, clip: Option<Area>) -> Option<Option<Area>> {
    nodes.iter().find_map(|node| {
        if node.id == id {
            return Some(clip);
        }
        clip_above(&node.children, id, child_clip(node, clip))
    })
}

fn scene_kind_label(kind: &NodeKind) -> &str {
    match kind {
        NodeKind::Frame => "frame",
        NodeKind::Group => "group",
        NodeKind::Rect => "rectangle",
        NodeKind::Ellipse => "ellipse",
        NodeKind::Polygon => "polygon",
        NodeKind::Line => "line",
        NodeKind::Path => "path",
        NodeKind::Text => "text",
        NodeKind::Other(label) => label.as_str(),
    }
}

/// `LayoutEntry[]` JSON for the subtree rooted at `root_id` in a resolved
/// scene page (depth-limited, document-absolute coords), read from the same
/// jian layout `snapshot_layout` reports — so auto-layout children carry their
/// flowed position, not a stale authored `x`/`y`. `root` supplies names and
/// type labels. Used by `design_refine` for its `layoutSnapshot` result.
pub(crate) fn subtree_layout_value(page: &ScenePage, root: &PenNode, max_depth: u32) -> Value {
    let Some(scene_root) = page.find(root.id_str()) else {
        return Value::Array(Vec::new());
    };
    let mut meta = BTreeMap::new();
    collect_node_meta(std::slice::from_ref(root), &mut meta);
    let clip = clip_above(&page.children, root.id_str(), None).flatten();
    let records = scene_layout_records(std::slice::from_ref(scene_root), &meta, clip);
    let selected: Vec<&LayoutRecord> = records.iter().filter(|r| r.depth <= max_depth).collect();
    Value::Array(layout_children_values(&selected, "", (0.0, 0.0)))
}

fn page_space(roots: &[SceneNode]) -> PageSpace {
    fn walk(nodes: &[SceneNode], out: &mut Vec<BoundsRecord>) {
        for n in nodes {
            out.push(bounds_record(n));
            walk(&n.children, out);
        }
    }
    let root_bounds = roots.iter().map(bounds_record).collect();
    let mut all = Vec::new();
    walk(roots, &mut all);
    PageSpace {
        roots: root_bounds,
        all,
    }
}

fn bounds_record(node: &SceneNode) -> BoundsRecord {
    let b = node.aggregate_bounds();
    BoundsRecord {
        id: node.id.clone(),
        x: b.origin.x as i32,
        y: b.origin.y as i32,
        w: b.size.x as i32,
        h: b.size.y as i32,
    }
}

fn find_padded_position(
    nodes: &[&BoundsRecord],
    direction: &str,
    width: i32,
    height: i32,
    padding: i32,
) -> Result<(i32, i32), ToolCallError> {
    if nodes.is_empty() {
        return Ok((0, 0));
    }
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for n in nodes {
        min_x = min_x.min(n.x);
        min_y = min_y.min(n.y);
        max_x = max_x.max(n.x + n.w);
        max_y = max_y.max(n.y + n.h);
    }
    match direction {
        "right" => Ok((max_x + padding, min_y)),
        "left" => Ok((min_x - padding - width, min_y)),
        "bottom" => Ok((min_x, max_y + padding)),
        "top" => Ok((min_x, min_y - padding - height)),
        other => Err((
            ToolErrorCode::InvalidArgument,
            format!("direction must be top/right/bottom/left, got {other:?}"),
        )),
    }
}

fn arg_alias<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| args.get(*key).map(String::as_str))
}

fn required_i32_arg(args: &BTreeMap<String, String>, key: &str) -> Result<i32, ToolCallError> {
    let Some(raw) = args.get(key) else {
        return Err((ToolErrorCode::MissingArgument, format!("{key} is required")));
    };
    raw.parse::<i32>().map_err(|_| {
        (
            ToolErrorCode::InvalidArgument,
            format!("{key} must be an i32, got {raw:?}"),
        )
    })
}

fn optional_i32_arg(
    args: &BTreeMap<String, String>,
    key: &str,
    default: i32,
) -> Result<i32, ToolCallError> {
    match args.get(key) {
        Some(raw) => raw.parse::<i32>().map_err(|_| {
            (
                ToolErrorCode::InvalidArgument,
                format!("{key} must be an i32, got {raw:?}"),
            )
        }),
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bottom_args(page_id: Option<&str>) -> BTreeMap<String, String> {
        let mut args = BTreeMap::from([
            ("width".into(), "50".into()),
            ("height".into(), "50".into()),
            ("padding".into(), "10".into()),
            ("direction".into(), "bottom".into()),
        ]);
        if let Some(id) = page_id {
            args.insert("pageId".into(), id.into());
        }
        args
    }

    #[test]
    fn empty_space_uses_resolved_omitted_height_and_requested_page() {
        let src = r#"{
          "version":"1.0.0","pages":[
            {"id":"hug-page","name":"Hug","children":[
              {"type":"frame","id":"hug","x":10,"y":20,"width":390,
               "layout":"vertical","gap":5,"padding":[10,20],"children":[
                 {"type":"rectangle","id":"a","width":"fill_container","height":40},
                 {"type":"rectangle","id":"b","width":"fill_container","height":50}
               ]},
              {"type":"frame","id":"fixed","x":500,"y":30,"width":200,"height":20}
            ]},
            {"id":"other-page","name":"Other","children":[
              {"type":"frame","id":"other","x":900,"y":100,"width":20,"height":30}
            ]}
          ],"children":[]
        }"#;
        let parsed = jian_ops_schema::load_str(src).expect("parse fixture");
        let mut state = EditorState::from_document(parsed.value);
        state.ui.active_page_index = 1;
        let tool = find_empty_space_snapshot(&state);

        let ToolOutcome::Ok(active) = tool.call(&bottom_args(None)) else {
            panic!("active-page lookup should succeed");
        };
        assert_eq!(active.get("x").map(String::as_str), Some("900"));
        assert_eq!(active.get("y").map(String::as_str), Some("140"));

        let ToolOutcome::Ok(hug) = tool.call(&bottom_args(Some("hug-page"))) else {
            panic!("requested-page lookup should succeed");
        };
        assert_eq!(hug.get("x").map(String::as_str), Some("10"));
        assert_eq!(
            hug.get("y").map(String::as_str),
            Some("145"),
            "bottom placement must include the resolved 115px Hug root"
        );
    }
}
