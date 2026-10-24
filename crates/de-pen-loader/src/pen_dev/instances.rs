//! Component instances (`ref`) for the pen.dev conversion.
//!
//! pen.dev writes an instance's own overrides (size, stroke, padding,
//! layout, …) as plain keys on the `ref` node. The canonical `RefNode` holds
//! only identity, position and a few common props; the resolver
//! (`de_editor_core::ref_resolve`) reads the rest from
//! `descendants[<component id>]`, the instance's top-level override. The
//! keys move there before the node is converted, so the walk converts them
//! like any other instance patch.
//!
//! An instance patch merges key by key, while the canonical `stroke` is one
//! object (paint, thickness, caps, alignment). A patch that names only part
//! of a stroke takes the rest from the node it patches, recorded per id in
//! [`StrokeBases`] before conversion.

use std::collections::HashMap;

use serde_json::{Map, Value};

/// Keys the canonical `RefNode` keeps on the instance itself (its own
/// fields, the common node base, and the pen.dev keys the node walk maps
/// onto those or removes). Everything else is a root override.
const INSTANCE_KEYS: &[&str] = &[
    "type",
    "id",
    "ref",
    "descendants",
    "children",
    "name",
    "role",
    "explain",
    "x",
    "y",
    "rotation",
    "constraints",
    "opacity",
    "enabled",
    "visible",
    "locked",
    "pin",
    "flipX",
    "flipY",
    "maskType",
    "blendMode",
    "theme",
    "transition",
    "animations",
    "state",
    "bindings",
    "events",
    "lifecycle",
    "semantics",
    "gestures",
    "route",
    // pen.dev spellings the walk converts or drops on the instance.
    "layoutPosition",
    "reusable",
    "placeholder",
    "context",
    "metadata",
];

/// Move a `ref` node's root overrides into `descendants[<ref target>]`.
/// Keys the file already wrote there win over the moved ones.
pub(super) fn move_root_overrides(map: &mut Map<String, Value>) {
    if map.get("type").and_then(Value::as_str) != Some("ref") {
        return;
    }
    let Some(target) = map.get("ref").and_then(Value::as_str).map(str::to_owned) else {
        return;
    };
    let keys: Vec<String> = map
        .keys()
        .filter(|key| !INSTANCE_KEYS.contains(&key.as_str()))
        .cloned()
        .collect();
    if keys.is_empty() {
        return;
    }
    let mut patch = Map::new();
    for key in keys {
        if let Some(value) = map.remove(&key) {
            patch.insert(key, value);
        }
    }
    let descendants = map
        .entry("descendants")
        .or_insert_with(|| Value::Object(Map::new()));
    if !descendants.is_object() {
        *descendants = Value::Object(Map::new());
    }
    let Value::Object(descendants) = descendants else {
        return;
    };
    match descendants.get_mut(&target) {
        Some(Value::Object(existing)) => {
            for (key, value) in patch {
                existing.entry(key).or_insert(value);
            }
        }
        _ => {
            descendants.insert(target, Value::Object(patch));
        }
    }
}

/// pen.dev stroke keys, folded into one canonical `stroke` object.
pub(super) const STROKE_KEYS: &[&str] = &[
    "stroke",
    "strokeWidth",
    "strokeLinecap",
    "strokeLinejoin",
    "strokeAlignment",
];

/// Node id → the node's own pen.dev stroke keys, as written in the file.
#[derive(Default)]
pub(super) struct StrokeBases {
    by_id: HashMap<String, Map<String, Value>>,
}

impl StrokeBases {
    /// Index every node that sets a stroke key. Iterative like the walk.
    pub(super) fn from_children(children: &[Value]) -> Self {
        let mut by_id = HashMap::new();
        let mut stack: Vec<&Value> = children.iter().collect();
        while let Some(node) = stack.pop() {
            let Value::Object(map) = node else {
                continue;
            };
            if let Some(id) = map.get("id").and_then(Value::as_str) {
                let keys: Map<String, Value> = STROKE_KEYS
                    .iter()
                    .filter_map(|key| Some(((*key).to_owned(), map.get(*key)?.clone())))
                    .collect();
                if !keys.is_empty() {
                    by_id.entry(id.to_owned()).or_insert(keys);
                }
            }
            if let Some(Value::Array(kids)) = map.get("children") {
                stack.extend(kids);
            }
        }
        Self { by_id }
    }

    /// The stroke keys of the node a patch keyed `path` applies to: the last
    /// segment of a pen.dev id path names that node.
    pub(super) fn get(&self, path: &str) -> Option<&Map<String, Value>> {
        let id = path.rsplit('/').next().unwrap_or(path);
        self.by_id.get(id)
    }
}

#[cfg(test)]
#[path = "instances_tests.rs"]
mod tests;
