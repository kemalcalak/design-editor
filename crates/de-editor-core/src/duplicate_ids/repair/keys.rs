//! Moving instance `descendants` keys along with renamed nodes.
//!
//! A key is an id path: its first segment names a node the instance can
//! address (its master's subtree or its own content), each later segment a
//! node inside the nested instance the segment before it names. A segment
//! that names a renamed id is rewritten to the new id when that new id is in
//! the segment's scope. When the old id is still in scope too, the entry
//! applied to both nodes before the repair, so it is kept and copied.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::{subtree_ids, walk_document, Origin, Renames};
use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::instance_descendants::{is_replacement, PATH_SEPARATOR};
use crate::instance_ids::{for_each_owned_node, json_node_id, remint_owned_ids};
use crate::node_id::NodeId;
use serde_json::{Map, Value};

/// A `ref` node: what it instantiates and the ids of its own content.
struct RefNode {
    target: String,
    owned: HashSet<String>,
}

/// What resolving key segments needs to know about the repaired document.
struct Scopes<'a> {
    renames: &'a Renames,
    refs: HashMap<String, RefNode>,
    /// Every id in the subtree of each `ref` target.
    masters: HashMap<String, HashSet<String>>,
}

impl Scopes<'_> {
    /// The ids a key segment can name inside the instance `ref_id`.
    fn of_ref(&self, ref_id: &str) -> HashSet<String> {
        let Some(reference) = self.refs.get(ref_id) else {
            return HashSet::new();
        };
        let mut scope = self
            .masters
            .get(&reference.target)
            .cloned()
            .unwrap_or_default();
        scope.extend(reference.owned.iter().cloned());
        scope
    }

    /// Every path `segments` can mean after the renames (see the module docs).
    fn variants(&self, segments: &[&str], scope: &HashSet<String>) -> Vec<Vec<String>> {
        let Some((first, rest)) = segments.split_first() else {
            return vec![Vec::new()];
        };
        let mut choices = Vec::new();
        match self.renames.get(*first) {
            None => choices.push(first.to_string()),
            Some(moved) => {
                if scope.contains(*first) {
                    choices.push(first.to_string());
                }
                choices.extend(
                    moved
                        .iter()
                        .filter(|rename| scope.contains(&rename.new_id))
                        .map(|rename| rename.new_id.clone()),
                );
                if choices.is_empty() {
                    // Pointed at nothing before the repair either.
                    choices.push(first.to_string());
                }
            }
        }
        let mut out = Vec::new();
        for choice in choices {
            let inner = if rest.is_empty() {
                HashSet::new()
            } else {
                self.of_ref(&choice)
            };
            for mut tail in self.variants(rest, &inner) {
                tail.insert(0, choice.clone());
                out.push(tail);
            }
        }
        out
    }
}

/// Move the keys of every instance in the document (see the module docs).
pub(super) fn rekey_instances(
    doc: &mut Value,
    renames: &Renames,
    allocator: &mut dyn IdAllocator,
    taken: &mut HashSet<NodeId>,
) -> Result<(), IdAllocError> {
    if renames.is_empty() {
        return Ok(());
    }
    let mut refs = HashMap::new();
    walk_document(doc, &mut |_, node, _| {
        let (Some(id), Some(target)) =
            (json_node_id(node), node.get("ref").and_then(Value::as_str))
        else {
            return;
        };
        let mut owned = HashSet::new();
        if let Some(Value::Object(descendants)) = node.get("descendants") {
            for_each_owned_node(descendants, &mut |owned_node, keeps_slot_id| {
                if let (false, Some(id)) = (keeps_slot_id, json_node_id(owned_node)) {
                    owned.insert(id.to_string());
                }
            });
        }
        refs.entry(id.to_string()).or_insert(RefNode {
            target: target.to_string(),
            owned,
        });
    });
    let targets: HashSet<String> = refs.values().map(|r| r.target.clone()).collect();
    let mut masters: HashMap<String, HashSet<String>> = HashMap::new();
    walk_document(doc, &mut |_, node, origin| {
        let Some(id) = json_node_id(node) else {
            return;
        };
        if origin == Origin::Tree && targets.contains(id) && !masters.contains_key(id) {
            let mut ids = HashSet::new();
            subtree_ids(node, &mut ids);
            masters.insert(id.to_string(), ids);
        }
    });
    let scopes = Scopes {
        renames,
        refs,
        masters,
    };

    let mut result = Ok(());
    walk_document(doc, &mut |_, node, _| {
        if result.is_err() {
            return;
        }
        let Some(id) = json_node_id(node).map(str::to_owned) else {
            return;
        };
        let Some(Value::Object(descendants)) = node.get_mut("descendants") else {
            return;
        };
        let scope = scopes.of_ref(&id);
        result = rekey_map(descendants, &scope, &scopes, allocator, taken);
    });
    result
}

fn rekey_map(
    descendants: &mut Map<String, Value>,
    scope: &HashSet<String>,
    scopes: &Scopes<'_>,
    allocator: &mut dyn IdAllocator,
    taken: &mut HashSet<NodeId>,
) -> Result<(), IdAllocError> {
    let hit: Vec<String> = descendants
        .keys()
        .filter(|key| {
            key.split(PATH_SEPARATOR)
                .any(|segment| scopes.renames.contains_key(segment))
        })
        .cloned()
        .collect();
    for key in hit {
        let segments: Vec<&str> = key.split(PATH_SEPARATOR).collect();
        let separator = PATH_SEPARATOR.to_string();
        let variants: Vec<String> = scopes
            .variants(&segments, scope)
            .into_iter()
            .map(|path| path.join(&separator))
            .collect();
        let keep = variants.contains(&key);
        let added: Vec<String> = variants
            .into_iter()
            .filter(|variant| *variant != key && !descendants.contains_key(variant))
            .collect();
        if added.is_empty() {
            continue;
        }
        let entry = if keep {
            descendants[&key].clone()
        } else {
            descendants.remove(&key).unwrap_or_default()
        };
        for (index, new_key) in added.iter().enumerate() {
            let mut value = entry.clone();
            follow_slot_id(&mut value, &key, new_key);
            // The first new key of a moved entry takes it over; every other
            // one is a copy and needs ids of its own.
            if keep || index > 0 {
                value = remint_entry(new_key, value, allocator, taken)?;
            }
            descendants.insert(new_key.clone(), value);
        }
    }
    Ok(())
}

fn last_segment(key: &str) -> &str {
    key.rsplit_once(PATH_SEPARATOR)
        .map_or(key, |(_, last)| last)
}

/// A replacement that kept the replaced node's id keeps the new one.
fn follow_slot_id(entry: &mut Value, old_key: &str, new_key: &str) {
    let (old_slot, new_slot) = (last_segment(old_key), last_segment(new_key));
    if old_slot == new_slot || !is_replacement(entry) {
        return;
    }
    if entry.get("id").and_then(Value::as_str) == Some(old_slot) {
        entry["id"] = Value::String(new_slot.to_string());
    }
}

/// Fresh ids for the instance content inside one copied entry.
fn remint_entry(
    key: &str,
    entry: Value,
    allocator: &mut dyn IdAllocator,
    taken: &mut HashSet<NodeId>,
) -> Result<Value, IdAllocError> {
    let mut single = BTreeMap::from([(key.to_string(), entry)]);
    remint_owned_ids(&mut single, allocator, taken)?;
    Ok(single.into_values().next().unwrap_or_default())
}
