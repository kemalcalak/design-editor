//! MCP `rename_layers` (#383): give layers semantic names instead of
//! `Frame 12` / `Rectangle 3`, as one undo step. Names only — ids never
//! change.
//!
//! `mode: "rules"` (default) uses the offline rule-based namer, in the
//! document's language. `mode: "ai"` makes the calling agent the namer:
//! without `names` it returns a brief of every layer to name (kind, text,
//! children, path, the rules' suggestion) and changes nothing; called
//! again with `names: {"<id>": "<name>"}` it applies them. `dryRun: true`
//! reports the renames without applying them. Layers are on the active
//! page: `nodeIds` and their descendants, else the selection, else the
//! whole page.

use std::collections::{BTreeMap, HashSet};

use de_design_lint::{document_locale, is_default_layer_name, propose_layer_names};
use de_editor_core::rename_layers::ai::{language_label, rename_layer_briefs};
use de_editor_core::rename_layers::rename_layers_command;
use de_editor_core::{EditorState, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct RenameLayers {
    /// Top-level layers of the active page.
    roots: Vec<PenNode>,
    /// Every node id on the active page with its current name.
    names: BTreeMap<String, Option<String>>,
    selection: Vec<String>,
}

pub fn rename_layers_snapshot(state: &EditorState) -> RenameLayers {
    fn walk(node: &PenNode, out: &mut BTreeMap<String, Option<String>>) {
        out.insert(node.id_str().to_string(), node.base().name.clone());
        for child in node.children().map_or(&[][..], Vec::as_slice) {
            walk(child, out);
        }
    }
    let roots = state.active_children().to_vec();
    let mut names = BTreeMap::new();
    for root in &roots {
        walk(root, &mut names);
    }
    RenameLayers {
        roots,
        names,
        selection: state
            .selection
            .set
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
    }
}

type ToolError = (ToolErrorCode, String);

fn parse_bool(args: &BTreeMap<String, String>, key: &str) -> Result<bool, ToolError> {
    match args.get(key).map(String::as_str) {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(other) => Err((
            ToolErrorCode::InvalidArgument,
            format!("{key} must be true or false, got {other}"),
        )),
    }
}

fn parse_node_ids(args: &BTreeMap<String, String>) -> Result<Vec<String>, ToolError> {
    let Some(raw) = args.get("nodeIds").map(|raw| raw.trim()) else {
        return Ok(Vec::new());
    };
    if raw.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(raw).map_err(|e| {
            (
                ToolErrorCode::InvalidArgument,
                format!("nodeIds must be a JSON string array: {e}"),
            )
        });
    }
    Ok(raw
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect())
}

fn parse_names(
    args: &BTreeMap<String, String>,
) -> Result<Option<BTreeMap<String, String>>, ToolError> {
    let Some(raw) = args.get("names") else {
        return Ok(None);
    };
    serde_json::from_str::<BTreeMap<String, String>>(raw)
        .map(Some)
        .map_err(|e| {
            (
                ToolErrorCode::InvalidArgument,
                format!("names must be an object mapping node ids to names: {e}"),
            )
        })
}

fn rename_json(id: &str, current: Option<&str>, proposed: &str) -> Value {
    json!({"id": id, "current": current, "proposed": proposed})
}

impl RenameLayers {
    fn check_ids(&self, ids: &[String]) -> Result<(), ToolError> {
        match ids.iter().find(|id| !self.names.contains_key(id.as_str())) {
            Some(missing) => Err((
                ToolErrorCode::InvalidArgument,
                format!(
                    "nodeId not found on the active page: {missing} (switch pages with \
                     set_active_page)"
                ),
            )),
            None => Ok(()),
        }
    }

    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolError> {
        let dry_run = parse_bool(args, "dryRun")?;
        let mode = args.get("mode").map(String::as_str).unwrap_or("rules");
        if !matches!(mode, "rules" | "ai") {
            return Err((
                ToolErrorCode::InvalidArgument,
                format!("mode must be \"rules\" or \"ai\", got {mode}"),
            ));
        }
        let mut ids = parse_node_ids(args)?;
        self.check_ids(&ids)?;
        if ids.is_empty() {
            ids = self.selection.clone();
        }
        let targets: HashSet<String> = ids.into_iter().collect();
        let locale = document_locale(&self.roots);
        let proposals = propose_layer_names(
            &self.roots,
            (!targets.is_empty()).then_some(&targets),
            locale,
        );
        let names = parse_names(args)?;
        if mode == "rules" && names.is_some() {
            return Err((
                ToolErrorCode::InvalidArgument,
                "names is only read with mode \"ai\"; mode \"rules\" proposes its own names"
                    .to_string(),
            ));
        }
        if mode == "ai" && names.is_none() {
            let briefs = rename_layer_briefs(&self.roots, &proposals);
            return Ok(ToolOutcome::OkJson(
                json!({
                    "mode": "ai",
                    "dryRun": true,
                    "language": language_label(locale),
                    "count": briefs.len(),
                    "total": proposals.len(),
                    // A brief holds at most `MAX_BRIEF_LAYERS` layers; name
                    // the rest with a narrower nodeIds call.
                    "truncated": proposals.len() > briefs.len(),
                    "layers": briefs,
                    "next": "Propose a short semantic name for each layer in the document's \
                             language (role and section, e.g. \"Primary button\", \"Hero / \
                             Title\"; never Frame / Group / Rectangle or numbers), then call \
                             rename_layers again with mode \"ai\" and names {\"<id>\": \"<name>\"}.",
                })
                .to_string(),
            ));
        }
        let mut skipped = Vec::new();
        let renames: Vec<(String, String)> = match names {
            Some(names) => {
                let given: Vec<String> = names.keys().cloned().collect();
                self.check_ids(&given)?;
                names
                    .into_iter()
                    .filter_map(|(id, name)| {
                        let name = name.trim().to_string();
                        if name.is_empty() || is_default_layer_name(&name) {
                            skipped.push(json!({"id": id, "name": name}));
                            None
                        } else {
                            Some((id, name))
                        }
                    })
                    .collect()
            }
            None => proposals
                .iter()
                .map(|p| (p.node_id.clone(), p.proposed.clone()))
                .collect(),
        };
        let renames: Vec<(String, String)> = renames
            .into_iter()
            .filter(|(id, name)| {
                self.names.get(id).and_then(|n| n.as_deref()).map(str::trim) != Some(name.trim())
            })
            .collect();
        let result = json!({
            "mode": mode,
            "dryRun": dry_run,
            "language": language_label(locale),
            "count": renames.len(),
            "renamed": renames
                .iter()
                .map(|(id, name)| rename_json(
                    id,
                    self.names.get(id).and_then(|n| n.as_deref()),
                    name
                ))
                .collect::<Vec<_>>(),
            "skipped": skipped,
        })
        .to_string();
        let command = rename_layers_command(
            &renames
                .into_iter()
                .map(|(id, name)| (NodeId::new(id), name))
                .collect::<Vec<_>>(),
        );
        Ok(match command {
            Some(command) if !dry_run => ToolOutcome::OkJsonWithCommand(result, command),
            _ => ToolOutcome::OkJson(result),
        })
    }
}

impl McpTool for RenameLayers {
    fn name(&self) -> &str {
        "rename_layers"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        match self.run(args) {
            Ok(outcome) => outcome,
            Err((code, message)) => ToolOutcome::Err(code, message),
        }
    }
}

#[cfg(test)]
#[path = "rename_layers_tool_tests.rs"]
mod tests;
