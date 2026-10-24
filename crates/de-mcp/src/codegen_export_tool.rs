//! Deterministic source export for the stable editor framework catalog.

use std::collections::BTreeMap;

use base64::Engine as _;
use de_codegen::{
    export_html, Codegen, Compose, CssVariables, Flutter, HtmlExportOptions, HtmlTarget, React,
    ReactNative, Svelte, SwiftUi, Vue,
};
use de_editor_core::codegen::Framework;
use de_editor_core::walkers::find_node;
use de_editor_core::{EditorState, NodeId};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{json, Map, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

/// Wire token of the variable-table-only target.
const CSS_VARIABLES: &str = "css-variables";
/// Target used when the caller names none.
const DEFAULT_TARGET: &str = "react";
/// Where the HTML targets write extracted images, relative to the HTML.
const HTML_ASSET_DIR: &str = "assets";

pub struct CodegenExport {
    doc: PenDocument,
    active_page: usize,
    selection: Vec<String>,
}

/// Every wire token `codegen_export` accepts, in catalog order.
pub fn codegen_export_targets() -> Vec<&'static str> {
    Framework::ALL
        .iter()
        .map(|framework| framework.as_wire())
        .chain(std::iter::once(CSS_VARIABLES))
        .collect()
}

pub fn codegen_export_snapshot(state: &EditorState) -> CodegenExport {
    CodegenExport {
        doc: state.doc.clone(),
        active_page: state.ui.active_page_index,
        selection: state
            .selection
            .set
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
    }
}

impl McpTool for CodegenExport {
    fn name(&self) -> &str {
        "codegen_export"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let target = args
            .get("framework")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_TARGET);
        let framework = Framework::from_wire(target);
        if framework.is_none() && target != CSS_VARIABLES {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!(
                    "unknown framework {target:?}; expected one of: {}",
                    codegen_export_targets().join(", ")
                ),
            );
        }
        let roots = match self.target_nodes(args) {
            Ok(roots) => roots,
            Err(message) => return ToolOutcome::Err(ToolErrorCode::ToolFailed, message),
        };
        let roots: Vec<PenNode> = roots.iter().map(|node| (*node).clone()).collect();
        let mut scoped = self.doc.clone();
        scoped.pages = None;
        // Instances resolve against the whole document: their components may
        // live on another page than the exported nodes.
        scoped.children =
            de_editor_core::ref_resolve::resolve_refs_for_canvas_roots(&roots, &self.doc);

        let mut files = Map::new();
        let mut assets = Vec::new();
        let code = match framework {
            Some(Framework::React) => React.generate(&scoped),
            Some(Framework::Vue) => Vue.generate(&scoped),
            Some(Framework::Svelte) => Svelte.generate(&scoped),
            Some(Framework::Html | Framework::HtmlTailwind) => {
                let target = if framework == Some(Framework::Html) {
                    HtmlTarget::Css
                } else {
                    HtmlTarget::Tailwind
                };
                let options = HtmlExportOptions {
                    scaffold: flag(args, "scaffold", true),
                    layer_data: flag(args, "layerData", false),
                    asset_dir: flag(args, "extractAssets", true).then(|| HTML_ASSET_DIR.into()),
                    title: None,
                };
                let export = export_html(&scoped, target, &options);
                assets = export.assets;
                export.html
            }
            Some(Framework::Flutter) => Flutter.generate(&scoped),
            Some(Framework::SwiftUi) => SwiftUi.generate(&scoped),
            Some(Framework::Compose) => Compose.generate(&scoped),
            Some(Framework::ReactNative) => ReactNative.generate(&scoped),
            None => CssVariables.generate(&scoped),
        };
        files.insert(format!("component.{}", extension(framework)), json!(code));
        let mut out = json!({
            "framework": target,
            "nodeCount": roots.len(),
            "code": code,
            "files": Value::Object(files),
        });
        if !assets.is_empty() {
            out["assets"] = Value::Array(
                assets
                    .iter()
                    .map(|asset| {
                        json!({
                            "path": asset.path,
                            "mimeType": asset.mime_type,
                            "base64": base64::engine::general_purpose::STANDARD.encode(&asset.bytes),
                        })
                    })
                    .collect(),
            );
        }
        ToolOutcome::OkJson(out.to_string())
    }
}

impl CodegenExport {
    fn pages(&self) -> Vec<(&str, &[PenNode])> {
        match self.doc.pages.as_deref() {
            Some(pages) if !pages.is_empty() => pages
                .iter()
                .map(|page| (page.id.as_str(), page.children.as_slice()))
                .collect(),
            _ => vec![("0", self.doc.children.as_slice())],
        }
    }

    fn find_anywhere(&self, id: &str) -> Option<&PenNode> {
        let id = NodeId::new_opt(id)?;
        self.pages()
            .into_iter()
            .find_map(|(_, roots)| find_node(roots, &id))
    }

    fn target_nodes(&self, args: &BTreeMap<String, String>) -> Result<Vec<&PenNode>, String> {
        if let Some(raw) = args.get("nodeIds").filter(|raw| !raw.trim().is_empty()) {
            let ids = parse_ids(raw)?;
            let mut nodes = Vec::with_capacity(ids.len());
            for id in &ids {
                nodes.push(
                    self.find_anywhere(id)
                        .ok_or_else(|| format!("node not found: {id}"))?,
                );
            }
            return Ok(nodes);
        }
        let pages = self.pages();
        if let Some(page_id) = args.get("pageId").filter(|p| !p.is_empty()) {
            let (_, roots) = pages
                .iter()
                .find(|(id, _)| id == page_id)
                .ok_or_else(|| format!("page not found: {page_id}"))?;
            return non_empty(roots.iter().collect());
        }
        let selected: Vec<&PenNode> = self
            .selection
            .iter()
            .filter_map(|id| self.find_anywhere(id))
            .collect();
        if !selected.is_empty() {
            return Ok(selected);
        }
        let index = self.active_page.min(pages.len().saturating_sub(1));
        non_empty(pages[index].1.iter().collect())
    }
}

/// A boolean argument (`"true"` / `"false"`), `default` when absent.
fn flag(args: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    match args.get(key).map(|v| v.trim().to_ascii_lowercase()) {
        Some(v) if v == "true" || v == "1" => true,
        Some(v) if v == "false" || v == "0" => false,
        _ => default,
    }
}

fn non_empty(nodes: Vec<&PenNode>) -> Result<Vec<&PenNode>, String> {
    if nodes.is_empty() {
        Err("nothing to export: the target page is empty".into())
    } else {
        Ok(nodes)
    }
}

/// `nodeIds` as a JSON string array or a comma-separated list.
fn parse_ids(raw: &str) -> Result<Vec<String>, String> {
    let raw = raw.trim();
    if raw.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(raw)
            .map_err(|e| format!("nodeIds must be a JSON string array: {e}"));
    }
    Ok(raw
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect())
}

fn extension(framework: Option<Framework>) -> &'static str {
    match framework {
        Some(Framework::React | Framework::ReactNative) => "tsx",
        Some(Framework::Vue) => "vue",
        Some(Framework::Svelte) => "svelte",
        Some(Framework::Html | Framework::HtmlTailwind) => "html",
        Some(Framework::Flutter) => "dart",
        Some(Framework::SwiftUi) => "swift",
        Some(Framework::Compose) => "kt",
        None => "css",
    }
}
