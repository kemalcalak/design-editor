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
//! Most `$variable` references are resolved once, under the theme the
//! component sits in (font sizes, radii, shader uniforms, …; see
//! `variables.rs`). An instance under another theme (a dark copy of a light
//! screen, a desktop card inside a mobile screen) therefore gets every such
//! field whose variables resolve differently there as a patch of its own,
//! which the walk resolves under the instance's theme.

use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Map, Value};

use super::paint::STROKE_KEYS;
use super::variables::{self, Theme, Variables};

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
    let descendants = descendants_mut(map);
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

/// The instance's `descendants` map, created (or replacing a malformed
/// value) when missing.
fn descendants_mut(map: &mut Map<String, Value>) -> &mut Map<String, Value> {
    let descendants = map
        .entry("descendants")
        .or_insert_with(|| Value::Object(Map::new()));
    if !descendants.is_object() {
        *descendants = Value::Object(Map::new());
    }
    match descendants {
        Value::Object(descendants) => descendants,
        _ => unreachable!("replaced by an object above"),
    }
}

/// Paint keys: their colour references stay bound, only shader uniforms
/// inside them are resolved at import.
const PAINT_KEYS: &[&str] = &["fill", "stroke", "effect"];

/// Node keys that are identity or structure, never a themed value.
const STRUCTURE_KEYS: &[&str] = &[
    "type",
    "id",
    "name",
    "ref",
    "theme",
    "children",
    "descendants",
    "reusable",
];

/// One field in a component whose value reads variables, as written.
struct ThemedField {
    id: String,
    key: String,
    value: Value,
    /// The variables it reads.
    names: Vec<String>,
}

/// A component's variable-reading fields and the theme it resolves them in.
struct ThemedComponent {
    theme: Rc<Theme>,
    fields: Vec<ThemedField>,
}

/// What the file says about component nodes before any conversion: each
/// node's own stroke keys, the component a nested instance points at, and
/// each component's fields that read variables. Only nodes inside a
/// component are indexed: instance patches never reach anything else.
#[derive(Default)]
pub(super) struct InstanceBases {
    strokes: HashMap<String, Map<String, Value>>,
    /// Nested instance id → its `ref` target.
    refs: HashMap<String, String>,
    /// Component id → its variable-reading fields.
    themed: HashMap<String, ThemedComponent>,
}

impl InstanceBases {
    /// Index the document's components. Iterative like the walk; a
    /// duplicate id keeps the first node indexed.
    pub(super) fn from_children(children: &[Value], variables: &Variables) -> Self {
        let mut bases = Self::default();
        let root_theme = Rc::new(variables.default_theme().clone());
        let mut stack: Vec<(&Value, bool, Rc<Theme>)> = children
            .iter()
            .map(|kid| (kid, false, Rc::clone(&root_theme)))
            .collect();
        while let Some((node, in_component, theme)) = stack.pop() {
            let Value::Object(map) = node else {
                continue;
            };
            let theme = variables::scoped_theme(map, &theme).map_or(theme, Rc::new);
            let reusable = map.get("reusable").and_then(Value::as_bool) == Some(true);
            let in_component = in_component || reusable;
            if let Some(id) = map
                .get("id")
                .and_then(Value::as_str)
                .filter(|_| in_component)
            {
                let keys: Map<String, Value> = STROKE_KEYS
                    .iter()
                    .filter_map(|key| Some(((*key).to_owned(), map.get(*key)?.clone())))
                    .collect();
                if !keys.is_empty() {
                    bases.strokes.entry(id.to_owned()).or_insert(keys);
                }
                if let Some(target) = map.get("ref").and_then(Value::as_str) {
                    bases
                        .refs
                        .entry(id.to_owned())
                        .or_insert_with(|| target.to_owned());
                }
                if reusable {
                    let fields = themed_fields(node);
                    if !fields.is_empty() {
                        let component = ThemedComponent {
                            theme: Rc::clone(&theme),
                            fields,
                        };
                        bases.themed.insert(id.to_owned(), component);
                    }
                }
            }
            if let Some(Value::Array(kids)) = map.get("children") {
                stack.extend(
                    kids.iter()
                        .map(|kid| (kid, in_component, Rc::clone(&theme))),
                );
            }
        }
        bases
    }

    /// The stroke keys of the node a patch keyed `path` applies to: the last
    /// segment of a pen.dev id path names that node. A nested instance
    /// without a stroke of its own shows its component's.
    pub(super) fn stroke(&self, path: &str) -> Option<&Map<String, Value>> {
        let mut id = path.rsplit('/').next().unwrap_or(path);
        // Bounded: a ref cycle must not loop.
        for _ in 0..8 {
            if let Some(keys) = self.strokes.get(id) {
                return Some(keys);
            }
            id = self.refs.get(id)?;
        }
        None
    }

    /// Give an instance its component's variable-reading fields as
    /// patches where the instance's theme resolves those variables to other
    /// values than the component's own theme does, so they resolve under the
    /// instance's theme. A field the instance already overrides is left
    /// alone.
    pub(super) fn add_themed_fields(
        &self,
        map: &mut Map<String, Value>,
        theme: &Theme,
        variables: &Variables,
    ) {
        if map.get("type").and_then(Value::as_str) != Some("ref") {
            return;
        }
        let Some(component) = map
            .get("ref")
            .and_then(Value::as_str)
            .and_then(|target| self.themed.get(target))
        else {
            return;
        };
        if *component.theme == *theme {
            return;
        }
        let differs = |field: &&ThemedField| {
            field
                .names
                .iter()
                .any(|name| variables.value(name, &component.theme) != variables.value(name, theme))
        };
        let changed: Vec<&ThemedField> = component.fields.iter().filter(differs).collect();
        if changed.is_empty() {
            return;
        }
        let descendants = descendants_mut(map);
        for field in changed {
            let patch = descendants
                .entry(field.id.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(patch) = patch {
                if !patch.contains_key("type") {
                    patch
                        .entry(field.key.clone())
                        .or_insert_with(|| field.value.clone());
                }
            }
        }
    }
}

/// The fields in a component's own subtree (not inside nested instances)
/// whose value the import resolves from variables: any reference outside a
/// paint, and shader uniforms inside one.
fn themed_fields(component: &Value) -> Vec<ThemedField> {
    let mut out = Vec::new();
    let mut stack = vec![component];
    while let Some(node) = stack.pop() {
        let Value::Object(map) = node else {
            continue;
        };
        if let Some(id) = map.get("id").and_then(Value::as_str) {
            for (key, value) in map {
                if STRUCTURE_KEYS.contains(&key.as_str()) {
                    continue;
                }
                let mut names = Vec::new();
                if PAINT_KEYS.contains(&key.as_str()) {
                    if reads_variable_uniform(value) {
                        collect_uniform_references(value, &mut names);
                    }
                } else {
                    collect_references(value, &mut names);
                }
                if !names.is_empty() {
                    out.push(ThemedField {
                        id: id.to_owned(),
                        key: key.clone(),
                        value: value.clone(),
                        names,
                    });
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

/// Every `$variable` named anywhere in `value`.
fn collect_references(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if let Some(name) = variables::reference(text) {
                names.push(name.to_owned());
            }
        }
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_references(item, names)),
        Value::Object(obj) => obj
            .values()
            .for_each(|item| collect_references(item, names)),
        _ => {}
    }
}

/// The `$variable`s named in a paint's shader uniforms.
fn collect_uniform_references(paint: &Value, names: &mut Vec<String>) {
    let shaders: Vec<&Value> = match paint {
        Value::Array(items) => items.iter().collect(),
        single => vec![single],
    };
    for shader in shaders {
        if let Some(uniforms) = shader.get("uniforms") {
            collect_references(uniforms, names);
        }
    }
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
                        .any(|value| value.as_str().and_then(variables::reference).is_some())
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
