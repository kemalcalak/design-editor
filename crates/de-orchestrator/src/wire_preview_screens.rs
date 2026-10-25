//! Preview-entry screen wiring (#223).
//!
//! Generation marks screens by shape ([`super::wire_screen_navigation`]);
//! Preview already knows which boards it browses: the entry board and the
//! boards of its kind (`de_editor_core::preview_screen_set`). This pass
//! routes exactly those, gives the entry `/` when no page of the document
//! has one, names the other routes like the panel's "Mark as screen"
//! (`de_editor_core::screen_marker_edit::unique_route_path`, any script)
//! and binds their tab bars, headers and footers. Like the generation pass it is
//! additive only: an authored `screen` marker or `events` block is never
//! touched. Boards outside the site keep any authored route but are not
//! link targets, so a same-named page of another site (the mobile "About"
//! beside the desktop one) cannot turn a link ambiguous.
//!
//! Caller: `de_preview_core::auto_wire`, on a preview-only clone whose
//! component instances are already expanded, so links inside a shared
//! header instance are ordinary nodes there.

use std::collections::BTreeSet;

use de_editor_core::screen_marker_edit::unique_route_path;
use de_editor_core::{EditorCommand, NodeId};
use jian_ops_schema::node::PenNode;

use super::wire_web_nav_links::wire_web_nav_links_on;
use super::{wire_nav_tabs, ScreenCandidate};
use crate::types::DocSink;

/// Route `screen_ids` (entry first), then bind their header / footer
/// links. A lone board is not routed (it previews as a page), but its
/// section links are still bound.
pub fn wire_preview_screens(sink: &mut dyn DocSink, screen_ids: &[String]) {
    let site: Vec<(String, String, Option<String>)> = sink
        .state()
        .active_children()
        .iter()
        .filter_map(|node| match node {
            PenNode::Frame(frame) if screen_ids.iter().any(|id| id == &frame.base.id) => Some((
                frame.base.id.clone(),
                frame.base.name.clone().unwrap_or_default(),
                frame.screen.clone(),
            )),
            _ => None,
        })
        .collect();
    if site.len() >= 2 {
        // Every route of the document: screen projection reads every page,
        // and a later duplicate path is dropped.
        let mut used = document_routes(sink);
        let entry = screen_ids.first();
        let mut patches = Vec::new();
        for (id, name, existing) in &site {
            if existing.is_some() {
                continue;
            }
            let path = if Some(id) == entry && used.insert("/".to_owned()) {
                "/".to_owned()
            } else {
                unique_route_path(name, &mut used)
            };
            patches.push((id.clone(), path));
        }
        for (node_id, path) in patches {
            sink.apply(EditorCommand::PatchNodeData {
                node_id: NodeId::new(node_id),
                patch_json: serde_json::json!({ "screen": path }).to_string(),
                page_id: None,
            });
        }
        // App-style tab bars and sidebars, the generation pass's binding.
        let screens: Vec<ScreenCandidate> = sink
            .state()
            .active_children()
            .iter()
            .filter_map(|node| match node {
                PenNode::Frame(frame) if site.iter().any(|(id, ..)| id == &frame.base.id) => {
                    Some(ScreenCandidate {
                        id: frame.base.id.clone(),
                        name: frame
                            .base
                            .name
                            .clone()
                            .unwrap_or_else(|| frame.base.id.clone()),
                        existing_path: frame.screen.clone(),
                    })
                }
                _ => None,
            })
            .collect();
        let screen_paths: Vec<(String, String)> = screens
            .iter()
            .filter_map(|s| Some((s.name.clone(), s.existing_path.clone()?)))
            .collect();
        wire_nav_tabs(sink, &screens, &screen_paths);
    }
    let pages: Vec<String> = if site.len() >= 2 {
        site.into_iter().map(|(id, _, _)| id).collect()
    } else {
        screen_ids.iter().take(1).cloned().collect()
    };
    wire_web_nav_links_on(sink, &pages);
}

/// The `screen` paths of every top-level frame on every page.
fn document_routes(sink: &dyn DocSink) -> BTreeSet<String> {
    let doc = &sink.state().doc;
    let roots: Vec<&PenNode> = match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages.iter().flat_map(|p| &p.children).collect(),
        _ => doc.children.iter().collect(),
    };
    roots
        .into_iter()
        .filter_map(|node| match node {
            PenNode::Frame(frame) => frame.screen.clone(),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "wire_preview_screens_tests.rs"]
mod tests;
