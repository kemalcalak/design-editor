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
//! [`InstanceBases`] before conversion.
//!
//! Shader uniforms cannot stay bound to a `$variable` (the import resolves
//! them once, under the component's theme). An instance with its own
//! `theme` (a dark copy of a light screen) therefore gets the component's
//! variable-driven shader paints as patches of its own, which the walk
//! resolves under the instance's theme.

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

/// Paint keys that can hold a shader paint.
const PAINT_KEYS: &[&str] = &["fill", "stroke", "effect"];

/// What the file says about nodes before any conversion: each node's own
/// stroke keys, and each component's shader paints that read variables.
#[derive(Default)]
pub(super) struct InstanceBases {
    strokes: HashMap<String, Map<String, Value>>,
    /// Component id → `(node id, paint key, paint as written)`.
    themed_paints: HashMap<String, Vec<(String, &'static str, Value)>>,
}

impl InstanceBases {
    /// Index the document's nodes. Iterative like the walk.
    pub(super) fn from_children(children: &[Value]) -> Self {
        let mut bases = Self::default();
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
                    bases.strokes.entry(id.to_owned()).or_insert(keys);
                }
                if map.get("reusable").and_then(Value::as_bool) == Some(true) {
                    let paints = themed_paints(node);
                    if !paints.is_empty() {
                        bases.themed_paints.insert(id.to_owned(), paints);
                    }
                }
            }
            if let Some(Value::Array(kids)) = map.get("children") {
                stack.extend(kids);
            }
        }
        bases
    }

    /// The stroke keys of the node a patch keyed `path` applies to: the last
    /// segment of a pen.dev id path names that node.
    pub(super) fn stroke(&self, path: &str) -> Option<&Map<String, Value>> {
        let id = path.rsplit('/').next().unwrap_or(path);
        self.strokes.get(id)
    }

    /// Give a themed instance its component's variable-driven shader paints
    /// as patches, so they resolve under the instance's theme. A paint the
    /// instance already overrides is left alone.
    pub(super) fn add_themed_paints(&self, map: &mut Map<String, Value>) {
        let themed = map
            .get("theme")
            .and_then(Value::as_object)
            .is_some_and(|theme| !theme.is_empty());
        if !themed || map.get("type").and_then(Value::as_str) != Some("ref") {
            return;
        }
        let Some(paints) = map
            .get("ref")
            .and_then(Value::as_str)
            .and_then(|target| self.themed_paints.get(target))
        else {
            return;
        };
        let descendants = map
            .entry("descendants")
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(descendants) = descendants else {
            return;
        };
        for (id, key, paint) in paints {
            let patch = descendants
                .entry(id.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(patch) = patch {
                if !patch.contains_key("type") {
                    patch.entry(*key).or_insert_with(|| paint.clone());
                }
            }
        }
    }
}

/// The shader paints in a component's own subtree (not inside nested
/// instances) whose uniforms name a variable.
fn themed_paints(component: &Value) -> Vec<(String, &'static str, Value)> {
    let mut out = Vec::new();
    let mut stack = vec![component];
    while let Some(node) = stack.pop() {
        let Value::Object(map) = node else {
            continue;
        };
        if let Some(id) = map.get("id").and_then(Value::as_str) {
            for key in PAINT_KEYS {
                if let Some(paint) = map.get(*key).filter(|paint| reads_variable_uniform(paint)) {
                    out.push((id.to_owned(), *key, paint.clone()));
                }
            }
        }
        if map.get("type").and_then(Value::as_str) == Some("ref") {
            continue;
        }
        if let Some(Value::Array(kids)) = map.get("children") {
            stack.extend(kids);
        }
    }
    out
}

/// Whether a paint (one or a list) holds a shader whose uniforms name a
/// `$variable`.
fn reads_variable_uniform(paint: &Value) -> bool {
    let shader_reads = |paint: &Value| {
        paint.get("type").and_then(Value::as_str) == Some("shader")
            && paint
                .get("uniforms")
                .and_then(Value::as_object)
                .is_some_and(|uniforms| {
                    uniforms
                        .values()
                        .any(|value| value.as_str().is_some_and(|text| text.starts_with('$')))
                })
    };
    match paint {
        Value::Array(items) => items.iter().any(shader_reads),
        single => shader_reads(single),
    }
}

#[cfg(test)]
#[path = "instances_tests.rs"]
mod tests;
