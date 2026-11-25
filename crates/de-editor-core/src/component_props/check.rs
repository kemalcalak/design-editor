//! Validation of property values and bindings against a master.

use jian_ops_schema::component_props::{
    BindingField, ComponentProperties, ComponentProperty, ComponentPropertyType,
};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{Map, Value};

use super::PropsError;
use crate::component_backing::find_node_in_document;
use crate::pen_node_ext::PenNodeExt;
use crate::{ComponentLibrary, NodeId};

/// The document and registry a property check reads.
#[derive(Clone, Copy)]
pub struct PropsContext<'a> {
    pub doc: &'a PenDocument,
    pub components: &'a ComponentLibrary,
}

impl<'a> PropsContext<'a> {
    /// The master `id` names: a registered component or a `reusable` frame
    /// on any page.
    pub fn master(&self, id: &str) -> Result<&'a PenNode, PropsError> {
        let node_id = NodeId::new(id);
        if let Some(root) = self.components.resolved_root(self.doc, &node_id) {
            return Ok(root);
        }
        match find_node_in_document(self.doc, id) {
            Some(node @ PenNode::Frame(frame)) if frame.reusable == Some(true) => Ok(node),
            _ => Err(PropsError::ComponentNotFound(id.to_string())),
        }
    }

    /// Whether `id` names a component master.
    pub fn is_component(&self, id: &str) -> bool {
        self.master(id).is_ok()
    }

    /// The instance `id` names, on any page.
    pub fn instance(&self, id: &str) -> Result<&'a jian_ops_schema::node::RefNode, PropsError> {
        match find_node_in_document(self.doc, id) {
            Some(PenNode::Ref(reference)) => Ok(reference),
            _ => Err(PropsError::InstanceNotFound(id.to_string())),
        }
    }

    /// Check `props` (values by name; `null` clears one) for an instance of
    /// `master_id`.
    pub fn check_props(
        &self,
        master_id: &str,
        props: &Map<String, Value>,
    ) -> Result<(), PropsError> {
        let master = self.master(master_id)?;
        let empty = ComponentProperties::new();
        let properties = properties_of(master).unwrap_or(&empty);
        for (name, value) in props {
            let Some(property) = properties.get(name) else {
                return Err(PropsError::UnknownProperty {
                    component: master_id.to_string(),
                    name: name.clone(),
                    allowed: describe(properties),
                });
            };
            if !value.is_null() {
                self.check_value(name, property, value, properties)?;
            }
        }
        Ok(())
    }

    /// Check one value against its property's type.
    pub fn check_value(
        &self,
        name: &str,
        property: &ComponentProperty,
        value: &Value,
        all: &ComponentProperties,
    ) -> Result<(), PropsError> {
        if !super::resolve::value_fits(&property.kind, value) {
            return Err(PropsError::WrongType {
                name: name.to_string(),
                expected: property.kind.as_str().to_string(),
                got: json_kind(value).to_string(),
                allowed: describe(all),
            });
        }
        let text = value.as_str().unwrap_or_default();
        match property.kind {
            ComponentPropertyType::Instance if !self.is_component(text) => {
                Err(PropsError::UnknownComponent {
                    name: name.to_string(),
                    value: text.to_string(),
                })
            }
            ComponentPropertyType::Color if !self.is_colour(text) => {
                Err(PropsError::InvalidColor {
                    name: name.to_string(),
                    value: text.to_string(),
                })
            }
            _ => Ok(()),
        }
    }

    /// `#hex` or a `$variable` the document defines.
    pub fn is_colour(&self, value: &str) -> bool {
        match value.strip_prefix('$') {
            Some(variable) => self
                .doc
                .variables
                .as_ref()
                .is_some_and(|variables| variables.contains_key(variable)),
            None => {
                value.starts_with('#')
                    && de_util::hex_color::parse_hex_rgba8(
                        value,
                        de_util::hex_color::HexOptions::LENIENT,
                    )
                    .is_some()
            }
        }
    }

    /// Check that `field` of `node` (inside `master`) can carry a property
    /// of `kind`; returns the bound node.
    pub fn check_binding(
        &self,
        master: &'a PenNode,
        name: &str,
        kind: &ComponentPropertyType,
        node: &str,
        field: &BindingField,
    ) -> Result<(), PropsError> {
        let target = find_in_subtree(master, node).ok_or_else(|| PropsError::BindNodeNotFound {
            node: node.to_string(),
            component: master.id_str().to_string(),
        })?;
        let mismatch = |reason: &str| PropsError::BindMismatch {
            name: name.to_string(),
            node: node.to_string(),
            field: field.to_wire(),
            reason: reason.to_string(),
        };
        let expected = match field {
            BindingField::Content => {
                if !matches!(target, PenNode::Text(_)) {
                    return Err(mismatch("content binds a text node"));
                }
                ComponentPropertyType::Text
            }
            BindingField::Visible => ComponentPropertyType::Boolean,
            BindingField::Ref => {
                if !matches!(target, PenNode::Ref(_)) {
                    return Err(mismatch("ref binds a nested instance"));
                }
                ComponentPropertyType::Instance
            }
            BindingField::Fill => {
                if matches!(target, PenNode::Ref(_)) {
                    return Err(mismatch(
                        "bind the fill of a node inside the nested instance's master",
                    ));
                }
                ComponentPropertyType::Color
            }
            BindingField::Prop(inner) => {
                let PenNode::Ref(reference) = target else {
                    return Err(mismatch("props.<name> binds a nested instance"));
                };
                let nested = self.master(&reference.target)?;
                let inner_kind = properties_of(nested)
                    .and_then(|properties| properties.get(inner))
                    .map(|property| property.kind.clone())
                    .ok_or_else(|| mismatch("the nested component has no such property"))?;
                inner_kind
            }
            BindingField::Other(_) => return Err(mismatch("unknown field")),
        };
        if &expected != kind {
            return Err(mismatch(&format!(
                "it carries a {} value, the property is {}",
                expected.as_str(),
                kind.as_str()
            )));
        }
        Ok(())
    }
}

/// A master's declared properties.
pub fn properties_of(master: &PenNode) -> Option<&ComponentProperties> {
    match master {
        PenNode::Frame(frame) => frame.component_properties.as_ref(),
        _ => None,
    }
}

/// `label (text), showIcon (boolean)`: the properties an error offers.
pub fn describe(properties: &ComponentProperties) -> String {
    if properties.is_empty() {
        return "none (the component declares no properties)".into();
    }
    properties
        .iter()
        .map(|(name, property)| format!("{name} ({})", property.kind.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The node `id` inside `root`'s subtree, `root` included.
pub fn find_in_subtree<'a>(root: &'a PenNode, id: &str) -> Option<&'a PenNode> {
    if root.id_str() == id {
        return Some(root);
    }
    root.children()
        .into_iter()
        .flatten()
        .find_map(|child| find_in_subtree(child, id))
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}
