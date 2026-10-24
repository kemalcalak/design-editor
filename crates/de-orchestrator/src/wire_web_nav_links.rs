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
//! output is unchanged. It inherits the parent pass's contract: additive
//! only (an item whose subtree already carries `events` is skipped),
//! label matching through [`labels_match`], ambiguity never binds, and no
//! new schema fields.

use de_editor_core::{EditorCommand, NodeId, PenNodeExt};
use jian_ops_schema::node::{PenNode, TextContent};

use super::{labels_match, navigate_patch, subtree_has_events};
use crate::types::DocSink;

/// Roles that mark a website navigation surface.
const SURFACE_ROLES: [&str; 6] = ["header", "footer", "navbar", "nav", "navigation", "menu"];

/// Name / id fragments that mark one (compared lowercased, separators
/// kept, so "Site Header", "top-nav" and "Footer links" all count).
const SURFACE_NAMES: [&str; 10] = [
    "header",
    "footer",
    "navbar",
    "nav bar",
    "nav-bar",
    "top nav",
    "top-nav",
    "topbar",
    "top bar",
    "navigation",
];

/// Bind header / footer link items to their pages or sections.
pub fn wire_web_nav_links(sink: &mut dyn DocSink) {
    let patches = {
        let roots = sink.state().active_children();
        let pages: Vec<(String, String)> = roots
            .iter()
            .filter_map(|node| match node {
                PenNode::Frame(frame) => frame
                    .screen
                    .as_ref()
                    .map(|path| (display_name(node), path.clone())),
                _ => None,
            })
            .collect();
        let mut patches = Vec::new();
        for root in roots
            .iter()
            .filter(|node| matches!(node, PenNode::Frame(_)))
        {
            let mut surfaces = Vec::new();
            collect_surfaces(root, &mut surfaces);
            if surfaces.is_empty() {
                continue;
            }
            let mut sections = Vec::new();
            for child in root.children().into_iter().flatten() {
                collect_sections(child, &mut sections);
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
    for (node_id, patch_json) in patches {
        sink.apply(EditorCommand::PatchNodeData {
            node_id: NodeId::new(node_id),
            patch_json,
            page_id: None,
        });
    }
}

/// The patch for a link labelled `label`: a page match wins over a
/// section match; two matching candidates of the same kind bind nothing.
fn link_patch(
    label: &str,
    pages: &[(String, String)],
    sections: &[(String, String)],
) -> Option<String> {
    if let Some(path) = single_match(label, pages) {
        return Some(navigate_patch("replace", path));
    }
    let target = single_match(label, sections)?;
    let patch = serde_json::json!({
        "events": { "onTap": [ { "scroll_to": { "target": target, "alignment": "start" } } ] }
    });
    Some(patch.to_string())
}

fn single_match<'a>(label: &str, candidates: &'a [(String, String)]) -> Option<&'a str> {
    let mut found = candidates
        .iter()
        .filter(|(name, _)| labels_match(label, name));
    let first = found.next()?;
    found.next().is_none().then_some(first.1.as_str())
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
    let hay = format!(
        "{} {}",
        node.id_str().to_ascii_lowercase(),
        node.base()
            .name
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase()
    );
    SURFACE_NAMES.iter().any(|needle| hay.contains(needle))
}

/// Outermost surfaces under `node`; a surface's own nested surfaces are
/// part of it.
fn collect_surfaces<'a>(node: &'a PenNode, out: &mut Vec<&'a PenNode>) {
    for child in node.children().into_iter().flatten() {
        if is_surface(child) {
            out.push(child);
        } else {
            collect_surfaces(child, out);
        }
    }
}

/// Named frames outside any surface: the page's sections.
fn collect_sections(node: &PenNode, out: &mut Vec<(String, String)>) {
    if is_surface(node) {
        return;
    }
    if let PenNode::Frame(frame) = node {
        if let Some(name) = frame.base.name.as_deref().filter(|n| !n.trim().is_empty()) {
            out.push((name.to_owned(), frame.base.id.clone()));
        }
    }
    for child in node.children().into_iter().flatten() {
        collect_sections(child, out);
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
