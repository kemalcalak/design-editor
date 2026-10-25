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
/// bands a standalone screen is drawn at. The orchestrator's screen pass
/// reads the same constants.
pub const MOBILE_SCREEN_WIDTH: std::ops::RangeInclusive<f64> = 320.0..=480.0;
pub const DESKTOP_SCREEN_MIN_WIDTH: f64 = 1024.0;

/// The top-level board Preview should open on, by id.
///
/// `roots` are the previewed page's top-level nodes; `selected` is the
/// selection anchor. A selected node resolves to its top-level ancestor,
/// including a node inside a component instance (`inst__child` canvas
/// anchors and `inst/child` id paths name the instance first). With no
/// usable selection, the frame an author made the start screen (`/`)
/// wins, else the top-left-most screen-like board (smallest y, then x);
/// `None` when the page has none.
pub fn preview_entry_root(roots: &[PenNode], selected: Option<&str>) -> Option<String> {
    selected
        .filter(|id| !id.is_empty())
        .and_then(|id| top_level_board(roots, id))
        .or_else(|| start_screen(roots))
        .or_else(|| top_left_screen_board(roots))
}

fn start_screen(roots: &[PenNode]) -> Option<String> {
    roots.iter().find_map(|node| match node {
        PenNode::Frame(frame) if frame.screen.as_deref() == Some("/") => {
            Some(frame.base.id.clone())
        }
        _ => None,
    })
}

/// The top-level nodes of page `index` (a pageless document's children),
/// clamped like `EditorState::active_children`.
pub fn page_roots(doc: &jian_ops_schema::PenDocument, index: usize) -> &[PenNode] {
    match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => &pages[index.min(pages.len() - 1)].children,
        _ => &doc.children,
    }
}

/// The id of the top-level node in `roots` that is node `id` or holds it.
/// A node inside an instance is not in the tree: its canvas anchor
/// (`inst__child`) or id path (`inst/child`) names the instance first, so
/// every prefix ending at a separator is tried (an id may itself contain
/// `__`), and the instance may sit anywhere below a board.
pub fn top_level_board(roots: &[PenNode], id: &str) -> Option<String> {
    let target = crate::NodeId::new(id.to_owned());
    if let Some(root) = roots
        .iter()
        .find(|root| crate::walkers::descendant_contains(root, &target))
    {
        return Some(root.id_str().to_owned());
    }
    let mut cuts: Vec<usize> = id
        .match_indices(crate::ref_resolve::INSTANCE_CHILD_SEPARATOR)
        .map(|(at, _)| at)
        .chain(
            id.match_indices(crate::instance_descendants::PATH_SEPARATOR)
                .map(|(at, _)| at),
        )
        .filter(|&at| at > 0)
        .collect();
    cuts.sort_unstable();
    cuts.into_iter()
        .find_map(|at| top_level_board(roots, &id[..at]))
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
        let same_site = match kind.lead_component {
            Some(lead) => other.lead_component == Some(lead),
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
    /// The component the board's first child instantiates: on a website,
    /// the header every page of the site shares.
    lead_component: Option<&'a str>,
    screen_like: bool,
}

/// A top-level frame's shape, or an instance's: its master's, with the
/// size the instance overrides on the master's root.
fn board_shape<'a>(roots: &'a [PenNode], node: &'a PenNode) -> Option<BoardShape<'a>> {
    let (frame, root_override) = match node {
        PenNode::Frame(_) => (node, None),
        PenNode::Ref(r) => (
            // The master: a top-level board, or a frame anywhere on the
            // page (a components board).
            crate::walkers::find_node(roots, &crate::NodeId::new(r.target.clone()))
                .filter(|master| matches!(master, PenNode::Frame(_)))?,
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
    let in_band = MOBILE_SCREEN_WIDTH.contains(&width) || width >= DESKTOP_SCREEN_MIN_WIDTH;
    Some(BoardShape {
        width,
        instance: matches!(node, PenNode::Ref(_)),
        theme: node.base().theme.as_ref(),
        lead_component: match children.first() {
            Some(PenNode::Ref(lead)) => Some(lead.target.as_str()),
            _ => None,
        },
        screen_like: in_band && height.is_some_and(|h| h > 0.0) && !holds_masters && !text_only,
    })
}

#[cfg(test)]
#[path = "preview_screen_set_tests.rs"]
mod tests;
