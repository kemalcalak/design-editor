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
//! clone; through `wire_preview_screens` for the boards Preview browses),
//! so generation output is unchanged. Like the parent pass it is additive only (an item
//! whose subtree already carries `events` is skipped), matches labels
//! through `crate::label_match` and adds no schema fields. Unlike it, an
//! ambiguous label binds nothing: a wrong website link is worse than a
//! dead one.
//!
//! Surfaces are found only at the page's top level (a direct child of the
//! page, or of an unnamed wrapper directly under it), so a "Card Header"
//! deep inside a section never turns its title into a link.

use de_editor_core::PenNodeExt;
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::{PenNode, TextContent};

use super::{apply_on_tap, identity_haystack, subtree_has_events};
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
/// The tallest last child that still reads as a footer band.
const FOOTER_MAX_HEIGHT: f64 = 480.0;
/// The longest text a link band holds, in characters: link labels are
/// short; a heading or a paragraph marks a content section instead.
const LINK_TEXT_MAX_CHARS: usize = 40;

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

/// [`wire_web_nav_links`] for the top-level frames `page_ids` only, as the
/// pages links are bound on and as the pages they may lead to: the boards
/// Preview browses (#223).
pub(crate) fn wire_web_nav_links_on(sink: &mut dyn DocSink, page_ids: &[String]) {
    let patches = {
        let roots = sink.state().active_children();
        let pages: Vec<Target> = roots
            .iter()
            .filter(|node| page_ids.iter().any(|id| id == node.id_str()))
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

/// The `onTap` action for a link labelled `label`: the target named most
/// strongly (`crate::label_match`, any language); among equally strong
/// ones a page beats a section and a shallower section a deeper one (a
/// "Pricing" section beats its own "Pricing Card"). A tie that survives
/// binds nothing: a wrong website link is worse than a dead one.
fn link_patch(label: &str, pages: &[Target], sections: &[Target]) -> Option<Action> {
    enum Link<'a> {
        Page(&'a str),
        Section(&'a str),
    }
    let targets = pages
        .iter()
        .map(|t| (t.name.as_str(), 0, Link::Page(&t.value)))
        .chain(
            sections
                .iter()
                .map(|t| (t.name.as_str(), 1 + t.depth, Link::Section(&t.value))),
        );
    Some(match crate::label_match::best_match(label, targets)? {
        Link::Page(path) => navigate_action(NavigateVerb::Replace, path),
        Link::Section(id) => scroll_to_action(id),
    })
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
/// language (#223): its first child holding only short texts, at most
/// `HEADER_MAX_HEIGHT` tall or, without an authored height, laid out as a
/// row (a header); and its last child holding only short texts, at most
/// `FOOTER_MAX_HEIGHT` tall when its height is authored (a footer). A
/// heading or paragraph marks a page column, hero or content section.
fn collect_surfaces(root: &PenNode) -> Vec<&PenNode> {
    let mut out = Vec::new();
    let children: Vec<&PenNode> = root.children().into_iter().flatten().collect();
    let last = children.len().saturating_sub(1);
    for (index, &child) in children.iter().enumerate() {
        let header_band = index == 0
            && match child.height_px() {
                Some(height) => height <= HEADER_MAX_HEIGHT,
                None => is_row(child),
            }
            && is_link_band(child);
        let footer_band = index == last
            && index > 0
            && child.height_px().is_none_or(|h| h <= FOOTER_MAX_HEIGHT)
            && is_link_band(child);
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

fn is_row(node: &PenNode) -> bool {
    matches!(
        node,
        PenNode::Frame(frame) if frame.container.layout == Some(LayoutMode::Horizontal)
    )
}

/// Several texts, all of them short: a row of links, not content.
fn is_link_band(node: &PenNode) -> bool {
    let mut texts = Vec::new();
    collect_plain_texts(node, &mut texts);
    texts.len() >= 2
        && texts
            .iter()
            .all(|text| text.trim().chars().count() <= LINK_TEXT_MAX_CHARS)
}

fn collect_plain_texts<'a>(node: &'a PenNode, out: &mut Vec<&'a str>) {
    if let PenNode::Text(text) = node {
        if let TextContent::Plain(label) = &text.content {
            out.push(label.as_str());
        }
        return;
    }
    for child in node.children().into_iter().flatten() {
        collect_plain_texts(child, out);
    }
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
