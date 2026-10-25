//! Website header / footer link wiring (#153).
//!
//! [`super::wire_screen_navigation`] binds app-style nav (bottom tab bars,
//! sidebars). A website's links sit elsewhere: a header or footer whose
//! items are plain text links or small buttons. This pass finds those
//! surfaces on every top-level frame and binds each item whose text
//! matches
//!
//! 1. a routed page (a top-level frame with a `screen` marker) by name →
//!    `replace` navigation to it, or else
//! 2. a section of the same page (a named frame below the page root,
//!    outside the header / footer) → `scroll_to` that section.
//!
//! Preview entry is the only caller (`de_preview_core::auto_wire`, on a
//! clone, and only when no `screen` marker is authored), so generation
//! output is unchanged. Like the parent pass it is additive only (an item
//! whose subtree already carries `events` is skipped), matches labels
//! through [`labels_match`] and adds no schema fields. Unlike it, an
//! ambiguous label binds nothing: a wrong website link is worse than a
//! dead one.
//!
//! Surfaces are found only near the page's top (a direct child of the
//! page, or of an unnamed wrapper directly under it), so a "Card Header"
//! deep inside a section never turns its title into a link.

use de_editor_core::PenNodeExt;
use jian_ops_schema::node::{PenNode, TextContent};

use super::{apply_on_tap, identity_haystack, labels_match, normalize_label, subtree_has_events};
use crate::types::DocSink;
use de_editor_core::interaction_edit::{navigate_action, scroll_to_action, NavigateVerb};
use jian_ops_schema::events::Action;

/// Roles that mark a website navigation surface.
const SURFACE_ROLES: [&str; 6] = ["header", "footer", "navbar", "nav", "navigation", "menu"];

/// Whole name / id words that mark one ("Site Header", "top-nav",
/// "Footer links"); `top nav`, `nav bar` and `menu bar` count as pairs.
const SURFACE_WORDS: [&str; 6] = [
    "header",
    "footer",
    "navbar",
    "topbar",
    "navigation",
    "menubar",
];
const SURFACE_PAIRS: [(&str, &str); 4] = [
    ("top", "nav"),
    ("nav", "bar"),
    ("top", "bar"),
    ("menu", "bar"),
];

/// The tallest first child that still reads as a header band.
const HEADER_MAX_HEIGHT: f64 = 200.0;

/// A link target: what its label is matched against, the patch value
/// (a route path for a page, a node id for a section), and its depth
/// under the page (sections only; shallower wins a tie).
struct Target {
    name: String,
    value: String,
    depth: usize,
}

/// Bind header / footer link items to their pages or sections.
pub fn wire_web_nav_links(sink: &mut dyn DocSink) {
    let ids: Vec<String> = sink
        .state()
        .active_children()
        .iter()
        .map(|node| node.id_str().to_owned())
        .collect();
    wire_web_nav_links_on(sink, &ids);
}

/// [`wire_web_nav_links`] for the top-level frames `page_ids` only: the
/// boards Preview browses (#223).
pub(crate) fn wire_web_nav_links_on(sink: &mut dyn DocSink, page_ids: &[String]) {
    let patches = {
        let roots = sink.state().active_children();
        let pages: Vec<Target> = roots
            .iter()
            .filter_map(|node| match node {
                PenNode::Frame(frame) => frame.screen.as_ref().map(|path| Target {
                    name: display_name(node),
                    value: path.clone(),
                    depth: 0,
                }),
                _ => None,
            })
            .collect();
        let mut patches = Vec::new();
        for root in roots.iter().filter(|node| {
            matches!(node, PenNode::Frame(_)) && page_ids.iter().any(|id| id == node.id_str())
        }) {
            let surfaces = collect_surfaces(root);
            if surfaces.is_empty() {
                continue;
            }
            let mut sections = Vec::new();
            for child in root.children().into_iter().flatten() {
                if !surfaces.iter().any(|surface| std::ptr::eq(*surface, child)) {
                    collect_sections(child, 1, &mut sections);
                }
            }
            for surface in surfaces {
                let mut items = Vec::new();
                collect_items(surface, &mut Vec::new(), &mut items);
                for (item, label) in items {
                    if subtree_has_events(item) {
                        continue;
                    }
                    if let Some(patch) = link_patch(label, &pages, &sections) {
                        patches.push((item.id_str().to_owned(), patch));
                    }
                }
            }
        }
        patches
    };
    for (node_id, action) in patches {
        apply_on_tap(sink, &node_id, action);
    }
}

/// The `onTap` action for a link labelled `label`: a page match wins
/// over a section match; see [`single_match`] for ties.
fn link_patch(label: &str, pages: &[Target], sections: &[Target]) -> Option<Action> {
    if let Some(path) = single_match(label, pages) {
        return Some(navigate_action(NavigateVerb::Replace, path));
    }
    let target = single_match(label, sections)?;
    Some(scroll_to_action(target))
}

/// The value of the one target `label` names. Exact (normalized) name
/// matches win over [`labels_match`]'s prefix / word matches, then the
/// shallowest wins, so a "Pricing" section beats its own "Pricing Card".
/// A tie that survives both binds nothing.
fn single_match<'a>(label: &str, targets: &'a [Target]) -> Option<&'a str> {
    let wanted = normalize_label(label);
    let exact: Vec<&Target> = targets
        .iter()
        .filter(|t| !wanted.is_empty() && normalize_label(&t.name) == wanted)
        .collect();
    let matched = if exact.is_empty() {
        targets
            .iter()
            .filter(|t| labels_match(label, &t.name))
            .collect()
    } else {
        exact
    };
    let shallowest = matched.iter().map(|t| t.depth).min()?;
    let mut best = matched.into_iter().filter(|t| t.depth == shallowest);
    let first = best.next()?;
    best.next().is_none().then_some(first.value.as_str())
}

fn display_name(node: &PenNode) -> String {
    node.base()
        .name
        .clone()
        .unwrap_or_else(|| node.id_str().to_owned())
}

fn is_surface(node: &PenNode) -> bool {
    let role = node
        .base()
        .role
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if SURFACE_ROLES.contains(&role.as_str()) {
        return true;
    }
    let hay = identity_haystack(node);
    let words: Vec<&str> = hay
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    words.iter().any(|w| SURFACE_WORDS.contains(w))
        || words
            .windows(2)
            .any(|pair| SURFACE_PAIRS.contains(&(pair[0], pair[1])))
}

/// The page's surfaces: direct children of `root` that are one, and the
/// ones directly inside an unnamed wrapper child (a page's layout column).
/// Besides names and roles, the page's structure marks them in any
/// language (#223): its first child when it is a short band holding
/// several texts (a header), and its last child holding several texts (a
/// footer).
fn collect_surfaces(root: &PenNode) -> Vec<&PenNode> {
    let mut out = Vec::new();
    let children: Vec<&PenNode> = root.children().into_iter().flatten().collect();
    let last = children.len().saturating_sub(1);
    for (index, &child) in children.iter().enumerate() {
        let header_band = index == 0
            && child.height_px().is_none_or(|h| h <= HEADER_MAX_HEIGHT)
            && text_count(child) >= 2;
        let footer_band = index == last && index > 0 && text_count(child) >= 2;
        if header_band || footer_band || is_surface(child) {
            out.push(child);
        } else if child.base().name.is_none() {
            out.extend(
                child
                    .children()
                    .into_iter()
                    .flatten()
                    .filter(|grandchild| is_surface(grandchild)),
            );
        }
    }
    out
}

/// Named frames outside any surface, with their depth under the page:
/// the page's sections.
fn collect_sections(node: &PenNode, depth: usize, out: &mut Vec<Target>) {
    if is_surface(node) {
        return;
    }
    if let PenNode::Frame(frame) = node {
        if let Some(name) = frame.base.name.as_deref().filter(|n| !n.trim().is_empty()) {
            out.push(Target {
                name: name.to_owned(),
                value: frame.base.id.clone(),
                depth,
            });
        }
    }
    for child in node.children().into_iter().flatten() {
        collect_sections(child, depth + 1, out);
    }
}

/// Each plain text inside `surface` with the node a click on it should
/// land on: the largest ancestor (below the surface) whose only text is
/// this one — the link's button frame, or the text itself for a bare link.
fn collect_items<'a>(
    node: &'a PenNode,
    stack: &mut Vec<&'a PenNode>,
    out: &mut Vec<(&'a PenNode, &'a str)>,
) {
    if let PenNode::Text(text) = node {
        if let TextContent::Plain(label) = &text.content {
            if !label.trim().is_empty() {
                let mut item = node;
                // `stack[0]` is the surface itself and never becomes an item.
                for ancestor in stack.iter().skip(1).rev() {
                    if text_count(ancestor) == 1 {
                        item = ancestor;
                    } else {
                        break;
                    }
                }
                out.push((item, label.as_str()));
            }
        }
        return;
    }
    stack.push(node);
    for child in node.children().into_iter().flatten() {
        collect_items(child, stack, out);
    }
    stack.pop();
}

fn text_count(node: &PenNode) -> usize {
    match node {
        PenNode::Text(_) => 1,
        _ => node.children().into_iter().flatten().map(text_count).sum(),
    }
}

#[cfg(test)]
#[path = "wire_web_nav_links_tests.rs"]
mod tests;
