//! The tree an `execute` visitor walks (`Get(path, visit)` / `Get(visit)`):
//! one entry per node with the node's data, its resolved bounds and its
//! clipping problem, read from the same jian layout `snapshot_layout` uses.
//!
//! Wire shape per entry (kept short, the prelude walks it):
//! `{ "n": node, "b": {x, y, width, height}, "i": index, "p"?: problem,
//!    "c"?: [entries] }`. `n` is the node as `Get` returns it, with its
//! children elided to `"..."` (the visitor sees them as their own entries).
//! `b` is in the parent's coordinate space, like the node's own `x` / `y`.

use std::collections::HashMap;

use de_editor_core::EditorState;
use jian_scene::layout_scene::SceneNode;
use serde_json::{json, Map, Value};

use super::layout_problems::{child_clip, clip_problem, Area};

/// Resolved boxes of every node on the active page, by scene id.
#[derive(Default)]
pub(crate) struct LayoutIndex {
    boxes: HashMap<String, NodeBox>,
}

struct NodeBox {
    /// Document-space box.
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    /// Document-space origin of the parent's box; `(0, 0)` for page roots.
    parent_x: f32,
    parent_y: f32,
    problem: Option<&'static str>,
}

impl LayoutIndex {
    pub(crate) fn build(state: &EditorState) -> Self {
        let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
        let mut index = Self::default();
        if let Some(page) = scene.active_page() {
            index.walk(&page.children, (0.0, 0.0), None);
        }
        index
    }

    fn walk(&mut self, nodes: &[SceneNode], parent: (f32, f32), clip: Option<Area>) {
        for node in nodes {
            let bounds = node.aggregate_bounds();
            self.boxes.insert(
                node.id.clone(),
                NodeBox {
                    x: bounds.origin.x,
                    y: bounds.origin.y,
                    width: bounds.size.x,
                    height: bounds.size.y,
                    parent_x: parent.0,
                    parent_y: parent.1,
                    problem: clip_problem(node, clip),
                },
            );
            self.walk(
                &node.children,
                (bounds.origin.x, bounds.origin.y),
                child_clip(node, clip),
            );
        }
    }

    /// The box of `id`, or of an `instance/child` path through the virtual
    /// `instance__child` id the canvas expander gives instance children.
    fn find(&self, id: &str) -> Option<&NodeBox> {
        self.boxes
            .get(id)
            .or_else(|| self.boxes.get(&id.replace('/', "__")))
    }
}

/// Visitor entries for `nodes` (full-depth `Get` values), down to
/// `max_depth` levels below them (`None` = all). `first_index` is the
/// sibling index of `nodes[0]` (a path-rooted walk starts at the node's own
/// position among its siblings).
pub(crate) fn visitor_entries(
    nodes: Vec<Value>,
    layout: &LayoutIndex,
    max_depth: Option<usize>,
    first_index: usize,
) -> Value {
    Value::Array(
        nodes
            .into_iter()
            .enumerate()
            .map(|(i, node)| entry(node, layout, max_depth, 0, first_index + i))
            .collect(),
    )
}

fn entry(
    mut node: Value,
    layout: &LayoutIndex,
    max_depth: Option<usize>,
    depth: usize,
    index: usize,
) -> Value {
    let children = match node.get_mut("children") {
        Some(Value::Array(items)) if !items.is_empty() => {
            let items = std::mem::take(items);
            node["children"] = json!("...");
            items
        }
        _ => Vec::new(),
    };
    let mut out = Map::new();
    out.insert("b".into(), bounds_value(&node, layout));
    if let Some(problem) = node
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| layout.find(id))
        .and_then(|b| b.problem)
    {
        out.insert("p".into(), json!(problem));
    }
    out.insert("i".into(), json!(index));
    let descend = max_depth.is_none_or(|max| depth < max);
    if descend && !children.is_empty() {
        let entries: Vec<Value> = children
            .into_iter()
            .enumerate()
            .map(|(i, child)| entry(child, layout, max_depth, depth + 1, i))
            .collect();
        out.insert("c".into(), Value::Array(entries));
    }
    out.insert("n".into(), node);
    Value::Object(out)
}

/// The node's box relative to its parent. A node the layout does not know
/// (it paints nothing) falls back to its authored numeric `x` / `y` /
/// `width` / `height`, zero where those are not numbers.
fn bounds_value(node: &Value, layout: &LayoutIndex) -> Value {
    let found = node
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| layout.find(id));
    match found {
        Some(b) => json!({
            "x": round(b.x - b.parent_x),
            "y": round(b.y - b.parent_y),
            "width": round(b.width),
            "height": round(b.height),
        }),
        None => {
            let number = |key: &str| node.get(key).and_then(Value::as_f64).unwrap_or(0.0);
            json!({
                "x": number("x"),
                "y": number("y"),
                "width": number("width"),
                "height": number("height"),
            })
        }
    }
}

/// Two decimals: layout floats carry solver noise (`99.99999`) that makes
/// `bounds.x === 100` checks fail for no visible reason.
fn round(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}
