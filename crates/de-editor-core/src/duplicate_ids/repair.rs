//! The duplicate-id repair (#364).
//!
//! Each duplicated id stays with the use that `ref`s resolve to: the first
//! component master (a reusable frame, the component library's rule), else
//! the first node in the page trees (the tree lookup's rule), else the first
//! node of instance content; document order breaks ties. Every other use
//! gets a fresh id. So `ref` targets never need to move, component and
//! library masters keep their ids, and instance content, which only its own
//! instance addresses, is what usually moves. The references that meant a
//! renamed node follow it, so the canvas does not change:
//!
//! - `descendants` keys of every instance, segment by segment through nested
//!   instances ([`keys`]): an entry moves to the renamed id, and is copied
//!   (with fresh ids for its own content) when the old id is still there too;
//! - `scroll_to` targets on a page where the renamed node is now the only
//!   node in the page tree that the target can have meant.
//!
//! The document is rewritten as JSON: instance content is untyped JSON
//! anyway, and this way one walk covers page trees and instance content
//! alike, in the order [`super::find_duplicate_ids`] counts.

use std::collections::{HashMap, HashSet};

use super::{
    find_duplicate_ids, owned_node_label, page_label, DuplicateIdRepairError, RenamedId,
    ROOT_LIST_LABEL,
};
use crate::id_allocator::{collect_document_ids, IdAllocator};
use crate::instance_ids::{for_each_owned_node, for_each_owned_node_mut, json_node_id};
use crate::node_id::NodeId;
use jian_ops_schema::PenDocument;
use serde_json::{Map, Value};

mod keys;

/// A repaired copy of a document and what was renamed in it.
#[derive(Debug, Clone)]
pub struct RepairedDocument {
    pub doc: PenDocument,
    pub renamed: Vec<RenamedId>,
}

/// Repair `doc` in place. Returns what was renamed (empty when every id was
/// already unique).
pub fn repair_duplicate_ids(
    doc: &mut PenDocument,
    allocator: &mut dyn IdAllocator,
) -> Result<Vec<RenamedId>, DuplicateIdRepairError> {
    match repaired_document(doc, allocator)? {
        Some(repaired) => {
            *doc = repaired.doc;
            Ok(repaired.renamed)
        }
        None => Ok(Vec::new()),
    }
}

/// The repaired form of `doc`, or `None` when it has no duplicate ids. `doc`
/// itself is untouched, so this also serves a dry run.
pub fn repaired_document(
    doc: &PenDocument,
    allocator: &mut dyn IdAllocator,
) -> Result<Option<RepairedDocument>, DuplicateIdRepairError> {
    if find_duplicate_ids(doc).is_empty() {
        return Ok(None);
    }
    let mut value = serde_json::to_value(doc)?;
    let mut taken = collect_document_ids(doc);
    let renamed = repair_value(&mut value, allocator, &mut taken)?;
    let doc: PenDocument = serde_json::from_value(value)?;
    debug_assert!(
        find_duplicate_ids(&doc).is_empty(),
        "the repair leaves no duplicate id"
    );
    Ok(Some(RepairedDocument { doc, renamed }))
}

/// Where a node sits: in a page tree, or in an instance's own content.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Tree,
    Owned { keeps_slot_id: bool },
}

/// The id a node counts under: `None` for a node without one, and for a
/// replacement root that keeps the replaced node's id (see the module docs
/// of [`super`]). Both passes of [`repair_value`] pair uses through it.
fn counted_id(node: &Map<String, Value>, origin: Origin) -> Option<&str> {
    match origin {
        Origin::Owned {
            keeps_slot_id: true,
        } => None,
        _ => json_node_id(node),
    }
}

/// Which use of an id keeps it: lowest first (see the module docs).
fn keep_rank(node: &Map<String, Value>, origin: Origin) -> u8 {
    let reusable_frame = node.get("type").and_then(Value::as_str) == Some("frame")
        && node.get("reusable").and_then(Value::as_bool) == Some(true);
    match origin {
        Origin::Tree if reusable_frame => 0,
        Origin::Tree => 1,
        Origin::Owned { .. } => 2,
    }
}

type Visit<'v> = dyn FnMut(usize, &mut Map<String, Value>, Origin) + 'v;

/// Visit every node of the JSON document with its page index (the root list
/// is the index after the last page), in counting order.
fn walk_document(doc: &mut Value, visit: &mut Visit<'_>) {
    let page_count = doc
        .get("pages")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if let Some(Value::Array(pages)) = doc.get_mut("pages") {
        for (index, page) in pages.iter_mut().enumerate() {
            if let Some(Value::Array(children)) = page.get_mut("children") {
                walk_nodes(index, children, visit);
            }
        }
    }
    if let Some(Value::Array(children)) = doc.get_mut("children") {
        walk_nodes(page_count, children, visit);
    }
}

fn walk_nodes(page: usize, nodes: &mut [Value], visit: &mut Visit<'_>) {
    for node in nodes {
        let Value::Object(map) = node else {
            continue;
        };
        visit(page, map, Origin::Tree);
        if let Some(Value::Object(descendants)) = map.get_mut("descendants") {
            for_each_owned_node_mut(descendants.iter_mut(), &mut |owned, keeps_slot_id| {
                visit(page, owned, Origin::Owned { keeps_slot_id });
            });
        }
        if let Some(Value::Array(children)) = map.get_mut("children") {
            walk_nodes(page, children, visit);
        }
    }
}

fn page_labels(doc: &Value) -> Vec<String> {
    let mut labels: Vec<String> = doc
        .get("pages")
        .and_then(Value::as_array)
        .map(|pages| {
            pages
                .iter()
                .map(|page| {
                    let field = |key| page.get(key).and_then(Value::as_str).unwrap_or_default();
                    page_label(field("name"), field("id"))
                })
                .collect()
        })
        .unwrap_or_default();
    labels.push(ROOT_LIST_LABEL.to_string());
    labels
}

/// One use of an id, in counting order.
struct Use {
    id: String,
    rank: u8,
}

/// A renamed node, with what the reference passes need to know about it.
struct Rename {
    new_id: String,
    page: usize,
    in_tree: bool,
}

/// Old id → the nodes that used it and were renamed.
type Renames = HashMap<String, Vec<Rename>>;

fn repair_value(
    doc: &mut Value,
    allocator: &mut dyn IdAllocator,
    taken: &mut HashSet<NodeId>,
) -> Result<Vec<RenamedId>, DuplicateIdRepairError> {
    let mut uses = Vec::new();
    walk_document(doc, &mut |_, node, origin| {
        if let Some(id) = counted_id(node, origin) {
            uses.push(Use {
                id: id.to_string(),
                rank: keep_rank(node, origin),
            });
        }
    });

    let mut keeper: HashMap<&str, usize> = HashMap::new();
    for (ordinal, use_) in uses.iter().enumerate() {
        let best = keeper.entry(use_.id.as_str()).or_insert(ordinal);
        if use_.rank < uses[*best].rank {
            *best = ordinal;
        }
    }
    let mut new_ids: HashMap<usize, String> = HashMap::new();
    for (ordinal, use_) in uses.iter().enumerate() {
        if keeper[use_.id.as_str()] != ordinal {
            new_ids.insert(ordinal, allocator.allocate(taken)?.as_str().to_string());
        }
    }

    let labels = page_labels(doc);
    let mut renamed = Vec::new();
    let mut renames = Renames::new();
    let mut ordinal = 0;
    walk_document(doc, &mut |page, node, origin| {
        let Some(old_id) = counted_id(node, origin).map(str::to_owned) else {
            return;
        };
        let this = ordinal;
        ordinal += 1;
        let Some(new_id) = new_ids.get(&this) else {
            return;
        };
        node.insert("id".into(), Value::String(new_id.clone()));
        renamed.push(RenamedId {
            old_id: old_id.clone(),
            new_id: new_id.clone(),
            page: labels[page].clone(),
            name: owned_node_label(node),
        });
        renames.entry(old_id).or_default().push(Rename {
            new_id: new_id.clone(),
            page,
            in_tree: origin == Origin::Tree,
        });
    });

    keys::rekey_instances(doc, &renames, allocator, taken)?;
    retarget_scrolls(doc, &renames);
    Ok(renamed)
}

/// Ids an instance's keys can address: its master's subtree and its own
/// content.
fn subtree_ids(node: &Map<String, Value>, out: &mut HashSet<String>) {
    if let Some(id) = json_node_id(node) {
        out.insert(id.to_string());
    }
    if let Some(Value::Object(descendants)) = node.get("descendants") {
        for_each_owned_node(descendants, &mut |owned, keeps_slot_id| {
            if let (false, Some(id)) = (keeps_slot_id, json_node_id(owned)) {
                out.insert(id.to_string());
            }
        });
    }
    if let Some(Value::Array(children)) = node.get("children") {
        for child in children {
            if let Value::Object(child) = child {
                subtree_ids(child, out);
            }
        }
    }
}

/// Call `retarget` with every `scroll_to` target inside an `events` value.
fn for_each_scroll_target(value: &mut Value, retarget: &mut dyn FnMut(&mut Value)) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                if key == "scroll_to" {
                    if let Some(target) = inner.get_mut("target") {
                        retarget(target);
                        continue;
                    }
                }
                for_each_scroll_target(inner, retarget);
            }
        }
        Value::Array(items) => {
            for item in items {
                for_each_scroll_target(item, retarget);
            }
        }
        _ => {}
    }
}

/// `scroll_to` targets: a section link on a page whose tree no longer holds
/// the old id, and holds exactly one renamed copy of it, follows that copy.
fn retarget_scrolls(doc: &mut Value, renames: &HashMap<String, Vec<Rename>>) {
    let mut tree_ids: HashSet<(usize, String)> = HashSet::new();
    walk_document(doc, &mut |page, node, origin| {
        if let (Origin::Tree, Some(id)) = (origin, json_node_id(node)) {
            tree_ids.insert((page, id.to_string()));
        }
    });
    walk_document(doc, &mut |page, node, _| {
        let Some(events) = node.get_mut("events") else {
            return;
        };
        for_each_scroll_target(events, &mut |target| {
            let Some(old) = target.as_str() else {
                return;
            };
            if tree_ids.contains(&(page, old.to_string())) {
                return;
            }
            let mut on_page = renames
                .get(old)
                .into_iter()
                .flatten()
                .filter(|rename| rename.in_tree && rename.page == page);
            if let (Some(only), None) = (on_page.next(), on_page.next()) {
                *target = Value::String(only.new_id.clone());
            }
        });
    });
}
