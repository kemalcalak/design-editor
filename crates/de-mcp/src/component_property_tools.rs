//! Component-property MCP tools (#367): `upsert_component_property`
//! declares or changes a master's property (rename with `newName`),
//! `remove_component_property` drops one while instances keep what they
//! showed. Instance values are written by `batch_design` `I()` / `U()` and
//! `update_node` (`props`), checked by [`check_instance_props`].
//!
//! Every refusal is the core's [`PropsError`] message, which names the
//! allowed properties, types or values.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;

use de_editor_core::component_props::{
    property_summaries, ComponentPropertyCommand, PropertyUpsert, PropsContext, PropsError,
};
use de_editor_core::{ComponentLibrary, EditorCommand, EditorState};
use jian_ops_schema::component_props::{ComponentPropertyType, PropertyBinding};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{json, Map, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

/// The document and registry a property tool checks against.
pub struct PropsSnapshot {
    doc: PenDocument,
    components: ComponentLibrary,
}

impl PropsSnapshot {
    pub fn new(state: &EditorState) -> Self {
        Self {
            doc: state.doc.clone(),
            components: state.components.clone(),
        }
    }

    pub fn context(&self) -> PropsContext<'_> {
        PropsContext {
            doc: &self.doc,
            components: &self.components,
        }
    }
}

fn arg<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|key| args.get(*key))
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
}

fn required<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Result<&'a str, ToolOutcome> {
    arg(args, keys).ok_or_else(|| {
        ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            format!("{} is required", keys[0]),
        )
    })
}

fn invalid(message: impl ToString) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message.to_string())
}

fn json_arg<T: serde::de::DeserializeOwned>(
    args: &BTreeMap<String, String>,
    key: &str,
    shape: &str,
) -> Result<Option<T>, ToolOutcome> {
    match arg(args, &[key]) {
        None => Ok(None),
        Some(raw) => serde_json::from_str(raw)
            .map(Some)
            .map_err(|_| invalid(format!("{key} must be {shape}"))),
    }
}

/// `default` arrives as text; read it the way the property's type needs.
fn default_value(raw: &str, kind: &ComponentPropertyType) -> Value {
    match kind {
        ComponentPropertyType::Boolean => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            other => Value::String(other.to_string()),
        },
        _ => Value::String(raw.to_string()),
    }
}

/// `upsert_component_property`.
pub struct UpsertComponentProperty(PropsSnapshot);

pub fn upsert_component_property_snapshot(state: &EditorState) -> UpsertComponentProperty {
    UpsertComponentProperty(PropsSnapshot::new(state))
}

impl UpsertComponentProperty {
    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolOutcome> {
        let context = self.0.context();
        let component_id = required(args, &["component_id", "componentId"])?.to_string();
        let name = required(args, &["name"])?.to_string();
        let new_name = arg(args, &["newName", "new_name"]).map(str::to_string);
        let kind =
            arg(args, &["type", "kind"]).map(|raw| ComponentPropertyType::from(raw.to_string()));
        let bind: Option<Vec<PropertyBinding>> = json_arg(
            args,
            "bind",
            "an array of {node, field} (field: content, visible, ref, fill or props.<name>)",
        )?;
        let preferred: Option<Vec<String>> =
            json_arg(args, "preferred", "an array of component ids")?;
        let master = context.master(&component_id).map_err(invalid)?;
        let existing_kind = de_editor_core::component_props::properties_of(master)
            .and_then(|properties| properties.get(&name))
            .map(|property| property.kind.clone());
        let effective_kind = kind
            .clone()
            .or(existing_kind)
            .unwrap_or(ComponentPropertyType::Text);
        let upsert = PropertyUpsert {
            component_id: component_id.clone(),
            name: new_name.clone().unwrap_or_else(|| name.clone()),
            rename_from: new_name.as_ref().map(|_| name.clone()),
            kind,
            default: arg(args, &["default"]).map(|raw| default_value(raw, &effective_kind)),
            bind,
            preferred,
        };
        let planned = upsert.plan(context).map_err(invalid)?;
        let final_name = upsert.name.trim().to_string();
        let mut result = json!({
            "componentId": component_id,
            "name": final_name,
            "type": planned.property.kind.as_str(),
            "bind": planned.property.bind.iter().map(|b| json!({"node": b.node, "field": b.field.to_wire()})).collect::<Vec<_>>(),
        });
        if let Some(default) = &planned.property.default {
            result["default"] = default.clone();
        }
        if let Some(old) = &upsert.rename_from {
            result["renamedFrom"] = json!(old);
        }
        Ok(ToolOutcome::OkJsonWithCommand(
            result.to_string(),
            EditorCommand::ComponentProperty(Box::new(ComponentPropertyCommand::Upsert(upsert))),
        ))
    }
}

impl McpTool for UpsertComponentProperty {
    fn name(&self) -> &str {
        "upsert_component_property"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args).unwrap_or_else(|error| error)
    }
}

/// `remove_component_property`.
pub struct RemoveComponentProperty(PropsSnapshot);

pub fn remove_component_property_snapshot(state: &EditorState) -> RemoveComponentProperty {
    RemoveComponentProperty(PropsSnapshot::new(state))
}

impl McpTool for RemoveComponentProperty {
    fn name(&self) -> &str {
        "remove_component_property"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let run = || -> Result<ToolOutcome, ToolOutcome> {
            let component_id = required(args, &["component_id", "componentId"])?.to_string();
            let name = required(args, &["name"])?.to_string();
            let command = ComponentPropertyCommand::Remove {
                component_id: component_id.clone(),
                name: name.clone(),
            };
            command.check(self.0.context()).map_err(invalid)?;
            let kept = instances_setting(&self.0.doc, &component_id, &name);
            let result = json!({
                "componentId": component_id,
                "removed": name,
                "instancesKeptAsOverrides": kept,
            });
            Ok(ToolOutcome::OkJsonWithCommand(
                result.to_string(),
                EditorCommand::ComponentProperty(Box::new(command)),
            ))
        };
        run().unwrap_or_else(|error| error)
    }
}

fn instances_setting(doc: &PenDocument, master_id: &str, name: &str) -> usize {
    fn walk(nodes: &[PenNode], master_id: &str, name: &str) -> usize {
        nodes
            .iter()
            .map(|node| {
                let own = matches!(node, PenNode::Ref(reference)
                    if reference.target == master_id
                        && reference.props.as_ref().is_some_and(|props| props.contains_key(name)));
                usize::from(own)
                    + de_editor_core::PenNodeExt::children(node)
                        .map_or(0, |children| walk(children, master_id, name))
            })
            .sum()
    }
    doc.pages
        .iter()
        .flatten()
        .map(|page| walk(&page.children, master_id, name))
        .sum::<usize>()
        + walk(&doc.children, master_id, name)
}

/// Check every instance in `nodes` (a subtree about to be inserted) whose
/// `props` the master would not accept.
pub fn check_inserted_props(
    context: PropsContext<'_>,
    nodes: &[PenNode],
) -> Result<(), PropsError> {
    for node in nodes {
        if let PenNode::Ref(reference) = node {
            if let Some(props) = reference.props.as_ref() {
                let props: Map<String, Value> = props.clone().into_iter().collect();
                context.check_props(&reference.target, &props)?;
            }
        }
        if let Some(children) = de_editor_core::PenNodeExt::children(node) {
            check_inserted_props(context, children)?;
        }
    }
    Ok(())
}

/// Split `props` off an update patch for the instance `node_id`: the
/// values merge into the instance's own (`null` clears one) instead of
/// replacing them. `Ok(None)` when the patch has no `props`.
pub fn take_props_update(
    context: Option<PropsContext<'_>>,
    node_id: &str,
    patch: &mut Value,
) -> Result<Option<ComponentPropertyCommand>, PropsError> {
    let Some(Value::Object(props)) = patch.as_object_mut().and_then(|obj| obj.remove("props"))
    else {
        return Ok(None);
    };
    let command = ComponentPropertyCommand::SetInstanceProps {
        node_id: node_id.to_string(),
        props,
    };
    if let Some(context) = context {
        command.check(context)?;
    }
    Ok(Some(command))
}

/// The `properties` JSON `get_component` returns for one master.
pub fn properties_json(context: PropsContext<'_>, master: &PenNode) -> Option<String> {
    let summaries = property_summaries(context, master);
    (!summaries.is_empty()).then(|| {
        Value::Array(summaries.iter().map(|summary| summary.to_json()).collect()).to_string()
    })
}

/// `update_node` with `data.props` on an instance: the values merge by
/// name (one `SetInstanceProps`); the rest of `data` stays a shallow
/// patch. `None` when `data` carries no `props`.
pub(crate) fn update_node_props(
    snapshot: Option<&PropsSnapshot>,
    node_id: &de_editor_core::NodeId,
    args: &BTreeMap<String, String>,
) -> Option<ToolOutcome> {
    let mut data: Value = serde_json::from_str(args.get("data")?).ok()?;
    data.get("props")?;
    let command = match take_props_update(
        snapshot.map(PropsSnapshot::context),
        node_id.as_str(),
        &mut data,
    ) {
        Ok(Some(command)) => command,
        Ok(None) => return Some(invalid("props must be an object of values by name")),
        Err(error) => return Some(invalid(error)),
    };
    let props = EditorCommand::ComponentProperty(Box::new(command));
    let mut out = BTreeMap::new();
    out.insert("wrote".into(), "true".into());
    if data.as_object().is_none_or(Map::is_empty) {
        return Some(ToolOutcome::OkWithCommand(out, props));
    }
    let patch_json = match crate::update_node_data::ts_update_patch_json_value(&data) {
        Ok(Some(patch)) => patch,
        Ok(None) => data.to_string(),
        Err(error) => return Some(invalid(error)),
    };
    let page_id = arg(args, &["pageId", "page_id", "page"]).map(str::to_string);
    let patch = EditorCommand::PatchNodeData {
        node_id: node_id.clone(),
        patch_json,
        page_id,
    };
    Some(ToolOutcome::OkWithCommand(
        out,
        EditorCommand::Batch {
            commands: vec![patch, props],
        },
    ))
}
