//! Fingerprint every frame subtree in scope and group the ones that share a
//! structure.

use std::collections::{HashMap, HashSet};

use jian_ops_schema::PenDocument;
use serde_json::Value;

use super::diff::{children, node_id, node_type, projection, Fnv};
use super::{RepeatedGroup, GROUP_ID_PREFIX};
use crate::page_mutators::is_components_page;
use crate::{ComponentLibrary, NodeId};

/// One node of the scanned page trees.
struct Entry {
    id: String,
    fingerprint: u64,
    count: usize,
    page: Option<usize>,
    parent: Option<usize>,
    /// A frame that may become a master or an instance.
    eligible: bool,
    name: Option<String>,
}

struct Context<'a> {
    masters: HashSet<&'a str>,
    ref_targets: HashSet<String>,
}

/// The page trees in scope: one page (by index), else every page but the
/// `Components` page; a document without pages is its root list.
fn scope_roots(
    doc: &PenDocument,
    page: Option<usize>,
) -> Vec<(Option<usize>, &[jian_ops_schema::node::PenNode])> {
    match doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages
            .iter()
            .enumerate()
            .filter(|(index, candidate)| {
                page.map_or(!is_components_page(candidate), |page| page == *index)
            })
            .map(|(index, candidate)| (Some(index), candidate.children.as_slice()))
            .collect(),
        _ if page.is_none_or(|page| page == 0) => vec![(None, doc.children.as_slice())],
        _ => Vec::new(),
    }
}

/// Every `ref` target in the document, overrides included.
pub(super) fn ref_targets(value: &Value, out: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("ref") {
                if let Some(target) = map.get("ref").and_then(Value::as_str) {
                    out.insert(target.to_string());
                }
            }
            for item in map.values() {
                ref_targets(item, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                ref_targets(item, out);
            }
        }
        _ => {}
    }
}

/// Whether a node is a component master (a reusable frame or a registered
/// component root).
pub(super) fn is_master(node: &Value, masters: &HashSet<&str>) -> bool {
    (node_type(node) == "frame" && node.get("reusable") == Some(&Value::Bool(true)))
        || masters.contains(node_id(node))
}

/// Fingerprint `node` and record it (and its subtree) in `entries`.
/// Returns the fingerprint, the subtree's node count and whether it holds a
/// master. A master is hashed but not recorded: neither it nor anything
/// inside it can be converted, and a frame holding one cannot either.
fn walk(
    node: &Value,
    page: Option<usize>,
    parent: Option<usize>,
    cx: &Context<'_>,
    entries: &mut Vec<Entry>,
) -> (u64, usize, bool) {
    let mut hash = Fnv::new();
    hash.write(Value::Object(projection(node)).to_string().as_bytes());
    if is_master(node, &cx.masters) {
        hash.write(node_id(node).as_bytes());
        return (hash.finish(), 1, true);
    }
    let index = entries.len();
    entries.push(Entry {
        id: node_id(node).to_string(),
        fingerprint: 0,
        count: 0,
        page,
        parent,
        eligible: false,
        name: node.get("name").and_then(Value::as_str).map(str::to_owned),
    });
    let mut count = 1;
    let mut holds_master = false;
    // An instance's content is not part of the page tree: its projection
    // already carries its slot children by value.
    if node_type(node) != "ref" {
        for child in children(node) {
            let (child_hash, child_count, child_master) =
                walk(child, page, Some(index), cx, entries);
            hash.write_u64(child_hash);
            count += child_count;
            holds_master |= child_master;
        }
    }
    let entry = &mut entries[index];
    entry.fingerprint = hash.finish();
    entry.count = count;
    entry.eligible =
        node_type(node) == "frame" && !holds_master && !cx.ref_targets.contains(node_id(node));
    (entry.fingerprint, count, holds_master)
}

fn scan(doc: &PenDocument, components: &ComponentLibrary, page: Option<usize>) -> Vec<Entry> {
    let mut ref_target_set = HashSet::new();
    if let Ok(value) = serde_json::to_value(doc) {
        ref_targets(&value, &mut ref_target_set);
    }
    let cx = Context {
        masters: components
            .components
            .iter()
            .map(|c| c.id.as_str())
            .collect(),
        ref_targets: ref_target_set,
    };
    let mut entries = Vec::new();
    for (page, roots) in scope_roots(doc, page) {
        let Ok(Value::Array(roots)) = serde_json::to_value(roots) else {
            continue;
        };
        for root in &roots {
            walk(root, page, None, &cx, &mut entries);
        }
    }
    entries
}

fn group_id(fingerprint: u64) -> String {
    format!("{GROUP_ID_PREFIX}{fingerprint:016x}")
}

/// Groups of at least two frames of at least `min_nodes` nodes sharing a
/// structure, on page `page` (index) or every page but `Components`.
/// Larger units claim first: a smaller group keeps only the members outside
/// the claimed ones and is reported when two or more are left.
pub fn find_repeated_structures(
    doc: &PenDocument,
    components: &ComponentLibrary,
    page: Option<usize>,
    min_nodes: usize,
) -> Vec<RepeatedGroup> {
    let entries = scan(doc, components, page);
    let mut by_fingerprint: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.eligible && entry.count >= min_nodes.max(1) {
            by_fingerprint
                .entry(entry.fingerprint)
                .or_default()
                .push(index);
        }
    }
    let mut candidates: Vec<Vec<usize>> = by_fingerprint
        .into_values()
        .filter(|members| members.len() >= 2)
        .collect();
    candidates.sort_by_key(|members| (std::cmp::Reverse(entries[members[0]].count), members[0]));
    let mut claimed: HashSet<usize> = HashSet::new();
    let inside_claimed = |index: usize, claimed: &HashSet<usize>| {
        let mut parent = entries[index].parent;
        while let Some(at) = parent {
            if claimed.contains(&at) {
                return true;
            }
            parent = entries[at].parent;
        }
        false
    };
    let mut groups = Vec::new();
    for members in candidates {
        let free: Vec<usize> = members
            .into_iter()
            .filter(|index| !inside_claimed(*index, &claimed))
            .collect();
        if free.len() < 2 {
            continue;
        }
        claimed.extend(free.iter().copied());
        let first = &entries[free[0]];
        groups.push(RepeatedGroup {
            group_id: group_id(first.fingerprint),
            node_ids: free
                .iter()
                .map(|i| NodeId::new(entries[*i].id.as_str()))
                .collect(),
            pages: free.iter().map(|i| entries[*i].page).collect(),
            nodes_per_member: first.count,
            name: first.name.clone(),
        });
    }
    // Most nodes saved first.
    groups.sort_by_key(|group| {
        std::cmp::Reverse((group.node_ids.len() - 1) * group.nodes_per_member)
    });
    groups
}

/// The members of `group_id` in scope (any size), `None` when no such group
/// exists there.
pub fn group_members(
    doc: &PenDocument,
    components: &ComponentLibrary,
    page: Option<usize>,
    group_id: &str,
) -> Option<Vec<NodeId>> {
    find_repeated_structures(doc, components, page, 1)
        .into_iter()
        .find(|group| group.group_id == group_id)
        .map(|group| group.node_ids)
}

/// Every frame on page `page` with the structure of `id` (itself
/// included, document order), whatever it is nested in; empty when `id` is
/// no convertible frame there, an empty frame, or has no copy.
pub fn similar_layers(
    doc: &PenDocument,
    components: &ComponentLibrary,
    page: Option<usize>,
    id: &NodeId,
) -> Vec<NodeId> {
    let entries = scan(doc, components, page);
    let Some(target) = entries
        .iter()
        .find(|entry| entry.eligible && entry.id == id.as_str())
    else {
        return Vec::new();
    };
    // An empty frame is a box, not a component worth making (sizes are
    // not structural, so every empty frame would match).
    if target.count < 2 {
        return Vec::new();
    }
    let similar: Vec<NodeId> = entries
        .iter()
        .filter(|entry| entry.eligible && entry.fingerprint == target.fingerprint)
        .map(|entry| NodeId::new(entry.id.as_str()))
        .collect();
    if similar.len() < 2 {
        return Vec::new();
    }
    similar
}

/// Whether every id names a convertible frame on page `page` and all share
/// one structure.
pub(super) fn share_structure(
    doc: &PenDocument,
    components: &ComponentLibrary,
    page: Option<usize>,
    ids: &[NodeId],
) -> bool {
    let entries = scan(doc, components, page);
    let mut fingerprints = ids.iter().map(|id| {
        entries
            .iter()
            .find(|entry| entry.eligible && entry.id == id.as_str())
            .map(|entry| entry.fingerprint)
    });
    let Some(Some(first)) = fingerprints.next() else {
        return false;
    };
    fingerprints.all(|fingerprint| fingerprint == Some(first))
}
