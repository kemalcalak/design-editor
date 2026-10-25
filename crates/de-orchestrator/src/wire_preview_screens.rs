//! Preview-entry screen wiring (#223).
//!
//! Generation marks screens by shape ([`super::wire_screen_navigation`]);
//! Preview already knows which boards it browses: the entry board and the
//! boards of its kind (`de_editor_core::preview_screen_set`). This pass
//! routes exactly those, gives the entry `/`, and binds their tab bars,
//! headers and footers. Like the generation pass it is additive only: an
//! authored `screen` marker or `events` block is never touched, and boards
//! an author marked stay routed and stay link targets.
//!
//! Caller: `de_preview_core::auto_wire`, on a preview-only clone whose
//! component instances are already expanded, so links inside a shared
//! header instance are ordinary nodes there.

use std::collections::BTreeSet;

use de_editor_core::{EditorCommand, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;

use super::wire_web_nav_links::wire_web_nav_links_on;
use super::{assign_screen_paths, wire_nav_tabs, ScreenCandidate};
use crate::types::DocSink;

/// Route `screen_ids` (entry first) plus every authored screen, then bind
/// their navigation. With fewer than two routed boards nothing is routed
/// (a single board previews as a page), but its header / footer section
/// links are still bound.
pub fn wire_preview_screens(sink: &mut dyn DocSink, screen_ids: &[String]) {
    let wanted: BTreeSet<&str> = screen_ids.iter().map(String::as_str).collect();
    let mut candidates = preview_candidates(sink, &wanted);
    if candidates.len() >= 2 {
        let entry = screen_ids.first().map(String::as_str);
        let mut patches = Vec::new();
        let root_taken = candidates
            .iter()
            .any(|c| c.existing_path.as_deref() == Some("/"));
        if let Some(entry) = candidates
            .iter_mut()
            .find(|c| Some(c.id.as_str()) == entry && c.existing_path.is_none())
            .filter(|_| !root_taken)
        {
            entry.existing_path = Some("/".to_owned());
            patches.push((entry.id.clone(), "/".to_owned()));
        }
        patches.extend(assign_screen_paths(&candidates));
        for (node_id, path) in patches {
            sink.apply(EditorCommand::PatchNodeData {
                node_id: NodeId::new(node_id),
                patch_json: format!(r#"{{"screen":"{path}"}}"#),
                page_id: None,
            });
        }
        candidates = preview_candidates(sink, &wanted);
        let screen_paths: Vec<(String, String)> = candidates
            .iter()
            .filter_map(|c| Some((c.name.clone(), c.existing_path.clone()?)))
            .collect();
        wire_nav_tabs(sink, &candidates, &screen_paths);
    }
    // A lone board is not routed, but its section links still work.
    let pages: Vec<String> = if candidates.len() >= 2 {
        candidates.into_iter().map(|c| c.id).collect()
    } else {
        screen_ids.iter().take(1).cloned().collect()
    };
    wire_web_nav_links_on(sink, &pages);
}

/// The top-level frames in `wanted` or carrying an authored marker, in
/// document order.
fn preview_candidates(sink: &dyn DocSink, wanted: &BTreeSet<&str>) -> Vec<ScreenCandidate> {
    sink.state()
        .active_children()
        .iter()
        .filter_map(|node| {
            let PenNode::Frame(frame) = node else {
                return None;
            };
            (wanted.contains(node.id_str()) || frame.screen.is_some()).then(|| ScreenCandidate {
                id: frame.base.id.clone(),
                name: frame
                    .base
                    .name
                    .clone()
                    .unwrap_or_else(|| frame.base.id.clone()),
                existing_path: frame.screen.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "wire_preview_screens_tests.rs"]
mod tests;
