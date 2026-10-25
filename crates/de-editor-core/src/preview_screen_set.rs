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

/// The top-level nodes of page `index` (a pageless document's children).
pub fn page_roots(doc: &jian_ops_schema::PenDocument, index: usize) -> &[PenNode] {
    match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages
            .get(index)
            .map(|page| page.children.as_slice())
            .unwrap_or_default(),
        _ => &doc.children,
    }
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

/// The boards Preview browses from `entry_root_id`, entry first, then in
/// document order: every top-level board of the entry's kind. Same kind
/// means an instance or not like the entry, the same width, for an
/// instance the same `theme` (light and dark variants of one page are
/// different sites), and when the entry opens with a component instance
/// (a shared header) the same component; without one, the others must be
/// screen-like ([`is_screen_like_board`]). Empty when the entry is not a
/// top-level board.
pub fn preview_screen_set(roots: &[PenNode], entry_root_id: &str) -> Vec<String> {
    let Some(entry) = roots.iter().find(|node| node.id_str() == entry_root_id) else {
        return Vec::new();
    };
    let mut out = vec![entry_root_id.to_owned()];
    let Some(kind) = board_shape(roots, entry) else {
        return out;
    };
    for node in roots.iter().filter(|node| node.id_str() != entry_root_id) {
        let Some(other) = board_shape(roots, node) else {
            continue;
        };
        let same_kind = other.instance == kind.instance
            && (other.width - kind.width).abs() < 0.5
            && (!kind.instance || other.theme == kind.theme);
        let same_site = match kind.header {
            Some(header) => other.header == Some(header),
            None => other.screen_like,
        };
        if same_kind && same_site {
            out.push(node.id_str().to_owned());
        }
    }
    out
}

/// A top-level frame (or an instance of one) at a screen width with a
/// height, that is neither a components board (it holds reusable
/// masters) nor a note board (its direct children are all text).
pub fn is_screen_like_board(roots: &[PenNode], node: &PenNode) -> bool {
    board_shape(roots, node).is_some_and(|shape| shape.screen_like)
}

/// What decides which boards belong together.
struct BoardShape<'a> {
    width: f64,
    instance: bool,
    theme: Option<&'a std::collections::BTreeMap<String, String>>,
    /// The component the board's first child instantiates: a header
    /// shared by every page of one site.
    header: Option<&'a str>,
    screen_like: bool,
}

/// A top-level frame's shape, or an instance's: its master's, with the
/// size the instance overrides on the master's root.
fn board_shape<'a>(roots: &'a [PenNode], node: &'a PenNode) -> Option<BoardShape<'a>> {
    let (frame, root_override) = match node {
        PenNode::Frame(_) => (node, None),
        PenNode::Ref(r) => (
            roots
                .iter()
                .find(|root| matches!(root, PenNode::Frame(_)) && root.id_str() == r.target)?,
            r.descendants.as_ref().and_then(|d| d.get(&r.target)),
        ),
        _ => return None,
    };
    let size = |key: &str, own: Option<f64>| {
        root_override
            .and_then(|patch| patch.get(key))
            .and_then(serde_json::Value::as_f64)
            .or(own)
    };
    let width = size("width", frame.width_px())?;
    let height = size("height", frame.height_px());
    let children: Vec<&PenNode> = frame.children().into_iter().flatten().collect();
    let holds_masters = children
        .iter()
        .any(|child| matches!(child, PenNode::Frame(f) if f.reusable == Some(true)));
    let text_only = !children.is_empty()
        && children
            .iter()
            .all(|child| matches!(child, PenNode::Text(_)));
    let in_band = MOBILE_WIDTH.contains(&width) || width >= DESKTOP_MIN_WIDTH;
    Some(BoardShape {
        width,
        instance: matches!(node, PenNode::Ref(_)),
        theme: node.base().theme.as_ref(),
        header: match children.first() {
            Some(PenNode::Ref(header)) => Some(header.target.as_str()),
            _ => None,
        },
        screen_like: in_band && height.is_some_and(|h| h > 0.0) && !holds_masters && !text_only,
    })
}

#[cfg(test)]
#[path = "preview_screen_set_tests.rs"]
mod tests;
