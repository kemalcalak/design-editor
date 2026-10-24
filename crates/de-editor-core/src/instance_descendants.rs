//! How an instance's `descendants` map applies to its component subtree
//! (#166, pen.dev's `Ref.descendants` contract).
//!
//! Keys are id paths of the descendant they change:
//!
//! - `childId` — a node anywhere in the component subtree (matched by its
//!   original id at any depth, the long-standing canonical rule);
//! - `nestedId/childId[/…]` — a node inside the nested instance
//!   `nestedId`. The entry is forwarded into that nested instance's own
//!   `descendants` (stripped of the prefix, outer entry wins), so it
//!   works at any nesting depth.
//!
//! Values:
//!
//! - no `type` — a property patch spread onto the node. A patch that
//!   carries `children` replaces the node's children: that is how an
//!   instance fills a slot frame with its own content;
//! - with `type` — a replacement: the whole descendant subtree is
//!   swapped for the entry, which is a complete node tree.
//!
//! Replacement subtrees and slot children are "instance-owned": they live
//! only in the entry, so edits to them are written into the entry in place
//! ([`patch_owned_node`]).

use serde_json::{Map, Value};

/// Separator between the segments of a descendant id path.
pub const PATH_SEPARATOR: char = '/';

/// True when a `descendants` entry replaces its node instead of patching
/// it (pen.dev: "`type` is present = replacement").
pub fn is_replacement(entry: &Value) -> bool {
    entry.get("type").is_some_and(|kind| !kind.is_null())
}

/// True when an entry brings instance-owned children: a replacement with
/// children or a patch that fills the node's children (slot content).
pub fn owns_children(entry: &Value) -> bool {
    entry.get("children").is_some_and(Value::is_array)
}

/// Join a key prefix (`""` or `"a/"`) and an id into a descendant key.
pub fn join_key(prefix: &str, id: &str) -> String {
    format!("{prefix}{id}")
}

/// The entries of `overrides` that address nodes inside the nested
/// instance `nested_id`, re-keyed relative to it (`nested/x` → `x`).
pub fn forwarded_entries(overrides: &Map<String, Value>, nested_id: &str) -> Map<String, Value> {
    let prefix = format!("{nested_id}{PATH_SEPARATOR}");
    overrides
        .iter()
        .filter_map(|(key, value)| {
            let rest = key.strip_prefix(&prefix)?;
            (!rest.is_empty()).then(|| (rest.to_string(), value.clone()))
        })
        .collect()
}

/// Apply `overrides` to one component child and its subtree, by original
/// id. `virtual_prefix` is the instance id when the result is rendered (ids
/// become `instance__id`), `None` when the original ids must stay (detach,
/// inspector display).
pub fn apply_descendants(
    child: &Value,
    overrides: Option<&Map<String, Value>>,
    virtual_prefix: Option<&str>,
) -> Value {
    let mut mapped = apply_entry(child, overrides);
    let Value::Object(map) = &mut mapped else {
        return mapped;
    };
    if let Some(prefix) = virtual_prefix {
        let node_id = map.get("id").and_then(Value::as_str).unwrap_or_default();
        let virtual_id = crate::ref_resolve::instance_child_virtual_id(prefix, node_id);
        map.insert("id".into(), Value::String(virtual_id));
    }
    if let Some(Value::Array(children)) = map.get_mut("children") {
        for child in children.iter_mut() {
            *child = apply_descendants(child, overrides, virtual_prefix);
        }
    }
    mapped
}

/// Apply the entry of this one node (a patch, a replacement, or the entries
/// forwarded into a nested instance), leaving its children as they are.
pub fn apply_entry(child: &Value, overrides: Option<&Map<String, Value>>) -> Value {
    let Value::Object(child_map) = child else {
        return child.clone();
    };
    let original_id = child_map
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let entry = overrides.and_then(|o| o.get(original_id));
    let mut mapped = match entry {
        Some(entry @ Value::Object(replacement)) if is_replacement(entry) => {
            let mut replacement = replacement.clone();
            // A replacement without its own id keeps the slot's id, so the
            // canvas anchor and the descendant key stay stable.
            if replacement.get("id").and_then(Value::as_str).is_none() {
                replacement.insert("id".into(), Value::String(original_id.to_string()));
            }
            replacement
        }
        Some(Value::Object(patch)) => {
            let mut mapped = child_map.clone();
            if is_ref(&mapped) {
                patch_nested_instance_root(&mut mapped, patch);
            } else {
                for (key, value) in patch {
                    mapped.insert(key.clone(), value.clone());
                }
            }
            mapped
        }
        _ => child_map.clone(),
    };
    if is_ref(&mapped) {
        if let Some(overrides) = overrides {
            let node_id = mapped
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            forward_into_nested_instance(&mut mapped, overrides, &node_id);
        }
    }
    Value::Object(mapped)
}

fn is_ref(map: &Map<String, Value>) -> bool {
    map.get("type").and_then(Value::as_str) == Some("ref")
}

/// A patch on a nested instance's own node targets its component root:
/// the canonical `RefNode` holds no paint or size fields, so the keys go to
/// `descendants[<its ref>]`, the instance's top-level override slot.
fn patch_nested_instance_root(ref_map: &mut Map<String, Value>, patch: &Map<String, Value>) {
    let Some(target) = ref_map
        .get("ref")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let top = descendants_mut(ref_map)
        .entry(target)
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(top) = top {
        for (key, value) in patch {
            if !matches!(key.as_str(), "id" | "type" | "children") {
                top.insert(key.clone(), value.clone());
            }
        }
    }
}

/// Copy the outer instance's `nested_id/…` entries into the nested
/// instance's own `descendants`. The outer instance wins on a clash: it is
/// the closer, more specific override.
fn forward_into_nested_instance(
    ref_map: &mut Map<String, Value>,
    overrides: &Map<String, Value>,
    nested_id: &str,
) {
    let forwarded = forwarded_entries(overrides, nested_id);
    if forwarded.is_empty() {
        return;
    }
    let descendants = descendants_mut(ref_map);
    for (key, value) in forwarded {
        // Two patches merge key by key; a replacement on either side wins
        // whole.
        let merge = !is_replacement(&value)
            && descendants
                .get(&key)
                .is_some_and(|existing| existing.is_object() && !is_replacement(existing));
        match (descendants.get_mut(&key), value) {
            (Some(Value::Object(existing)), Value::Object(patch)) if merge => {
                existing.extend(patch);
            }
            (_, value) => {
                descendants.insert(key, value);
            }
        }
    }
}

/// The `descendants` map of a `ref` object, created when absent.
pub fn descendants_mut(ref_map: &mut Map<String, Value>) -> &mut Map<String, Value> {
    let entry = ref_map
        .entry("descendants")
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    match entry {
        Value::Object(map) => map,
        _ => unreachable!("descendants was just made an object"),
    }
}

/// The id a replacement entry's root renders under: its own id, else the
/// id of the node it replaced (the last segment of its key).
pub fn replacement_root_id<'a>(entry: &'a Value, entry_key: &'a str) -> &'a str {
    entry.get("id").and_then(Value::as_str).unwrap_or_else(|| {
        entry_key
            .rsplit_once(PATH_SEPARATOR)
            .map_or(entry_key, |(_, last)| last)
    })
}

/// Find the node `node_id` inside the instance-owned tree of the entry at
/// `entry_key`: the entry itself when it is a replacement, else its
/// `children`, at any depth.
pub fn find_owned_node_mut<'a>(
    entry: &'a mut Value,
    entry_key: &str,
    node_id: &str,
) -> Option<&'a mut Value> {
    fn walk<'a>(node: &'a mut Value, node_id: &str) -> Option<&'a mut Value> {
        if node.get("id").and_then(Value::as_str) == Some(node_id) {
            return Some(node);
        }
        node.get_mut("children")?
            .as_array_mut()?
            .iter_mut()
            .find_map(|child| walk(child, node_id))
    }
    if is_replacement(entry) {
        if replacement_root_id(entry, entry_key) == node_id {
            return Some(entry);
        }
        return entry
            .get_mut("children")?
            .as_array_mut()?
            .iter_mut()
            .find_map(|child| walk(child, node_id));
    }
    entry
        .get_mut("children")?
        .as_array_mut()?
        .iter_mut()
        .find_map(|child| walk(child, node_id))
}

/// Spread `patch` onto the owned node `node_id` inside `entry`. A `null`
/// value removes the key (the inspector's "cleared" signal). Returns false
/// when the node is not part of the entry.
pub fn patch_owned_node(
    entry: &mut Value,
    entry_key: &str,
    node_id: &str,
    patch: &Map<String, Value>,
) -> bool {
    let Some(Value::Object(node)) = find_owned_node_mut(entry, entry_key, node_id) else {
        return false;
    };
    if is_ref(node) {
        patch_nested_instance_root(node, patch);
        return true;
    }
    for (key, value) in patch {
        if matches!(key.as_str(), "id" | "type") {
            continue;
        }
        if value.is_null() {
            node.remove(key);
        } else {
            node.insert(key.clone(), value.clone());
        }
    }
    true
}

/// Remove the override at `key` and every path entry below it
/// (`key/…`), restoring the master's node. Returns true when anything was
/// removed.
pub fn reset_entry(descendants: &mut Map<String, Value>, key: &str) -> bool {
    let prefix = format!("{key}{PATH_SEPARATOR}");
    let before = descendants.len();
    descendants.retain(|existing, _| existing != key && !existing.starts_with(&prefix));
    descendants.len() != before
}

#[cfg(test)]
#[path = "instance_descendants_tests.rs"]
mod tests;
