//! MCP tool `execute` (#168): the script runner lives in
//! `de_mcp::script_execute`; this file is its render half, which needs the
//! skia export stack `de-mcp` cannot link. `TakeScreenshot` renders through
//! the same capture `debug_screenshot` uses, `Export` through the
//! `export_nodes` raster / SVG / PDF writers, both from the state the script
//! had when it asked.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use de_editor_core::EditorState;
use de_mcp::script_execute::{
    execute_shared_with_renderer, Execute, ExportFormat, ExportRequest, ExportWritten,
    ScriptRenderError, ScriptRenderer,
};
use de_mcp::ImageContent;

use crate::export::screenshot::{capture_scene, CaptureSpec};
use crate::export::{render_node_raster_bytes, RasterFormat};
use crate::export_pdf::render_nodes_pdf_bytes;

/// The host renderer handed to every `execute` snapshot.
pub(crate) struct HostScriptRenderer {
    /// The call runs on a narrowed live snapshot (#456): a script that
    /// writes is refused and run again on the whole document, so it renders
    /// and exports nothing here.
    narrowed_read: bool,
}

impl ScriptRenderer for HostScriptRenderer {
    fn only_for_read_only_scripts(&self) -> bool {
        self.narrowed_read
    }

    fn screenshot(
        &self,
        state: &EditorState,
        node_id: Option<&str>,
    ) -> Result<ImageContent, ScriptRenderError> {
        // Same brand-icon catalog `get_screenshot` installs before painting.
        let _ = de_editor_ui::set_brand_catalog(crate::brand_catalog::ICONIFY_BRANDS_JSON);
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
    ) -> Result<ExportWritten, ScriptRenderError> {
        let _ = de_editor_ui::set_brand_catalog(crate::brand_catalog::ICONIFY_BRANDS_JSON);
        let dir = PathBuf::from(request.output_path.trim());
        std::fs::create_dir_all(&dir)
            .map_err(|e| ScriptRenderError(format!("create {}: {e}", dir.display())))?;
        let scale = request.scale.clamp(0.5, 8.0);
        let (format, ext) = match request.format {
            // The SVG writer builds its own scene, with layer names.
            ExportFormat::Svg => return export_layered_svg(state, request, &dir),
            ExportFormat::Pdf => {
                let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
                let ids: Vec<&str> = request.node_ids.iter().map(String::as_str).collect();
                let bytes = render_nodes_pdf_bytes(&scene, &ids, scale)
                    .map_err(|e| ScriptRenderError(e.to_string()))?;
                let path = write(&dir.join("export.pdf"), &bytes)?;
                return Ok(ExportWritten {
                    paths: vec![path],
                    warnings: Vec::new(),
                });
            }
            ExportFormat::Png => (RasterFormat::Png, "png"),
            ExportFormat::Jpeg => (RasterFormat::Jpeg, "jpeg"),
            ExportFormat::Webp => (RasterFormat::Webp, "webp"),
        };
        let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
        let paths = request
            .node_ids
            .iter()
            .map(|id| {
                let bytes = render_node_raster_bytes(&scene, id, format, scale)
                    .map_err(|e| ScriptRenderError(format!("{id}: {e}")))?;
                write(&dir.join(format!("{}.{ext}", file_stem(id))), &bytes)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ExportWritten {
            paths,
            warnings: Vec::new(),
        })
    }
}

/// One layered `<nodeId>.svg` per node (#407); each file's left-out
/// features come back as warnings prefixed with the node id.
fn export_layered_svg(
    state: &EditorState,
    request: &ExportRequest,
    dir: &Path,
) -> Result<ExportWritten, ScriptRenderError> {
    let (scene, names) = de_pen_loader::editor_state_to_active_page_layout_scene_with_names(state);
    let mut written = ExportWritten::default();
    for id in &request.node_ids {
        let export = crate::export_svg::render_layered_node_svg(&scene, &names, id)
            .map_err(|e| ScriptRenderError(format!("{id}: {e}")))?;
        let path = write(
            &dir.join(format!("{}.svg", file_stem(id))),
            export.svg.as_bytes(),
        )?;
        written.paths.push(path);
        written.warnings.extend(
            export
                .warnings
                .into_iter()
                .map(|w| format!("{id}.svg: {w}")),
        );
    }
    Ok(written)
}

/// A node id as a file name: instance paths (`a/b`) must not make folders.
fn file_stem(id: &str) -> String {
    id.replace(['/', '\\'], "_")
}

fn write(path: &Path, bytes: &[u8]) -> Result<String, ScriptRenderError> {
    std::fs::write(path, bytes)
        .map_err(|e| ScriptRenderError(format!("write {}: {e}", path.display())))?;
    // Canonical, so the caller gets an absolute path; without the Windows
    // verbatim prefix `canonicalize` adds there (#408).
    Ok(de_util::display_path::display_path(
        &std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
    ))
}

#[cfg(test)]
pub(crate) fn execute_snapshot(state: &EditorState) -> Execute {
    de_mcp::script_execute::execute_snapshot_with_renderer(
        state,
        Arc::new(HostScriptRenderer {
            narrowed_read: false,
        }),
    )
}

/// [`execute_snapshot`] over the request's shared state (#456);
/// `narrowed_read` for a call answered from a narrowed live snapshot.
pub(crate) fn execute_shared(
    state: de_editor_core::shared_state::SharedState,
    narrowed_read: bool,
) -> Execute {
    execute_shared_with_renderer(state, Arc::new(HostScriptRenderer { narrowed_read }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use de_mcp::{McpTool, ToolOutcome};

    use super::execute_snapshot;

    /// The limits are spelled out in the tool description and the guide;
    /// keep both in step with the constants that enforce them.
    #[test]
    fn documented_limits_match_the_constants() {
        use de_mcp::script_execute::{
            EXECUTE_BUDGET, MAX_EXPORT_NODES, MAX_PRINT_BYTES, MAX_SCREENSHOTS, MAX_WRITES,
        };
        use de_mcp::script_runner::MAX_SCRIPT_BYTES;
        let schema = super::super::TOOL_SCHEMAS
            .iter()
            .find(|schema| schema.contains(r#""name":"execute""#))
            .expect("execute schema");
        let guide = de_ai_skills::guideline_for("execute").expect("execute guide");
        let phrases = [
            format!("{} s time limit", EXECUTE_BUDGET.as_secs()),
            format!("{} KiB source", MAX_SCRIPT_BYTES / 1024),
            format!("{MAX_WRITES} writes"),
            format!("{MAX_SCREENSHOTS} screenshots"),
            format!("{MAX_EXPORT_NODES} exported nodes"),
            format!("{} KiB of Print output", MAX_PRINT_BYTES / 1024),
        ];
        for phrase in &phrases {
            assert!(
                schema.contains(phrase.as_str()),
                "schema must say {phrase:?}"
            );
        }
        for phrase in [&phrases[0], &phrases[2], &phrases[3], &phrases[4]] {
            assert!(guide.contains(phrase.as_str()), "guide must say {phrase:?}");
        }
        assert!(guide.contains(&format!("{} KiB", MAX_PRINT_BYTES / 1024)));
    }

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

    fn execute_line(script: &str) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "execute", "arguments": {"script": script}},
        })
        .to_string()
    }

    /// A live host answers a read from a narrowed snapshot and runs a
    /// script that writes there again on the whole document (#456), so the
    /// narrowed run must leave the export to that run; a read-only script
    /// still exports from the narrowed run.
    #[test]
    fn a_narrowed_run_exports_only_when_its_script_writes_nothing() {
        let dir = std::env::temp_dir().join(format!("de-execute-narrowed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let doc: jian_ops_schema::PenDocument = serde_json::from_str(
            r##"{"version":"1.0","children":[
              {"type":"frame","id":"box","x":0,"y":0,"width":20,"height":20,
               "fill":[{"type":"solid","color":"#00FF00"}]}]}"##,
        )
        .expect("document");
        let mut state = de_editor_core::EditorState::from_document(doc);
        let dir_arg = format!("{:?}", dir.display().to_string());

        // Writes through a name the source check does not see.
        let writing = execute_line(&format!(
            r#"globalThis["Ins" + "ert"](document, {{type: "rectangle", width: 4, height: 4}});
Export(["box"], "png", {dir_arg}, {{scale: 1}});
TakeScreenshot(["box"]);"#
        ));
        let mut wrote = false;
        let reply = crate::mcp_serve::process_narrowed_read(&mut state, &writing, |_, _, _| {
            wrote = true;
            false
        })
        .expect("dispatch")
        .expect("reply");
        assert!(wrote, "the narrowed run reached the write");
        assert!(!dir.exists(), "no file from the refused run: {reply}");
        assert!(!reply.contains("image/png"), "no screenshot either");

        // The whole-document run that follows does export.
        let reply = crate::mcp_serve::process_message_with_write_applier(
            &mut state,
            &writing,
            |_, local, write| local.apply(write.command.clone()),
        )
        .expect("dispatch")
        .expect("reply");
        assert!(dir.join("box.png").exists(), "{reply}");
        let _ = std::fs::remove_dir_all(&dir);

        // A read-only script exports from the narrowed run itself.
        let reading = execute_line(&format!(
            r#"Export(["box"], "png", {dir_arg}, {{scale: 1}});"#
        ));
        let reply = crate::mcp_serve::process_narrowed_read(&mut state, &reading, |_, _, _| {
            panic!("a read-only script writes nothing")
        })
        .expect("dispatch")
        .expect("reply");
        assert!(dir.join("box.png").exists(), "{reply}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
