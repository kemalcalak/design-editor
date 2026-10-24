//! MCP tool `execute` (#168): the script runner lives in
//! `de_mcp::script_execute`; this file is its render half, which needs the
//! skia export stack `de-mcp` cannot link. `TakeScreenshot` renders through
//! the same capture `debug_screenshot` uses, `Export` through the
//! `export_nodes` raster / PDF writers, both from the state the script had
//! when it asked.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use de_editor_core::EditorState;
use de_mcp::script_execute::{
    execute_snapshot_with_renderer, Execute, ExportFormat, ExportRequest, ScriptRenderError,
    ScriptRenderer,
};
use de_mcp::ImageContent;

use crate::export::screenshot::{capture_scene, CaptureSpec};
use crate::export::{render_node_raster_bytes, RasterFormat};
use crate::export_pdf::render_nodes_pdf_bytes;

/// The host renderer handed to every `execute` snapshot.
pub(crate) struct HostScriptRenderer;

impl ScriptRenderer for HostScriptRenderer {
    fn screenshot(
        &self,
        state: &EditorState,
        node_id: Option<&str>,
    ) -> Result<ImageContent, ScriptRenderError> {
        // Same brand-icon catalog `get_screenshot` installs before painting.
        let _ = de_editor_ui::set_brand_catalog(crate::web_static::ICONIFY_BRANDS_JSON);
        let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
        let spec = CaptureSpec {
            node_id: node_id.map(str::to_string),
            padding: 0.0,
            scale: 1.0,
            time_ms: None,
        };
        let shot = capture_scene(&scene, &spec).map_err(|e| ScriptRenderError(e.to_string()))?;
        Ok(ImageContent {
            data: shot.png_base64,
            mime_type: "image/png".into(),
        })
    }

    fn export(
        &self,
        state: &EditorState,
        request: &ExportRequest,
    ) -> Result<Vec<String>, ScriptRenderError> {
        let _ = de_editor_ui::set_brand_catalog(crate::web_static::ICONIFY_BRANDS_JSON);
        let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
        let dir = PathBuf::from(request.output_path.trim());
        std::fs::create_dir_all(&dir)
            .map_err(|e| ScriptRenderError(format!("create {}: {e}", dir.display())))?;
        let scale = request.scale.clamp(0.5, 8.0);
        let (format, ext) = match request.format {
            ExportFormat::Pdf => {
                let ids: Vec<&str> = request.node_ids.iter().map(String::as_str).collect();
                let bytes = render_nodes_pdf_bytes(&scene, &ids, scale)
                    .map_err(|e| ScriptRenderError(e.to_string()))?;
                return write(&dir.join("export.pdf"), &bytes).map(|path| vec![path]);
            }
            ExportFormat::Png => (RasterFormat::Png, "png"),
            ExportFormat::Jpeg => (RasterFormat::Jpeg, "jpeg"),
            ExportFormat::Webp => (RasterFormat::Webp, "webp"),
        };
        request
            .node_ids
            .iter()
            .map(|id| {
                let bytes = render_node_raster_bytes(&scene, id, format, scale)
                    .map_err(|e| ScriptRenderError(format!("{id}: {e}")))?;
                write(&dir.join(format!("{}.{ext}", file_stem(id))), &bytes)
            })
            .collect()
    }
}

/// A node id as a file name: instance paths (`a/b`) must not make folders.
fn file_stem(id: &str) -> String {
    id.replace(['/', '\\'], "_")
}

fn write(path: &Path, bytes: &[u8]) -> Result<String, ScriptRenderError> {
    std::fs::write(path, bytes)
        .map_err(|e| ScriptRenderError(format!("write {}: {e}", path.display())))?;
    Ok(std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string())
}

pub(crate) fn execute_snapshot(state: &EditorState) -> Execute {
    execute_snapshot_with_renderer(state, Arc::new(HostScriptRenderer))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use de_mcp::{McpTool, ToolOutcome};

    use super::execute_snapshot;

    #[test]
    fn execute_returns_a_png_per_screenshot_and_its_writes() {
        let state = de_editor_core::EditorState::new();
        let mut args = BTreeMap::new();
        args.insert(
            "script".to_string(),
            r##"const f = Insert(document, {type: "frame", name: "Shot", x: 5000, y: 0, width: 40, height: 30, fill: "#FF0000"});
TakeScreenshot([f]);
Print(Get(f).name);"##
                .to_string(),
        );
        let ToolOutcome::OkJsonWithImages {
            json,
            images,
            command,
        } = execute_snapshot(&state).call(&args)
        else {
            panic!("expected an execute result");
        };
        assert!(command.is_some(), "{json}");
        assert_eq!(images.len(), 1, "{json}");
        assert_eq!(images[0].mime_type, "image/png");
        assert!(!images[0].data.is_empty());
        assert!(json.contains("\"Shot\""), "{json}");
    }

    #[test]
    fn export_writes_one_file_per_node() {
        let dir = std::env::temp_dir().join(format!("de-execute-export-{}", std::process::id()));
        let state = de_editor_core::EditorState::new();
        let mut args = BTreeMap::new();
        args.insert(
            "script".to_string(),
            format!(
                r##"const f = Insert(document, {{type: "frame", name: "Out", x: 6000, y: 0, width: 20, height: 20, fill: "#00FF00"}});
Export([f], "png", {dir:?}, {{scale: 1}});"##,
                dir = dir.display().to_string()
            ),
        );
        let ToolOutcome::OkJsonWithImages { json, .. } = execute_snapshot(&state).call(&args)
        else {
            panic!("expected an execute result");
        };
        let value: serde_json::Value = serde_json::from_str(&json).expect("json");
        let paths = value["exported"].as_array().expect("exported paths");
        assert_eq!(paths.len(), 1, "{json}");
        let path = std::path::Path::new(paths[0].as_str().expect("path"));
        assert!(path.exists(), "{json}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
