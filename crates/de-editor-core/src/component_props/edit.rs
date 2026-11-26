//! Component-property writes: declare / change / rename / remove a
//! master's property, and set an instance's values. Each is one undo step.
//!
//! Every write is planned against the document first ([`plan`]), so MCP
//! can return the same refusal the apply would give, with its message.

use jian_ops_schema::component_props::{
    ComponentProperties, ComponentProperty, ComponentPropertyType, PropertyBinding,
};
use jian_ops_schema::node::{PenNode, RefNode};
use jian_ops_schema::PenDocument;
use serde_json::{Map, Value};

use super::check::{properties_of, PropsContext};
use super::resolve::{bound_value, field_patch, props_patches, PROPS_KEY};
use super::PropsError;
use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;

/// A component-property write, carried by
/// [`crate::EditorCommand::ComponentProperty`].
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentPropertyCommand {
    /// Declare a property or change one (see [`PropertyUpsert`]).
    Upsert(PropertyUpsert),
    /// Drop a property. Instances keep what they showed: their values move
    /// into `descendants` overrides.
    Remove { component_id: String, name: String },
    /// Set an instance's values by name; `null` clears one.
    SetInstanceProps {
        node_id: String,
        props: Map<String, Value>,
    },
}

/// Declare or change one property of a master. Fields left `None` keep
/// the property's current value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertyUpsert {
    pub component_id: String,
    /// The property's (new) name.
    pub name: String,
    /// Rename the property `rename_from` to `name`; every instance's
    /// value follows.
    pub rename_from: Option<String>,
    pub kind: Option<ComponentPropertyType>,
    /// Written into the bound nodes of the master, so the master and every
    /// instance without a value show it. A new property without one takes
    /// its first bound node's current value.
    pub default: Option<Value>,
    pub bind: Option<Vec<PropertyBinding>>,
    pub preferred: Option<Vec<String>>,
}

/// The checked outcome of an upsert.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedUpsert {
    pub property: ComponentProperty,
    /// The default to write into the bound nodes, when the call gave one.
    pub write_default: Option<Value>,
}

impl PropertyUpsert {
    /// Check the upsert against the document and build the property.
    pub fn plan(&self, context: PropsContext<'_>) -> Result<PlannedUpsert, PropsError> {
        let master = context.master(&self.component_id)?;
        if !matches!(master, PenNode::Frame(_)) {
            return Err(PropsError::ComponentNotFound(self.component_id.clone()));
        }
        let name = self.name.trim();
        if name.is_empty() || name.contains(['.', '/']) {
            return Err(PropsError::InvalidName(self.name.clone()));
        }
        let empty = ComponentProperties::new();
        let properties = properties_of(master).unwrap_or(&empty);
        let source = self.rename_from.as_deref().unwrap_or(name);
        let existing = properties.get(source);
        if self.rename_from.is_some() {
            if existing.is_none() {
                return Err(PropsError::UnknownProperty {
                    component: self.component_id.clone(),
                    name: source.to_string(),
                    allowed: super::check::describe(properties),
                });
            }
            if source != name && properties.contains_key(name) {
                return Err(PropsError::NameTaken(name.to_string()));
            }
        }
        let kind = match (&self.kind, existing) {
            (Some(ComponentPropertyType::Other(kind)), _) => {
                return Err(PropsError::UnknownType(kind.clone()))
            }
            (Some(kind), _) => kind.clone(),
            (None, Some(existing)) => existing.kind.clone(),
            (None, None) => return Err(PropsError::MissingType(name.to_string())),
        };
        if existing.is_some_and(|existing| existing.kind != kind) {
            let instances = super::summary::instances_setting(context.doc, master.id_str(), source);
            if instances > 0 {
                return Err(PropsError::TypeChangeInUse {
                    name: source.to_string(),
                    instances,
                });
            }
        }
        let bind = match (&self.bind, existing) {
            (Some(bind), _) => bind.clone(),
            (None, Some(existing)) => existing.bind.clone(),
            (None, None) => Vec::new(),
        };
        // New bindings, or old ones under a new type, must fit; kept ones
        // may have lost their node and stay reported as unbound.
        if self.bind.is_some() || existing.is_some_and(|existing| existing.kind != kind) {
            for binding in &bind {
                context.check_binding(master, name, &kind, &binding.node, &binding.field)?;
            }
        }
        let preferred = match (&self.preferred, existing) {
            (Some(preferred), _) => Some(preferred.clone()).filter(|list| !list.is_empty()),
            (None, existing) => existing.and_then(|existing| existing.preferred.clone()),
        };
        let probe = ComponentProperty {
            kind: kind.clone(),
            default: None,
            bind: bind.clone(),
            preferred: preferred.clone(),
        };
        for id in preferred.iter().flatten() {
            context.check_value(name, &probe, &Value::String(id.clone()), properties)?;
        }
        if let Some(default) = &self.default {
            context.check_value(name, &probe, default, properties)?;
        }
        let derived = || {
            bind.iter().find_map(|binding| {
                let node = super::check::find_in_subtree(master, &binding.node)?;
                bound_value(node, &binding.field)
            })
        };
        let default = self
            .default
            .clone()
            .or_else(|| existing.and_then(|existing| existing.default.clone()))
            .or_else(derived);
        Ok(PlannedUpsert {
            property: ComponentProperty {
                kind,
                default,
                bind,
                preferred,
            },
            write_default: self.default.clone(),
        })
    }
}

impl ComponentPropertyCommand {
    /// Check the command against the document without changing it.
    pub fn check(&self, context: PropsContext<'_>) -> Result<(), PropsError> {
        match self {
            Self::Upsert(upsert) => upsert.plan(context).map(|_| ()),
            Self::Remove { component_id, name } => {
                let master = context.master(component_id)?;
                let empty = ComponentProperties::new();
                let properties = properties_of(master).unwrap_or(&empty);
                if properties.contains_key(name) {
                    Ok(())
                } else {
                    Err(PropsError::UnknownProperty {
                        component: component_id.clone(),
                        name: name.clone(),
                        allowed: super::check::describe(properties),
                    })
                }
            }
            Self::SetInstanceProps { node_id, props } => {
                let instance = context.instance(node_id)?;
                context.check_props(&instance.target, props)
            }
        }
    }
}

impl EditorState {
    /// Apply a component-property write as one undo step.
    pub fn apply_component_property(
        &mut self,
        command: &ComponentPropertyCommand,
    ) -> Result<(), PropsError> {
        let context = PropsContext {
            doc: &self.doc,
            components: &self.components,
        };
        let planned = match command {
            ComponentPropertyCommand::Upsert(upsert) => Some(upsert.plan(context)?),
            other => {
                other.check(context)?;
                None
            }
        };
        let snapshot = self.snapshot_for_history();
        match (command, planned) {
            (ComponentPropertyCommand::Upsert(upsert), Some(planned)) => {
                apply_upsert(&mut self.doc, upsert, planned);
            }
            (ComponentPropertyCommand::Remove { component_id, name }, _) => {
                apply_remove(&mut self.doc, component_id, name);
            }
            (ComponentPropertyCommand::SetInstanceProps { node_id, props }, _) => {
                if let Some(PenNode::Ref(reference)) = find_node_mut(&mut self.doc, node_id) {
                    merge_props(reference, props);
                }
            }
            _ => return Ok(()),
        }
        self.history_push_past(snapshot);
        Ok(())
    }
}

fn apply_upsert(doc: &mut PenDocument, upsert: &PropertyUpsert, planned: PlannedUpsert) {
    let name = upsert.name.trim().to_string();
    let master_id = upsert.component_id.as_str();
    if let Some(old) = upsert.rename_from.as_deref().filter(|old| *old != name) {
        for_each_instance_mut(doc, master_id, |reference| {
            if let Some(value) = reference.props.as_mut().and_then(|props| props.remove(old)) {
                reference
                    .props
                    .get_or_insert_with(Default::default)
                    .insert(name.clone(), value);
            }
        });
        rename_exposures(doc, master_id, old, &name);
    }
    let Some(PenNode::Frame(master)) = find_node_mut(doc, master_id) else {
        return;
    };
    let properties = master
        .component_properties
        .get_or_insert_with(Default::default);
    if let Some(old) = &upsert.rename_from {
        properties.remove(old);
    }
    let bind = planned.property.bind.clone();
    properties.insert(name, planned.property);
    if let Some(default) = planned.write_default {
        let mut root = PenNode::Frame(master.clone());
        for binding in &bind {
            if let Some((field, value)) = field_patch(&binding.field, &default) {
                patch_node(&mut root, &binding.node, &field, &value);
            }
        }
        if let PenNode::Frame(patched) = root {
            *master = patched;
        }
    }
}

/// Outer masters that expose `old` of a nested instance of `master_id`
/// bind `props.<new>` instead.
fn rename_exposures(doc: &mut PenDocument, master_id: &str, old: &str, new: &str) {
    let nested_ids = {
        let mut ids = Vec::new();
        for_each_instance(doc, master_id, |reference| {
            ids.push(reference.base.id.clone())
        });
        ids
    };
    for_each_node_mut(doc, &mut |node| {
        let PenNode::Frame(frame) = node else {
            return;
        };
        for property in frame
            .component_properties
            .iter_mut()
            .flat_map(|p| p.values_mut())
        {
            for binding in &mut property.bind {
                if nested_ids.contains(&binding.node)
                    && binding.field
                        == jian_ops_schema::component_props::BindingField::Prop(old.into())
                {
                    binding.field =
                        jian_ops_schema::component_props::BindingField::Prop(new.into());
                }
            }
        }
    });
}

fn apply_remove(doc: &mut PenDocument, master_id: &str, name: &str) {
    let Some(property) = find_node_in(doc, master_id)
        .and_then(properties_of)
        .and_then(|properties| properties.get(name))
        .cloned()
    else {
        return;
    };
    let single = ComponentProperties::from([(name.to_string(), property)]);
    for_each_instance_mut(doc, master_id, |reference| {
        let Some(value) = reference
            .props
            .as_mut()
            .and_then(|props| props.remove(name))
        else {
            return;
        };
        if reference
            .props
            .as_ref()
            .is_some_and(|props| props.is_empty())
        {
            reference.props = None;
        }
        let only = Map::from_iter([(name.to_string(), value)]);
        let descendants = reference.descendants.get_or_insert_with(Default::default);
        for (node, patch) in props_patches(&single, &only) {
            keep_as_override(
                descendants.entry(node).or_insert(Value::Object(Map::new())),
                patch,
            );
        }
    });
    if let Some(PenNode::Frame(master)) = find_node_mut(doc, master_id) {
        if let Some(properties) = master.component_properties.as_mut() {
            properties.remove(name);
            if properties.is_empty() {
                master.component_properties = None;
            }
        }
    }
}

/// Add `patch`'s fields to an override entry, keeping what the entry sets
/// itself (an explicit override already won over the property).
fn keep_as_override(entry: &mut Value, patch: Value) {
    if crate::instance_descendants::is_replacement(entry) {
        return;
    }
    let (Value::Object(entry), Value::Object(patch)) = (entry, patch) else {
        return;
    };
    for (field, value) in patch {
        match (entry.get_mut(&field), value) {
            (Some(Value::Object(own)), Value::Object(added)) if field == PROPS_KEY => {
                for (key, value) in added {
                    own.entry(key).or_insert(value);
                }
            }
            (Some(_), _) => {}
            (None, value) => {
                entry.insert(field, value);
            }
        }
    }
}

fn merge_props(reference: &mut RefNode, props: &Map<String, Value>) {
    let own = reference.props.get_or_insert_with(Default::default);
    for (name, value) in props {
        if value.is_null() {
            own.remove(name);
        } else {
            own.insert(name.clone(), value.clone());
        }
    }
    if own.is_empty() {
        reference.props = None;
    }
}

/// Write `field = value` on node `id` inside `root` through its JSON form
/// (nested-instance `props` merge).
fn patch_node(root: &mut PenNode, id: &str, field: &str, value: &Value) {
    let Some(node) = find_in_subtree_mut(root, id) else {
        return;
    };
    let Ok(Value::Object(mut map)) = serde_json::to_value(&*node) else {
        return;
    };
    match (map.get_mut(field), value) {
        (Some(Value::Object(own)), Value::Object(added)) if field == PROPS_KEY => {
            own.extend(added.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        _ => {
            map.insert(field.to_string(), value.clone());
        }
    }
    if let Ok(patched) = serde_json::from_value::<PenNode>(Value::Object(map)) {
        *node = patched;
    }
}

fn find_in_subtree_mut<'a>(node: &'a mut PenNode, id: &str) -> Option<&'a mut PenNode> {
    if node.id_str() == id {
        return Some(node);
    }
    node.children_mut()?
        .iter_mut()
        .find_map(|child| find_in_subtree_mut(child, id))
}

fn find_node_in<'a>(doc: &'a PenDocument, id: &str) -> Option<&'a PenNode> {
    crate::component_backing::find_node_in_document(doc, id)
}

/// The node `id` on any page, mutably.
pub(crate) fn find_node_mut<'a>(doc: &'a mut PenDocument, id: &str) -> Option<&'a mut PenNode> {
    let node_id = crate::NodeId::new(id);
    if let Some(pages) = doc.pages.as_mut() {
        for page in pages {
            if crate::walkers::contains_node(&page.children, &node_id) {
                return crate::walkers::find_node_mut(&mut page.children, &node_id);
            }
        }
    }
    crate::walkers::find_node_mut(&mut doc.children, &node_id)
}

/// Visit every page-tree instance of `master_id`, on every page.
pub(crate) fn for_each_instance(
    doc: &PenDocument,
    master_id: &str,
    mut visit: impl FnMut(&RefNode),
) {
    fn walk(nodes: &[PenNode], master_id: &str, visit: &mut dyn FnMut(&RefNode)) {
        for node in nodes {
            if let PenNode::Ref(reference) = node {
                if reference.target == master_id {
                    visit(reference);
                }
            }
            if let Some(children) = node.children() {
                walk(children, master_id, visit);
            }
        }
    }
    for page in doc.pages.iter().flatten() {
        walk(&page.children, master_id, &mut visit);
    }
    walk(&doc.children, master_id, &mut visit);
}

fn for_each_instance_mut(
    doc: &mut PenDocument,
    master_id: &str,
    mut visit: impl FnMut(&mut RefNode),
) {
    for_each_node_mut(doc, &mut |node| {
        if let PenNode::Ref(reference) = node {
            if reference.target == master_id {
                visit(reference);
            }
        }
    });
}

fn for_each_node_mut(doc: &mut PenDocument, visit: &mut dyn FnMut(&mut PenNode)) {
    fn walk(nodes: &mut [PenNode], visit: &mut dyn FnMut(&mut PenNode)) {
        for node in nodes {
            visit(node);
            if let Some(children) = node.children_mut() {
                walk(children, visit);
            }
        }
    }
    for page in doc.pages.iter_mut().flatten() {
        walk(&mut page.children, visit);
    }
    walk(&mut doc.children, visit);
}
