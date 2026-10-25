//! Which board Preview opens on (#223).
//!
//! Preview starts from the board the person is working on: the top-level
//! board holding the selection, or with nothing selected the top-left-most
//! board that looks like a screen. Every rule here reads the document's
//! structure (sizes, positions, component flags), never a name, so it
//! holds for every design and every language.
//!
//! Pure and wasm32-clean: the desktop host, the web host and MCP
//! `capture_preview` all ask the same question.

use jian_ops_schema::node::PenNode;

use crate::PenNodeExt;

/// Phone / narrow-tablet portrait widths, and desktop-and-up widths: the
/// bands a standalone screen is drawn at (the orchestrator's screen pass
/// uses the same bands).
const MOBILE_WIDTH: std::ops::RangeInclusive<f64> = 320.0..=480.0;
const DESKTOP_MIN_WIDTH: f64 = 1024.0;

/// The top-level board Preview should open on, by id.
///
/// `roots` are the previewed page's top-level nodes; `selected` is the
/// selection anchor. A selected node resolves to its top-level ancestor,
/// including a node inside a component instance (`inst__child` canvas
/// anchors and `inst/child` id paths name the instance first). With no
/// usable selection, the top-left-most screen-like board wins (smallest
/// y, then x); `None` when the page has none.
pub fn preview_entry_root(roots: &[PenNode], selected: Option<&str>) -> Option<String> {
    selected
        .filter(|id| !id.is_empty())
        .and_then(|id| top_level_ancestor(roots, id))
        .or_else(|| top_left_screen_board(roots))
}

/// The id of the top-level node in `roots` that is `id` or contains it.
fn top_level_ancestor(roots: &[PenNode], id: &str) -> Option<String> {
    if let Some(root) = roots.iter().find(|root| subtree_contains(root, id)) {
        return Some(root.id_str().to_owned());
    }
    // A node inside an instance is not in the tree: its id names the
    // instance first, which may itself sit anywhere below a board.
    let (head, _) = id.split_once("__").or_else(|| id.split_once('/'))?;
    top_level_ancestor(roots, head)
}

fn subtree_contains(node: &PenNode, id: &str) -> bool {
    node.id_str() == id
        || node
            .children()
            .into_iter()
            .flatten()
            .any(|child| subtree_contains(child, id))
}

fn top_left_screen_board(roots: &[PenNode]) -> Option<String> {
    roots
        .iter()
        .filter(|node| is_screen_like_board(roots, node))
        .min_by(|a, b| {
            let (ax, ay) = position(a);
            let (bx, by) = position(b);
            ay.total_cmp(&by).then(ax.total_cmp(&bx))
        })
        .map(|node| node.id_str().to_owned())
}

fn position(node: &PenNode) -> (f64, f64) {
    let base = node.base();
    (base.x.unwrap_or(0.0), base.y.unwrap_or(0.0))
}

/// A top-level frame (or an instance of one) at a screen width with a
/// height, that is neither a components board (it holds reusable
/// masters) nor a note board (its direct children are all text).
pub fn is_screen_like_board(roots: &[PenNode], node: &PenNode) -> bool {
    let Some(frame) = board_frame(roots, node) else {
        return false;
    };
    let (Some(width), Some(height)) = (frame.width_px(), frame.height_px()) else {
        return false;
    };
    if height <= 0.0 || !(MOBILE_WIDTH.contains(&width) || width >= DESKTOP_MIN_WIDTH) {
        return false;
    }
    let children: Vec<&PenNode> = frame.children().into_iter().flatten().collect();
    let holds_masters = children
        .iter()
        .any(|child| matches!(child, PenNode::Frame(f) if f.reusable == Some(true)));
    let text_only = !children.is_empty()
        && children
            .iter()
            .all(|child| matches!(child, PenNode::Text(_)));
    !holds_masters && !text_only
}

/// The frame that gives a board its shape: the board itself, or for an
/// instance the top-level master it points at.
fn board_frame<'a>(roots: &'a [PenNode], node: &'a PenNode) -> Option<&'a PenNode> {
    match node {
        PenNode::Frame(_) => Some(node),
        PenNode::Ref(r) => roots
            .iter()
            .find(|root| matches!(root, PenNode::Frame(_)) && root.id_str() == r.target),
        _ => None,
    }
}

#[cfg(test)]
#[path = "preview_screen_set_tests.rs"]
mod tests;
