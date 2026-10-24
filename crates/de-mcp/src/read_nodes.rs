//! TS-compatible `read_nodes` tool for codegen and inspection flows, with
//! pen.dev's `Get` options (#169): `resolveVariables`, `resolveInstances`
//! and `includePathGeometry`.

use std::collections::BTreeMap;

use de_editor_core::instance_path::{expand_instances_with_paths, INSTANCE_PATH_SEPARATOR};
use de_editor_core::variables_resolve::resolve_roots_for_canvas;
use de_editor_core::walkers::find_node;
use de_editor_core::{EditorState, NodeId};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

type ToolCallError = (ToolErrorCode, String);

pub struct ReadNodes {
    /// The whole document: `resolveInstances` looks components up on any
    /// page, and `resolveVariables` reads its variable table.
    doc: PenDocument,
    active_page_index: usize,
    active_theme: BTreeMap<String, String>,
}

#[derive(Clone)]
pub(crate) struct PageNodes {
    pub(crate) id: String,
    pub(crate) roots: Vec<PenNode>,
}

/// pen.dev `Get` options (#169) next to the original variables flag.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ReadOptions {
    include_variables: bool,
    resolve_variables: bool,
    resolve_instances: bool,
    include_path_geometry: bool,
}

impl ReadOptions {
    fn parse(args: &BTreeMap<String, String>) -> Result<Self, ToolCallError> {
        Ok(Self {
            include_variables: parse_flag(
                args,
                &["includeVariables", "include_variables", "vars"],
            )?,
            resolve_variables: parse_flag(args, &["resolveVariables", "resolve_variables"])?,
            resolve_instances: parse_flag(args, &["resolveInstances", "resolve_instances"])?,
            include_path_geometry: parse_flag(
                args,
                &["includePathGeometry", "include_path_geometry"],
            )?,
        })
    }
}

impl McpTool for ReadNodes {
    fn name(&self) -> &str {
        "read_nodes"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        match self.read(args) {
            Ok(json) => ToolOutcome::OkJson(json.to_string()),
            Err((code, msg)) => ToolOutcome::Err(code, msg),
        }
    }
}

pub fn read_nodes_snapshot(state: &EditorState) -> ReadNodes {
    ReadNodes {
        doc: state.doc.clone(),
        active_page_index: state.ui.active_page_index,
        active_theme: state.ui.variables.active_theme.clone(),
    }
}

impl ReadNodes {
    fn read(&self, args: &BTreeMap<String, String>) -> Result<Value, ToolCallError> {
        let roots = self.page_roots(args)?;
        let depth = parse_depth(args)?;
        let options = ReadOptions::parse(args)?;
        let node_ids = parse_node_ids(args)?;

        let mut selected: Vec<PenNode> = match node_ids {
            Some(ids) if !ids.is_empty() => {
                ids.iter().filter_map(|id| self.find(roots, id)).collect()
            }
            _ => roots.to_vec(),
        };
        if options.resolve_instances {
            selected = expand_instances_with_paths(&selected, &self.doc);
        }
        if options.resolve_variables {
            resolve_roots_for_canvas(&mut selected, &self.doc, &self.active_theme);
        }
        let nodes: Vec<Value> = selected
            .iter()
            .map(|node| {
                let mut value = node_snapshot_value(node, depth);
                if !options.include_path_geometry {
                    strip_path_geometry(&mut value);
                }
                value
            })
            .collect();
        // Match TS read-nodes EXACTLY: { nodes, variables?, themes? } with
        // NATIVE JSON values and NO Rust-only `count` key (TS ReadNodesResult
        // has none; cf. batch_get which also dropped `count`).
        let mut out = serde_json::Map::new();
        out.insert("nodes".into(), Value::Array(nodes));
        if options.include_variables {
            // TS handleReadNodes: variables = doc.variables ?? {} (absent →
            // empty OBJECT); themes = doc.themes ?? [] (absent → empty
            // ARRAY, NOT an empty object).
            let variables = match self.doc.variables.as_ref() {
                Some(v) => serde_json::to_value(v).unwrap_or_else(|_| json!({})),
                None => json!({}),
            };
            let themes = match self.doc.themes.as_ref() {
                Some(t) => serde_json::to_value(t).unwrap_or_else(|_| json!([])),
                None => json!([]),
            };
            out.insert("variables".into(), variables);
            out.insert("themes".into(), themes);
        }
        Ok(Value::Object(out))
    }

    /// The requested page's top-level nodes (`pageId`, else the active page).
    fn page_roots(&self, args: &BTreeMap<String, String>) -> Result<&[PenNode], ToolCallError> {
        let target = arg_alias(args, &["pageId", "page_id", "page"]);
        let found = match self.doc.pages.as_ref() {
            Some(pages) if !pages.is_empty() => {
                let active = self.active_page_index.min(pages.len() - 1);
                let page = match target {
                    Some(id) => pages.iter().find(|page| page.id == id),
                    None => pages.get(active),
                };
                page.map(|page| page.children.as_slice())
            }
            _ => match target {
                None | Some("0") => Some(self.doc.children.as_slice()),
                Some(_) => None,
            },
        };
        found.ok_or_else(|| {
            (
                ToolErrorCode::ToolFailed,
                format!("page not found: {}", target.unwrap_or("0")),
            )
        })
    }

    /// An authored node by id, or a node inside a component instance by its
    /// `instance/child` path (expanded on demand, as pen.dev's `Get` reads
    /// instance paths).
    fn find(&self, roots: &[PenNode], id: &str) -> Option<PenNode> {
        let node_id = NodeId::new_opt(id)?;
        if let Some(node) = find_node(roots, &node_id) {
            return Some(node.clone());
        }
        let (instance, _) = id.split_once(INSTANCE_PATH_SEPARATOR)?;
        let instance = find_node(roots, &NodeId::new_opt(instance)?)?;
        let expanded = expand_instances_with_paths(std::slice::from_ref(instance), &self.doc);
        find_node(&expanded, &node_id).cloned()
    }
}

/// Drop path outlines (`d`, `anchors`) from every `path` node of a
/// serialized subtree: pen.dev's `Get` leaves path geometry out unless
/// `includePathGeometry` asks for it, since one icon can carry kilobytes of
/// outline.
fn strip_path_geometry(value: &mut Value) {
    let Value::Object(map) = value else {
        return;
    };
    if map.get("type").and_then(Value::as_str) == Some("path") {
        map.remove("d");
        map.remove("anchors");
    }
    if let Some(Value::Array(children)) = map.get_mut("children") {
        for child in children {
            strip_path_geometry(child);
        }
    }
}

pub(crate) fn page_nodes_snapshots(state: &EditorState) -> (Vec<PageNodes>, String) {
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => {
            let active_idx = state
                .ui
                .active_page_index
                .min(pages.len().saturating_sub(1));
            let active_page_id = pages[active_idx].id.clone();
            let pages = pages
                .iter()
                .map(|page| PageNodes {
                    id: page.id.clone(),
                    roots: page.children.clone(),
                })
                .collect();
            (pages, active_page_id)
        }
        _ => (
            vec![PageNodes {
                id: "0".into(),
                roots: state.doc.children.clone(),
            }],
            "0".into(),
        ),
    }
}

fn parse_depth(args: &BTreeMap<String, String>) -> Result<i32, ToolCallError> {
    match args.get("depth") {
        None => Ok(-1),
        Some(raw) => raw.parse::<i32>().map_err(|_| {
            (
                ToolErrorCode::InvalidArgument,
                format!("depth must be an i32, got {raw:?}"),
            )
        }),
    }
}

/// A boolean flag under any of `keys`; absent is `false`. Errors name the
/// first (canonical) key.
fn parse_flag(args: &BTreeMap<String, String>, keys: &[&str]) -> Result<bool, ToolCallError> {
    match arg_alias(args, keys) {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(raw) => Err((
            ToolErrorCode::InvalidArgument,
            format!("{} must be true or false, got {raw:?}", keys[0]),
        )),
    }
}

fn parse_node_ids(args: &BTreeMap<String, String>) -> Result<Option<Vec<String>>, ToolCallError> {
    let Some(raw) = arg_alias(args, &["nodeIds", "node_ids", "ids"]) else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Some(Vec::new()));
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(trimmed)
            .map(Some)
            .map_err(|e| {
                (
                    ToolErrorCode::InvalidArgument,
                    format!("nodeIds must be a JSON string array: {e}"),
                )
            });
    }
    Ok(Some(
        trimmed
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .collect(),
    ))
}

pub(crate) fn node_snapshot_value(node: &PenNode, depth: i32) -> Value {
    let mut value = serde_json::to_value(node).unwrap_or(Value::Null);
    if depth != -1 {
        truncate_children(&mut value, depth);
    }
    value
}

fn truncate_children(value: &mut Value, depth: i32) {
    let Value::Object(map) = value else {
        return;
    };
    let Some(children) = map.get_mut("children") else {
        return;
    };
    let Value::Array(items) = children else {
        return;
    };
    if items.is_empty() {
        return;
    }
    if depth <= 0 {
        *children = Value::String("...".into());
    } else {
        for child in items {
            truncate_children(child, depth - 1);
        }
    }
}

fn arg_alias<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| args.get(*key).map(String::as_str))
}
