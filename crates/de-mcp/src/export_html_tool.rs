//! `export_html`, pen.dev's HTML export (#392), on `codegen_export`'s HTML
//! targets.
//!
//! pen.dev's tool writes the file to `outputPath` and answers with its path.
//! Here `outputPath` is optional: with it, the HTML is written there and its
//! images into an `assets/` folder beside it, as `codegen_export` names
//! them; without it, the HTML comes back inline with its images embedded,
//! since there is no folder to write them to.

use std::collections::BTreeMap;
use std::path::Path;

use de_codegen::{HtmlExportOptions, HtmlTarget};
use de_editor_core::EditorState;
use serde_json::json;

use super::codegen_export_tool::{codegen_export_snapshot, flag, CodegenExport};
use super::{McpTool, ToolErrorCode, ToolOutcome};

/// Where written exports put their images, relative to the HTML file.
const ASSET_DIR: &str = "assets";

pub struct ExportHtml {
    codegen: CodegenExport,
}

pub fn export_html_snapshot(state: &EditorState) -> ExportHtml {
    ExportHtml {
        codegen: codegen_export_snapshot(state),
    }
}

impl McpTool for ExportHtml {
    fn name(&self) -> &str {
        "export_html"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let format = args
            .get("format")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("html-tailwind");
        let target = match format {
            "html-tailwind" => HtmlTarget::Tailwind,
            "html-css" | "html" => HtmlTarget::Css,
            other => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("format must be html-tailwind or html-css, got {other:?}"),
                )
            }
        };
        let output = args
            .get("outputPath")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(Path::new);
        if let Some(path) = output.filter(|path| !path.is_absolute()) {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("outputPath must be absolute: {}", path.display()),
            );
        }
        let options = match options(args, output.is_some()) {
            Ok(options) => options,
            Err(message) => return ToolOutcome::Err(ToolErrorCode::InvalidArgument, message),
        };
        let (export, node_count) = match self.codegen.html(args, target, &options) {
            Ok(export) => export,
            Err(message) => return ToolOutcome::Err(ToolErrorCode::ToolFailed, message),
        };
        let Some(path) = output else {
            return ToolOutcome::OkJson(
                json!({ "format": format, "nodeCount": node_count, "html": export.html })
                    .to_string(),
            );
        };
        match write_export(path, &export) {
            Ok(()) => ToolOutcome::OkJson(
                json!({
                    "filePath": de_util::display_path::display_path(path),
                    "format": format,
                    "nodeCount": node_count,
                    "assets": export.assets.iter().map(|asset| &asset.path).collect::<Vec<_>>(),
                })
                .to_string(),
            ),
            Err(message) => ToolOutcome::Err(ToolErrorCode::ToolFailed, message),
        }
    }
}

/// pen.dev's switches as `HtmlExportOptions`. Its layer names and ids are
/// one `layerData` switch here, so either one turns both on.
fn options(args: &BTreeMap<String, String>, to_file: bool) -> Result<HtmlExportOptions, String> {
    let names = flag(args, "includeLayerNames", true)?;
    let ids = flag(args, "includeLayerIds", false)?;
    Ok(HtmlExportOptions {
        scaffold: flag(args, "includeHtmlScaffold", true)?,
        layer_data: names || ids,
        asset_dir: to_file.then(|| ASSET_DIR.into()),
        title: None,
    })
}

fn write_export(path: &Path, export: &de_codegen::HtmlExport) -> Result<(), String> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    std::fs::write(path, &export.html).map_err(|e| format!("write {}: {e}", path.display()))?;
    for asset in &export.assets {
        let file = dir.join(&asset.path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        std::fs::write(&file, &asset.bytes)
            .map_err(|e| format!("write {}: {e}", file.display()))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "export_html_tool_tests.rs"]
mod tests;
