//! Component-set MCP tools (#368): `create_component_set` combines masters
//! into a set (axes inferred from `key=value` names when omitted),
//! `add_variant` / `add_variant_axis` / `rename_variant_axis` /
//! `rename_variant_value` / `remove_variant_value` edit it, and
//! `set_instance_variant` switches an instance. `list_components` and
//! `get_component` describe sets through [`sets_json`] / [`set_detail`].
//!
//! Each tool runs its command on a copy of the snapshot first, so a refusal
//! is the core's [`VariantError`] message (it names the allowed axes or
//! values) and the result carries the set as it will be plus `warnings`
//! for fallbacks and dropped overrides.
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, HashSet};

use de_editor_core::component_set::{
    preview_command, set_summaries, set_summary, ComponentSetCommand, CreateSet, VariantError,
    VariantReport,
};
use de_editor_core::{EditorCommand, EditorState};
use jian_ops_schema::component_set::{VariantAxes, VariantValues};
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

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
    keys: &[&str],
    shape: &str,
) -> Result<Option<T>, ToolOutcome> {
    match arg(args, keys) {
        None => Ok(None),
        Some(raw) => serde_json::from_str(raw)
            .map(Some)
            .map_err(|_| invalid(format!("{} must be {shape}", keys[0]))),
    }
}

/// A list of ids: a JSON array, or ids separated by commas.
fn id_list(raw: &str) -> Vec<String> {
    match serde_json::from_str::<Vec<String>>(raw) {
        Ok(ids) => ids,
        Err(_) => raw
            .split(',')
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect(),
    }
}

/// The snapshot every set tool checks against.
pub struct SetSnapshot {
    doc: PenDocument,
    tool: &'static str,
}

/// The JSON a set tool answers with: the set after the write, and the
/// report's warnings.
fn answer(doc: &PenDocument, report: &VariantReport, mut extra: Value) -> String {
    if let Some(summary) = report.set_id.as_deref().and_then(|id| set_summary(doc, id)) {
        extra["set"] = summary.to_json();
    }
    if !report.created.is_empty() {
        extra["created"] = json!(report.created);
    }
    if !report.removed.is_empty() {
        extra["removed"] = json!(report.removed);
    }
    let warnings = report.warnings();
    if !warnings.is_empty() {
        extra["warnings"] = json!(warnings);
    }
    extra.to_string()
}

impl SetSnapshot {
    fn run(&self, command: ComponentSetCommand, extra: Value) -> Result<ToolOutcome, ToolOutcome> {
        let (after, report) = preview_command(&self.doc, &command).map_err(invalid)?;
        let mut extra = extra;
        // An instance switch answers with the master it now shows.
        if let Some(node_id) = extra["nodeId"].as_str().map(str::to_string) {
            if let Some(jian_ops_schema::node::PenNode::Ref(reference)) =
                de_editor_core::component_set::find_node(&after, &node_id)
            {
                extra["ref"] = json!(reference.target);
                extra["variant"] = json!(reference.variant);
            }
        }
        Ok(ToolOutcome::OkJsonWithCommand(
            answer(&after, &report, extra),
            EditorCommand::ComponentSet(Box::new(command)),
        ))
    }

    fn command(
        &self,
        args: &BTreeMap<String, String>,
    ) -> Result<(ComponentSetCommand, Value), ToolOutcome> {
        let set_id = || required(args, &["set_id", "setId", "componentSetId"]).map(str::to_string);
        Ok(match self.tool {
            "create_component_set" => {
                let name = required(args, &["name"])?.to_string();
                let master_ids = id_list(required(args, &["master_ids", "masterIds"])?);
                let axes: Option<VariantAxes> = json_arg(
                    args,
                    &["axes"],
                    "an object of axis name to a list of values",
                )?;
                let values: Option<Vec<VariantValues>> = json_arg(
                    args,
                    &["values"],
                    "an array of {axis: value} objects, one per master",
                )?;
                let mut create = CreateSet {
                    name,
                    master_ids,
                    axes: axes.map(|axes| axes.0),
                    values,
                    set_id: arg(args, &["set_id", "setId"]).map(str::to_string),
                };
                let planned = create.plan(&self.doc).map_err(invalid)?;
                create.set_id = Some(planned.set_id.clone());
                (
                    ComponentSetCommand::Create(create),
                    json!({"setId": planned.set_id}),
                )
            }
            "add_variant" => {
                let values: VariantValues =
                    json_arg(args, &["values"], "an object of axis to value")?
                        .ok_or_else(|| invalid("values is required"))?;
                (
                    ComponentSetCommand::AddVariant {
                        set_id: set_id()?,
                        values,
                        from: arg(args, &["from", "from_id"]).map(str::to_string),
                        new_id: arg(args, &["id", "newId"]).map(str::to_string),
                    },
                    json!({}),
                )
            }
            "add_variant_axis" => (
                ComponentSetCommand::AddAxis {
                    set_id: set_id()?,
                    axis: required(args, &["axis", "name"])?.to_string(),
                    value: required(args, &["value", "default"])?.to_string(),
                },
                json!({}),
            ),
            "rename_variant_axis" => (
                ComponentSetCommand::RenameAxis {
                    set_id: set_id()?,
                    from: required(args, &["axis", "from"])?.to_string(),
                    to: required(args, &["newName", "new_name", "to"])?.to_string(),
                },
                json!({}),
            ),
            "rename_variant_value" => (
                ComponentSetCommand::RenameValue {
                    set_id: set_id()?,
                    axis: required(args, &["axis"])?.to_string(),
                    from: required(args, &["value", "from"])?.to_string(),
                    to: required(args, &["newName", "new_name", "to"])?.to_string(),
                },
                json!({}),
            ),
            "remove_variant_value" => (
                ComponentSetCommand::RemoveValue {
                    set_id: set_id()?,
                    axis: required(args, &["axis"])?.to_string(),
                    value: required(args, &["value"])?.to_string(),
                },
                json!({}),
            ),
            _ => {
                let node_id = required(args, &["nodeId", "node_id", "instanceId"])?.to_string();
                let variant: VariantValues =
                    json_arg(args, &["variant"], "an object of axis to value")?
                        .ok_or_else(|| invalid("variant is required"))?;
                (
                    ComponentSetCommand::SetInstanceVariant {
                        node_id: node_id.clone(),
                        variant,
                    },
                    json!({"nodeId": node_id}),
                )
            }
        })
    }
}

impl McpTool for SetSnapshot {
    fn name(&self) -> &str {
        self.tool
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.command(args)
            .and_then(|(command, extra)| self.run(command, extra))
            .unwrap_or_else(|error| error)
    }
}

/// The set tools' names, in registration order.
pub const SET_TOOL_NAMES: [&str; 7] = [
    "create_component_set",
    "add_variant",
    "add_variant_axis",
    "rename_variant_axis",
    "rename_variant_value",
    "remove_variant_value",
    "set_instance_variant",
];

/// The set tool `name` over a snapshot of `state`.
pub fn component_set_tool_snapshot(state: &EditorState, name: &'static str) -> SetSnapshot {
    SetSnapshot {
        doc: state.doc.clone(),
        tool: name,
    }
}

/// `list_components`' `sets` field (JSON), and the ids of every variant
/// master (listed under their set, not as components of their own).
pub fn sets_json(doc: &PenDocument) -> (Option<String>, HashSet<String>) {
    let sets = set_summaries(doc);
    let variants = sets
        .iter()
        .flat_map(|set| set.variants.iter().map(|variant| variant.id.clone()))
        .collect();
    let json = (!sets.is_empty())
        .then(|| Value::Array(sets.iter().map(|set| set.to_json()).collect()).to_string());
    (json, variants)
}

/// `get_component` on a set id: its axes, default and variants.
pub fn set_detail(doc: &PenDocument, id: &str) -> Option<BTreeMap<String, String>> {
    let summary = set_summary(doc, id)?;
    let json = summary.to_json();
    let mut out = BTreeMap::new();
    out.insert("name".into(), summary.name.clone());
    out.insert("kind".into(), "component_set".into());
    out.insert("axes".into(), json["axes"].to_string());
    out.insert("default".into(), json["default"].to_string());
    out.insert("variants".into(), json["variants"].to_string());
    Some(out)
}

/// `get_component` on a variant master: its set and values.
pub fn variant_detail(doc: &PenDocument, master_id: &str, out: &mut BTreeMap<String, String>) {
    let Some(set) = de_editor_core::component_set::set_containing(doc, master_id) else {
        return;
    };
    out.insert(
        "set".into(),
        de_editor_core::PenNodeExt::id_str(set).to_string(),
    );
    let values = set_summary(doc, de_editor_core::PenNodeExt::id_str(set))
        .and_then(|summary| summary.variants.into_iter().find(|v| v.id == master_id))
        .map(|variant| variant.values)
        .unwrap_or_default();
    out.insert("variantValues".into(), json!(values).to_string());
}

/// Instances about to be inserted that name a set or ask for a variant
/// point at the master they show; unknown axes or values are refused.
/// Returns one warning per instance showing the nearest variant.
pub fn normalize_inserted_variants(
    doc: &PenDocument,
    nodes: &mut [jian_ops_schema::node::PenNode],
) -> Result<Vec<String>, VariantError> {
    let fallbacks = de_editor_core::component_set::normalize_inserted(doc, nodes)?;
    let report = VariantReport {
        fallbacks,
        ..Default::default()
    };
    Ok(report.warnings())
}

/// Split `variant` off an update patch for the instance `node_id`.
/// `Ok(None)` when the patch has no `variant`.
pub fn take_variant_update(
    doc: &PenDocument,
    node_id: &str,
    patch: &mut Value,
) -> Result<Option<(ComponentSetCommand, Vec<String>)>, VariantError> {
    let Some(raw) = patch.as_object_mut().and_then(|obj| obj.remove("variant")) else {
        return Ok(None);
    };
    let variant: VariantValues = serde_json::from_value(raw).map_err(|_| {
        VariantError::InvalidName("variant must be an object of axis to value".into())
    })?;
    let command = ComponentSetCommand::SetInstanceVariant {
        node_id: node_id.to_string(),
        variant,
    };
    let (_, report) = preview_command(doc, &command)?;
    Ok(Some((command, report.warnings())))
}

/// `update_node` with `data.variant` on an instance: the switch is its own
/// command, ahead of whatever else `data` patches (handled by `rest`).
/// `None` when `data` has no `variant`.
pub(crate) fn update_node_variant(
    doc: Option<&PenDocument>,
    node_id: &de_editor_core::NodeId,
    args: &BTreeMap<String, String>,
    rest: impl FnOnce(&BTreeMap<String, String>) -> ToolOutcome,
) -> Option<ToolOutcome> {
    let mut data: Value = serde_json::from_str(args.get("data")?).ok()?;
    data.get("variant")?;
    let Some(doc) = doc else {
        return Some(invalid(
            "variant needs the document; use set_instance_variant",
        ));
    };
    let (command, warnings) = match take_variant_update(doc, node_id.as_str(), &mut data) {
        Ok(Some(update)) => update,
        Ok(None) => return None,
        Err(error) => return Some(invalid(error)),
    };
    let switch = EditorCommand::ComponentSet(Box::new(command));
    let mut out = BTreeMap::new();
    out.insert("wrote".into(), "true".into());
    if !warnings.is_empty() {
        out.insert("warnings".into(), json!(warnings).to_string());
    }
    if data.as_object().is_none_or(serde_json::Map::is_empty) {
        return Some(ToolOutcome::OkWithCommand(out, switch));
    }
    let mut forwarded = args.clone();
    forwarded.insert("data".into(), data.to_string());
    Some(match rest(&forwarded) {
        ToolOutcome::OkWithCommand(mut more, command) => {
            more.extend(out);
            let mut commands = vec![switch];
            match command {
                EditorCommand::Batch { commands: inner } => commands.extend(inner),
                other => commands.push(other),
            }
            ToolOutcome::OkWithCommand(more, EditorCommand::Batch { commands })
        }
        other => other,
    })
}
