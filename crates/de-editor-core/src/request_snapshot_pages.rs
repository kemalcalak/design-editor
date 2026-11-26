//! Which pages a request snapshot needs (#456).
//!
//! A live MCP read that looks at one page used to get every page: the UI
//! thread compared the whole document against its anchor and the connection
//! copied all of it (~100% of the document per call on a 14-page file). A
//! [`SnapshotPages`] names what the call reads; [`kept_pages`] turns it into
//! the pages the capture keeps, adding every page an instance on them may
//! need: pages holding a component master, and pages holding the target of
//! any `ref` in the document (an instance's own target, a `ref` inside its
//! `descendants`, and any string an instance passes as a property). The
//! other pages keep their metadata in the snapshot and carry no nodes.
//!
//! What a tool reads is decided by its caller (`de_mcp::snapshot_pages`);
//! anything it does not list gets [`SnapshotPages::All`], the old snapshot.

use std::collections::HashSet;

use jian_ops_schema::node::PenNode;
use serde_json::Value;

use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;

/// The part of the document a request reads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SnapshotPages {
    /// Every page: writes, and reads that look across pages.
    #[default]
    All,
    /// No page's nodes: the call reads document-level state only (history
    /// depth, viewport, variables, themes).
    NoNodes,
    /// One page, by id (`None`: the active page).
    Page(Option<String>),
}

/// The pages every partial snapshot keeps, for one document revision:
/// pages holding a component master or the target of a `ref`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageNeeds {
    generation: u64,
    revision: u64,
    always: Vec<bool>,
}

impl PageNeeds {
    /// Walk `state`'s pages once to find what instances may resolve against.
    pub fn of(state: &EditorState) -> Self {
        let pages = state.doc.pages.as_deref().unwrap_or_default();
        let mut targets: HashSet<&str> = HashSet::new();
        let mut always: Vec<bool> = pages
            .iter()
            .map(|page| {
                let mut holds_master = false;
                scan(&page.children, &mut holds_master, &mut targets);
                holds_master
            })
            .collect();
        if !targets.is_empty() {
            for (index, page) in pages.iter().enumerate() {
                if !always[index] && holds_any(&page.children, &targets) {
                    always[index] = true;
                }
            }
        }
        Self {
            generation: state.document_generation(),
            revision: state.document_revision(),
            always,
        }
    }

    /// Whether these needs were taken from `state`'s current document.
    pub fn is_current(&self, state: &EditorState) -> bool {
        self.generation == state.document_generation() && self.revision == state.document_revision()
    }
}

/// Which pages to capture for `pages` (`None`: all of them). A page id the
/// document does not hold captures everything, so the tool answers exactly
/// as it did before (its own "page not found").
pub fn kept_pages(
    state: &EditorState,
    pages: &SnapshotPages,
    needs: &PageNeeds,
) -> Option<Vec<bool>> {
    let doc_pages = state
        .doc
        .pages
        .as_deref()
        .filter(|pages| !pages.is_empty())?;
    if needs.always.len() != doc_pages.len() {
        return None;
    }
    match pages {
        SnapshotPages::All => None,
        SnapshotPages::NoNodes => Some(vec![false; doc_pages.len()]),
        SnapshotPages::Page(id) => {
            let index = match id {
                Some(id) => doc_pages.iter().position(|page| page.id == *id)?,
                None => state.ui.active_page_index.min(doc_pages.len() - 1),
            };
            let mut keep = needs.always.clone();
            keep[index] = true;
            Some(keep)
        }
    }
}

/// Marks a master and collects every string an instance may resolve.
fn scan<'a>(nodes: &'a [PenNode], holds_master: &mut bool, targets: &mut HashSet<&'a str>) {
    for node in nodes {
        if crate::component_registration::reusable_component_name(node).is_some() {
            *holds_master = true;
        }
        if let PenNode::Ref(reference) = node {
            targets.insert(reference.target.as_str());
            for value in reference.descendants.iter().flat_map(|d| d.values()) {
                collect_ref_values(value, targets);
            }
            for value in reference.props.iter().flat_map(|p| p.values()) {
                collect_strings(value, targets);
            }
        }
        if let Some(children) = node.children() {
            scan(children, holds_master, targets);
        }
    }
}

/// `ref` targets inside a `descendants` override (a replacement or slot
/// content can hold instances of its own).
fn collect_ref_values<'a>(value: &'a Value, targets: &mut HashSet<&'a str>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(target)) = map.get("ref") {
                targets.insert(target.as_str());
            }
            for value in map.values() {
                collect_ref_values(value, targets);
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_ref_values(value, targets);
            }
        }
        _ => {}
    }
}

/// Every string in an instance property: an `instance` property names a
/// component by id, and other kinds are kept out cheaply by the id check.
fn collect_strings<'a>(value: &'a Value, targets: &mut HashSet<&'a str>) {
    match value {
        Value::String(text) => {
            targets.insert(text.as_str());
        }
        Value::Object(map) => {
            for value in map.values() {
                collect_strings(value, targets);
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_strings(value, targets);
            }
        }
        _ => {}
    }
}

fn holds_any(nodes: &[PenNode], targets: &HashSet<&str>) -> bool {
    nodes.iter().any(|node| {
        targets.contains(node.id_str()) || node.children().is_some_and(|c| holds_any(c, targets))
    })
}

#[cfg(test)]
#[path = "request_snapshot_pages_tests.rs"]
mod tests;
