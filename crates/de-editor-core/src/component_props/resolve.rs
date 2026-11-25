//! Turn an instance's `props` into `descendants` patches before the
//! instance expands (render, inspector display, detach).

use jian_ops_schema::component_props::{BindingField, ComponentProperties, ComponentPropertyType};
use serde_json::{Map, Value};

/// The node key holding a master's properties.
pub(crate) const PROPERTIES_KEY: &str = "componentProperties";
/// The instance key holding its property values.
pub(crate) const PROPS_KEY: &str = "props";

/// Move the instance's `props` into its `descendants`, in place on the
/// instance's JSON map, so the existing override path renders them.
///
/// Each value becomes a patch on every node its property binds; an explicit
/// `descendants` entry wins field by field (a replacement entry wins
/// whole). Values of an unknown property or of the wrong type are ignored.
/// `props` is removed from the map either way.
pub(crate) fn fold_props_into_descendants(
    component: &Map<String, Value>,
    instance: &mut Map<String, Value>,
) {
    let Some(Value::Object(props)) = instance.remove(PROPS_KEY) else {
        return;
    };
    let Some(properties) = component.get(PROPERTIES_KEY) else {
        return;
    };
    let Ok(properties) = serde_json::from_value::<ComponentProperties>(properties.clone()) else {
        return;
    };
    let mut patches = props_patches(&properties, &props);
    if patches.is_empty() {
        return;
    }
    if let Some(Value::Object(explicit)) = instance.remove("descendants") {
        for (key, entry) in explicit {
            match (patches.get_mut(&key), &entry) {
                (Some(Value::Object(patch)), Value::Object(fields))
                    if !crate::instance_descendants::is_replacement(&entry) =>
                {
                    for (field, value) in fields {
                        overlay_field(patch, field, value);
                    }
                }
                _ => {
                    patches.insert(key, entry);
                }
            }
        }
    }
    instance.insert("descendants".into(), Value::Object(patches));
}

/// The `descendants` patches `props` stand for, keyed by bound node id.
pub(crate) fn props_patches(
    properties: &ComponentProperties,
    props: &Map<String, Value>,
) -> Map<String, Value> {
    let mut patches = Map::new();
    for (name, value) in props {
        let Some(property) = properties.get(name) else {
            continue;
        };
        if !value_fits(&property.kind, value) {
            continue;
        }
        for binding in &property.bind {
            let Some((field, field_value)) = field_patch(&binding.field, value) else {
                continue;
            };
            let entry = patches
                .entry(binding.node.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(patch) = entry {
                overlay_field(patch, &field, &field_value);
            }
        }
    }
    patches
}

/// The node field and value one binding writes for `value`.
pub(crate) fn field_patch(field: &BindingField, value: &Value) -> Option<(String, Value)> {
    Some(match field {
        BindingField::Content => ("content".into(), value.clone()),
        BindingField::Visible => ("visible".into(), value.clone()),
        BindingField::Ref => ("ref".into(), value.clone()),
        BindingField::Fill => (
            "fill".into(),
            serde_json::json!([{ "type": "solid", "color": value }]),
        ),
        BindingField::Prop(name) => {
            let mut nested = Map::new();
            nested.insert(name.clone(), value.clone());
            (PROPS_KEY.into(), Value::Object(nested))
        }
        BindingField::Other(_) => return None,
    })
}

/// Set one patch field; nested-instance `props` merge name by name, the
/// later value winning.
fn overlay_field(patch: &mut Map<String, Value>, field: &str, value: &Value) {
    if field == PROPS_KEY {
        if let (Some(Value::Object(own)), Value::Object(added)) = (patch.get_mut(field), value) {
            own.extend(added.iter().map(|(k, v)| (k.clone(), v.clone())));
            return;
        }
    }
    patch.insert(field.to_owned(), value.clone());
}

/// Whether `value` has the JSON shape of `kind` (the resolver's lenient
/// check; MCP and the panel check ids and colours too).
pub(crate) fn value_fits(kind: &ComponentPropertyType, value: &Value) -> bool {
    match kind {
        ComponentPropertyType::Text
        | ComponentPropertyType::Instance
        | ComponentPropertyType::Color => value.is_string(),
        ComponentPropertyType::Boolean => value.is_boolean(),
        ComponentPropertyType::Other(_) => true,
    }
}

/// The value a binding's node shows now, read from the master's JSON.
pub(crate) fn bound_value(node: &Value, field: &BindingField) -> Option<Value> {
    match field {
        BindingField::Content => match node.get("content")? {
            Value::String(text) => Some(Value::String(text.clone())),
            Value::Array(runs) => Some(Value::String(
                runs.iter()
                    .filter_map(|run| run.get("text").and_then(Value::as_str))
                    .collect(),
            )),
            _ => None,
        },
        BindingField::Visible => Some(node.get("visible").cloned().unwrap_or(Value::Bool(true))),
        BindingField::Ref => node.get("ref").cloned(),
        BindingField::Fill => node
            .get("fill")
            .and_then(Value::as_array)
            .and_then(|fills| fills.first())
            .filter(|fill| fill.get("type").and_then(Value::as_str) == Some("solid"))
            .and_then(|fill| fill.get("color"))
            .cloned(),
        BindingField::Prop(name) => node
            .get(PROPS_KEY)
            .and_then(|props| props.get(name))
            .cloned(),
        BindingField::Other(_) => None,
    }
}
