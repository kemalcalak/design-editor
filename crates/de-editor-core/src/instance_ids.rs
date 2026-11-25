//! Node ids stored inside component instances (#364).
//!
//! An instance keeps its own content in its `descendants` map
//! ([`crate::instance_descendants`]): replacement subtrees (entries with
//! `type`) and slot children (the `children` of a patch). Those nodes carry
//! ids of their own but sit in no `children` array of the typed tree, so
//! every walk that has to see all ids (the id allocator, its collision set,
//! the duplicate-id check) visits them through this module.
//!
//! Order: entries in key order; each node before the content of the nested
//! instance it may be, and that before its own children. The immutable and
//! the mutable walk below keep the same order, which the duplicate-id repair
//! relies on.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::instance_descendants::{is_replacement, replacement_root_id, PATH_SEPARATOR};
use crate::node_id::NodeId;
use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

/// Whether a replacement root renders under the id of the node it replaced
/// (no id of its own, or the same id). Its id is then the master node's and
/// names no node of its own.
fn keeps_slot_id(key: &str, entry: &Value) -> bool {
    let slot = key
        .rsplit_once(PATH_SEPARATOR)
        .map_or(key, |(_, last)| last);
    replacement_root_id(entry, key) == slot
}

/// Visit every node stored in an instance's `descendants`. The flag is true
/// for a replacement root that keeps the replaced node's id
/// ([`keeps_slot_id`]); callers that look for nodes of their own skip it.
pub fn for_each_owned_node<'a, I>(
    descendants: I,
    visit: &mut dyn FnMut(&'a Map<String, Value>, bool),
) where
    I: IntoIterator<Item = (&'a String, &'a Value)>,
{
    for (key, entry) in descendants {
        let Value::Object(map) = entry else {
            continue;
        };
        if is_replacement(entry) {
            visit_node(map, keeps_slot_id(key, entry), visit);
        } else {
            visit_children(map, visit);
        }
    }
}

fn visit_node<'a>(
    node: &'a Map<String, Value>,
    keeps_slot_id: bool,
    visit: &mut dyn FnMut(&'a Map<String, Value>, bool),
) {
    visit(node, keeps_slot_id);
    if let Some(Value::Object(nested)) = node.get("descendants") {
        for_each_owned_node(nested, visit);
    }
    visit_children(node, visit);
}

fn visit_children<'a>(
    node: &'a Map<String, Value>,
    visit: &mut dyn FnMut(&'a Map<String, Value>, bool),
) {
    if let Some(Value::Array(children)) = node.get("children") {
        for child in children {
            if let Value::Object(child) = child {
                visit_node(child, false, visit);
            }
        }
    }
}

/// Mutable form of [`for_each_owned_node`], same order.
pub fn for_each_owned_node_mut<'a, I>(
    descendants: I,
    visit: &mut dyn FnMut(&mut Map<String, Value>, bool),
) where
    I: IntoIterator<Item = (&'a String, &'a mut Value)>,
{
    for (key, entry) in descendants {
        let replacement = is_replacement(entry);
        let keeps = replacement && keeps_slot_id(key, entry);
        let Value::Object(map) = entry else {
            continue;
        };
        if replacement {
            visit_node_mut(map, keeps, visit);
        } else {
            visit_children_mut(map, visit);
        }
    }
}

fn visit_node_mut(
    node: &mut Map<String, Value>,
    keeps_slot_id: bool,
    visit: &mut dyn FnMut(&mut Map<String, Value>, bool),
) {
    visit(node, keeps_slot_id);
    if let Some(Value::Object(nested)) = node.get_mut("descendants") {
        for_each_owned_node_mut(nested.iter_mut(), visit);
    }
    visit_children_mut(node, visit);
}

fn visit_children_mut(
    node: &mut Map<String, Value>,
    visit: &mut dyn FnMut(&mut Map<String, Value>, bool),
) {
    if let Some(Value::Array(children)) = node.get_mut("children") {
        for child in children {
            if let Value::Object(child) = child {
                visit_node_mut(child, false, visit);
            }
        }
    }
}

/// The id of a node stored as JSON (instance content, or any node of a
/// document rewritten as JSON), when it has one.
pub fn json_node_id(node: &Map<String, Value>) -> Option<&str> {
    node.get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
}

/// Visit every id in `node`'s own instance content: nothing unless `node` is
/// a `ref` with `descendants`. Its `children` are the caller's to walk. A
/// replacement root that keeps the replaced node's id is visited too: that
/// id is taken either way, which is all the id allocator asks.
pub fn for_each_owned_id(node: &PenNode, visit: &mut dyn FnMut(&str)) {
    let PenNode::Ref(reference) = node else {
        return;
    };
    let Some(descendants) = reference.descendants.as_ref() else {
        return;
    };
    for_each_owned_node(descendants, &mut |owned, _| {
        if let Some(id) = json_node_id(owned) {
            visit(id);
        }
    });
}

/// Give every node of an instance's own content a fresh id and move the
/// `descendants` keys that named the old ids along with them. Used when an
/// instance is copied, so the copy's content does not share ids with the
/// original's. Replacement roots that keep the replaced node's id stay as
/// they are: that id is the master's.
pub fn remint_owned_ids(
    descendants: &mut BTreeMap<String, Value>,
    allocator: &mut dyn IdAllocator,
    taken: &mut HashSet<NodeId>,
) -> Result<(), IdAllocError> {
    let mut renamed: HashMap<String, String> = HashMap::new();
    let mut failure = None;
    for_each_owned_node_mut(descendants.iter_mut(), &mut |node, keeps| {
        if keeps || failure.is_some() {
            return;
        }
        let Some(old) = json_node_id(node).map(str::to_owned) else {
            return;
        };
        match allocator.allocate(taken) {
            Ok(fresh) => {
                node.insert("id".into(), Value::String(fresh.as_str().to_owned()));
                renamed.insert(old, fresh.as_str().to_owned());
            }
            Err(error) => failure = Some(error),
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    if renamed.is_empty() {
        return Ok(());
    }
    // Keys naming re-minted nodes, at this level and in the instances nested
    // inside the content.
    for (old_key, new_key) in moved_keys(descendants.keys(), &renamed) {
        if let Some(value) = descendants.remove(&old_key) {
            descendants.insert(new_key, value);
        }
    }
    for_each_owned_node_mut(descendants.iter_mut(), &mut |node, _| {
        if let Some(Value::Object(nested)) = node.get_mut("descendants") {
            for (old_key, new_key) in moved_keys(nested.keys(), &renamed) {
                if let Some(value) = nested.remove(&old_key) {
                    nested.insert(new_key, value);
                }
            }
        }
    });
    Ok(())
}

/// The keys whose first segment is a renamed id, each with its new spelling
/// (`old/rest` → `new/rest`).
fn moved_keys<'k>(
    keys: impl Iterator<Item = &'k String>,
    renamed: &HashMap<String, String>,
) -> Vec<(String, String)> {
    keys.filter_map(|key| {
        let (first, rest) = split_first_segment(key);
        let fresh = renamed.get(first)?;
        Some((key.clone(), format!("{fresh}{rest}")))
    })
    .collect()
}

/// The first segment of a descendant key (`a/b/c` → `a`).
pub fn first_segment(key: &str) -> &str {
    split_first_segment(key).0
}

/// Split a descendant key into its first segment and the rest, the rest
/// keeping its leading separator (`a/b/c` → `("a", "/b/c")`).
pub fn split_first_segment(key: &str) -> (&str, &str) {
    match key.find(PATH_SEPARATOR) {
        Some(at) => key.split_at(at),
        None => (key, ""),
    }
}

#[cfg(test)]
#[path = "instance_ids_tests.rs"]
mod tests;
