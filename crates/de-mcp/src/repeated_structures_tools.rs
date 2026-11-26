//! Repeated copies → component instances over MCP (#369).
//!
//! - `find_repeated_structures {pageId?, minNodes?}` (read only): groups of
//!   frames sharing one structure, with their ids, size and the saving a
//!   conversion would bring.
//! - `convert_to_instances {nodeIds | groupId, masterFrom?, placement?,
//!   pageId?, dryRun?}`: plans on the snapshot (every check runs before the
//!   host mutates anything) and hands the host one
//!   `EditorCommand::ConvertToInstances` (one undo step). Members whose
//!   structure differs are reported in `skipped`, never merged; a call
//!   that would convert nothing is an error, not a silent no-op.
//!
//! `run` returns a finished `ToolOutcome::Err`, so the large `Err` is
//! deliberate (as in `instance_tools.rs`).
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;

use de_editor_core::id_allocator::SequentialIdAllocator;
use de_editor_core::repeated_structures::{
    find_repeated_structures, group_members, plan_conversion, ConversionPlan, ConvertRequest,
    MasterPlacement, RepeatedGroup, DEFAULT_MIN_NODES,
};
use de_editor_core::{ComponentLibrary, EditorCommand, EditorState, NodeId};
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

/// Rough bytes per token of compact document JSON.
const BYTES_PER_TOKEN: usize = 4;

/// What both tools read.
pub struct RepeatedStructuresSnapshot {
    doc: PenDocument,
    components: ComponentLibrary,
}

impl RepeatedStructuresSnapshot {
    fn new(state: &EditorState) -> Self {
        Self {
            doc: state.doc.clone(),
            components: state.components.clone(),
        }
    }

    /// `pageId` (a page id or 0-based index) as a page index.
    fn page(&self, args: &BTreeMap<String, String>) -> Result<Option<usize>, ToolOutcome> {
        let Some(raw) = arg(args, "pageId") else {
            return Ok(None);
        };
        let pages = self.doc.pages.as_deref().unwrap_or_default();
        pages
            .iter()
            .position(|page| page.id == raw)
            .or_else(|| {
                raw.parse::<usize>()
                    .ok()
                    .filter(|index| *index < pages.len().max(1))
            })
            .map(Some)
            .ok_or_else(|| invalid(format!("pageId not found: {raw}")))
    }

    fn page_id(&self, index: Option<usize>) -> Value {
        index
            .and_then(|index| self.doc.pages.as_ref()?.get(index))
            .map_or(Value::Null, |page| Value::from(page.id.as_str()))
    }

    fn plan(&self, request: &ConvertRequest) -> Result<ConversionPlan, String> {
        let mut allocator =
            SequentialIdAllocator::for_document(&self.doc, 1).map_err(|e| e.to_string())?;
        plan_conversion(&self.doc, &self.components, request, &mut allocator)
            .map_err(|error| error.to_string())
    }
}

fn arg<'a>(args: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    args.get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

fn invalid(message: impl ToString) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message.to_string())
}

fn saving(plan: &ConversionPlan) -> usize {
    plan.bytes_before.saturating_sub(plan.bytes_after)
}

/// `find_repeated_structures`.
pub struct FindRepeatedStructures(RepeatedStructuresSnapshot);

pub fn find_repeated_structures_snapshot(state: &EditorState) -> FindRepeatedStructures {
    FindRepeatedStructures(RepeatedStructuresSnapshot::new(state))
}

impl FindRepeatedStructures {
    fn group_json(&self, group: &RepeatedGroup) -> Value {
        let snapshot = &self.0;
        let request = ConvertRequest {
            node_ids: group.node_ids.clone(),
            ..Default::default()
        };
        let mut entry = json!({
            "groupId": group.group_id,
            "name": group.name,
            "members": group.node_ids.len(),
            "nodesPerMember": group.nodes_per_member,
            "nodeIds": group.node_ids.iter().map(NodeId::as_str).collect::<Vec<_>>(),
            "pageIds": group.pages.iter().map(|page| snapshot.page_id(*page)).collect::<Vec<_>>(),
        });
        match snapshot.plan(&request) {
            Ok(plan) => {
                entry["convertible"] = json!(plan.converted.len());
                entry["skipped"] = json!(plan.skipped.len());
                entry["estimatedBytesSaved"] = json!(saving(&plan));
                entry["estimatedTokenSaving"] = json!(saving(&plan) / BYTES_PER_TOKEN);
            }
            Err(reason) => {
                entry["convertible"] = json!(0);
                entry["note"] = json!(reason);
            }
        }
        entry
    }
}

impl McpTool for FindRepeatedStructures {
    fn name(&self) -> &str {
        "find_repeated_structures"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let page = match self.0.page(args) {
            Ok(page) => page,
            Err(error) => return error,
        };
        let min_nodes = match arg(args, "minNodes") {
            None => DEFAULT_MIN_NODES,
            Some(raw) => match raw.parse::<f64>() {
                Ok(value) if value.is_finite() && value >= 1.0 && value.fract() == 0.0 => {
                    value as usize
                }
                _ => return invalid(format!("minNodes must be a whole number >= 1, got {raw}")),
            },
        };
        let groups = find_repeated_structures(&self.0.doc, &self.0.components, page, min_nodes);
        let groups: Vec<Value> = groups.iter().map(|group| self.group_json(group)).collect();
        ToolOutcome::OkJson(
            json!({
                "count": groups.len(),
                "minNodes": min_nodes,
                "pageId": self.0.page_id(page),
                "groups": groups,
            })
            .to_string(),
        )
    }
}

/// `convert_to_instances`.
pub struct ConvertToInstances(RepeatedStructuresSnapshot);

pub fn convert_to_instances_snapshot(state: &EditorState) -> ConvertToInstances {
    ConvertToInstances(RepeatedStructuresSnapshot::new(state))
}

/// A JSON array of ids (or a comma list, as other tools accept).
fn id_list(raw: &str) -> Result<Vec<NodeId>, ToolOutcome> {
    let ids: Vec<String> = match serde_json::from_str::<Value>(raw) {
        Ok(Value::Array(items)) => items
            .into_iter()
            .map(|item| match item {
                Value::String(id) => Ok(id),
                other => Err(invalid(format!("nodeIds must hold strings, got {other}"))),
            })
            .collect::<Result<_, _>>()?,
        _ if raw.starts_with('[') => return Err(invalid("nodeIds must be a JSON array of ids")),
        _ => raw.split(',').map(|id| id.trim().to_string()).collect(),
    };
    Ok(ids
        .into_iter()
        .filter(|id| !id.trim().is_empty())
        .map(|id| NodeId::new(id.trim()))
        .collect())
}

impl ConvertToInstances {
    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolOutcome> {
        let snapshot = &self.0;
        let dry_run = match arg(args, "dryRun") {
            None | Some("false") => false,
            Some("true") => true,
            Some(other) => {
                return Err(invalid(format!(
                    "dryRun must be true or false, got {other}"
                )))
            }
        };
        let placement = match arg(args, "placement") {
            None | Some("components") => MasterPlacement::ComponentsPage,
            Some("inPlace") => MasterPlacement::InPlace,
            Some(other) => {
                return Err(invalid(format!(
                    "placement must be components or inPlace, got {other}"
                )))
            }
        };
        let page = snapshot.page(args)?;
        let group_id = arg(args, "groupId");
        let node_ids = match (arg(args, "nodeIds"), group_id) {
            (Some(_), Some(_)) => return Err(invalid("pass nodeIds or groupId, not both")),
            (None, None) => {
                return Err(ToolOutcome::Err(
                    ToolErrorCode::MissingArgument,
                    "nodeIds or groupId is required".into(),
                ))
            }
            (Some(raw), None) => id_list(raw)?,
            (None, Some(group_id)) => group_members(
                &snapshot.doc,
                &snapshot.components,
                page,
                group_id,
            )
            .ok_or_else(|| {
                invalid(format!(
                    "groupId not found: {group_id} (call find_repeated_structures for current ids)"
                ))
            })?,
        };
        let request = ConvertRequest {
            node_ids,
            master_from: arg(args, "masterFrom").map(NodeId::new),
            placement,
        };
        let plan = snapshot.plan(&request).map_err(invalid)?;
        let result = json!({
            "dryRun": dry_run,
            "groupId": group_id,
            "masterId": plan.master_id.as_str(),
            "masterFrom": plan.master_from.as_str(),
            "masterName": plan.master_name,
            "placement": plan.placement.as_str(),
            "converted": plan.converted.iter().map(|member| json!({
                "id": member.id.as_str(),
                "overrides": member.overrides,
            })).collect::<Vec<_>>(),
            "skipped": plan.skipped.iter().map(|member| json!({
                "id": member.id.as_str(),
                "reason": member.reason,
            })).collect::<Vec<_>>(),
            "bytesBefore": plan.bytes_before,
            "bytesAfter": plan.bytes_after,
            "estimatedTokenSaving": saving(&plan) / BYTES_PER_TOKEN,
        })
        .to_string();
        Ok(if dry_run {
            ToolOutcome::OkJson(result)
        } else {
            ToolOutcome::OkJsonWithCommand(
                result,
                EditorCommand::ConvertToInstances(Box::new(request)),
            )
        })
    }
}

impl McpTool for ConvertToInstances {
    fn name(&self) -> &str {
        "convert_to_instances"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args).unwrap_or_else(|error| error)
    }
}

#[cfg(test)]
#[path = "repeated_structures_tools_tests.rs"]
mod tests;
