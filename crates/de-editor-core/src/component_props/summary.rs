//! What a master's properties look like to a reader: the MCP
//! `get_component` answer and the property panel's rows.

use jian_ops_schema::component_props::{BindingField, ComponentPropertyType};
use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use super::check::{find_in_subtree, properties_of, PropsContext};
use super::resolve::bound_value;

/// One declared property with its live default and any broken bindings.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertySummary {
    pub name: String,
    pub kind: ComponentPropertyType,
    /// The first bound node's current value, else the stored default.
    pub default: Option<Value>,
    pub preferred: Vec<String>,
    /// `node.field` of every binding.
    pub bind: Vec<(String, BindingField)>,
    /// Bindings whose node left the master, or whose nested component lost
    /// the exposed property.
    pub unbound: Vec<(String, BindingField)>,
}

impl PropertySummary {
    /// True when no binding reaches a node: the property drives nothing.
    pub fn is_unbound(&self) -> bool {
        self.bind.len() == self.unbound.len()
    }

    /// The compact JSON form MCP returns.
    pub fn to_json(&self) -> Value {
        let mut out = json!({
            "name": self.name,
            "type": self.kind.as_str(),
            "bind": self.bind.iter().map(binding_json).collect::<Vec<_>>(),
        });
        if let Some(default) = &self.default {
            out["default"] = default.clone();
        }
        if !self.preferred.is_empty() {
            out["preferred"] = json!(self.preferred);
        }
        if !self.unbound.is_empty() {
            out["unbound"] = Value::Array(self.unbound.iter().map(binding_json).collect());
        }
        out
    }
}

fn binding_json((node, field): &(String, BindingField)) -> Value {
    json!({ "node": node, "field": field.to_wire() })
}

/// The properties of `master`, in name order.
pub fn property_summaries(context: PropsContext<'_>, master: &PenNode) -> Vec<PropertySummary> {
    let Some(properties) = properties_of(master) else {
        return Vec::new();
    };
    properties
        .iter()
        .map(|(name, property)| {
            let mut live = None;
            let mut unbound = Vec::new();
            for binding in &property.bind {
                let node = find_in_subtree(master, &binding.node);
                let reachable = match (node, &binding.field) {
                    (Some(PenNode::Ref(reference)), BindingField::Prop(inner)) => context
                        .master(&reference.target)
                        .ok()
                        .and_then(properties_of)
                        .is_some_and(|nested| nested.contains_key(inner)),
                    (Some(_), BindingField::Other(_)) | (None, _) => false,
                    (Some(_), _) => true,
                };
                if !reachable {
                    unbound.push((binding.node.clone(), binding.field.clone()));
                    continue;
                }
                if live.is_none() {
                    live = node.and_then(|node| bound_value(node, &binding.field));
                }
            }
            PropertySummary {
                name: name.clone(),
                kind: property.kind.clone(),
                default: live.or_else(|| property.default.clone()),
                preferred: property.preferred.clone().unwrap_or_default(),
                bind: property
                    .bind
                    .iter()
                    .map(|binding| (binding.node.clone(), binding.field.clone()))
                    .collect(),
                unbound,
            }
        })
        .collect()
}

/// How many instances of `master_id` (any page) set a value for `name`.
pub fn instances_setting(doc: &jian_ops_schema::PenDocument, master_id: &str, name: &str) -> usize {
    let mut count = 0;
    super::edit::for_each_instance(doc, master_id, |reference| {
        if reference
            .props
            .as_ref()
            .is_some_and(|props| props.contains_key(name))
        {
            count += 1;
        }
    });
    count
}

/// Short `name|type` list for `list_components`: `label:text,icon:instance`.
pub fn property_names(master: &PenNode) -> String {
    properties_of(master)
        .map(|properties| {
            properties
                .iter()
                .map(|(name, property)| format!("{name}:{}", property.kind.as_str()))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default()
}
