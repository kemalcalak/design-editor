//! Per-call `filePath` routing for the desktop MCP transports.
//!
//! The TypeScript MCP treats `filePath` as the target document for
//! most tools. The Rust desktop transports are still started with a
//! primary document path, but when a tool call supplies a different
//! `filePath`, dispatch against that file for this call only.

use std::path::{Path, PathBuf};

use super::McpServeError;

const LIVE_CANVAS_SENTINELS: &[&str] = &["live://canvas", "live://canvas/current"];

struct TargetFileCall {
    tool: String,
    path: PathBuf,
}

pub fn process_message_for_file_path_arg(
    current_path: Option<&Path>,
    line: &str,
) -> Result<Option<String>, McpServeError> {
    let Some(target) = target_file_call(line, current_path)? else {
        return Ok(None);
    };
    // Only a `.de` path is created on demand; any other extension falls
    // through to the load, which refuses it without leaving a stray file.
    if target.tool == "open_document"
        && !target.path.exists()
        && de_util::document_file::is_document_path(&target.path)
    {
        create_empty_document(&target.path)?;
    }
    // Opening the target publishes its `imageThumbs` (its saves below need
    // them), and the registry is process-wide: put back the thumbnails of
    // the server's own document (the open canvas, on the live desktop path)
    // when this call ends, however it ends, so its paint and its next save
    // keep them. Known limit: a thumbnail another thread stores during the
    // call (the desktop decode worker, a document the user opens meanwhile)
    // is replaced by this snapshot; a per-document registry in jian would
    // close that.
    let _server_thumbs = RestoreThumbnails(jian_ops_schema::image_thumbs::capture_snapshot());
    let mut target_state = super::load_editor_state(&target.path)?;
    super::generate_tool::load_user_provider_settings(&mut target_state);
    super::generate_tool::land_generations_and_save(&mut target_state, &target.path);
    let mut applier_failed: Option<String> = None;
    let response =
        super::process_message_with_applier(&mut target_state, line, |tool_name, state, cmd| {
            let baseline = super::baseline_for_tool(tool_name, state);
            if !state.apply(cmd.clone()) {
                return false;
            }
            if let Some(baseline) = baseline {
                super::run_write_repairs_after_apply(state, &baseline.scope_after(state));
            }
            de_mcp::script_node::regenerate_stale_scripts(state);
            if let Err(e) = super::save_editor_state(state, &target.path) {
                applier_failed = Some(format!("save {} failed: {e}", target.path.display()));
                return false;
            }
            true
        })?;
    if let Some(msg) = applier_failed {
        eprintln!("design-editor mcp: {msg}");
    }
    Ok(response)
}

/// Whether [`process_message_for_file_path_arg`] would run `line` on another
/// file than the server's own document (a `live://canvas` sentinel or the
/// document's own path is not another file).
pub fn targets_another_file(current_path: Option<&Path>, line: &str) -> bool {
    !matches!(target_file_call(line, current_path), Ok(None))
}

/// Restores a captured thumbnail registry when dropped.
struct RestoreThumbnails(jian_ops_schema::image_thumbs::ImageThumbSnapshot);

impl Drop for RestoreThumbnails {
    fn drop(&mut self) {
        jian_ops_schema::image_thumbs::restore_snapshot(std::mem::take(&mut self.0));
    }
}

fn target_file_call(
    line: &str,
    current_path: Option<&Path>,
) -> Result<Option<TargetFileCall>, McpServeError> {
    let Some(call) = de_mcp::parse_tool_call(line.trim()) else {
        return Ok(None);
    };
    let source_arg = if call.tool == "save_document" {
        "sourceFilePath"
    } else {
        "filePath"
    };
    let Some(raw) = call
        .arguments
        .get(source_arg)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if LIVE_CANVAS_SENTINELS.contains(&raw) {
        return Ok(None);
    }
    let target = PathBuf::from(raw);
    if current_path.is_some_and(|current| same_path(&target, current)) {
        return Ok(None);
    }
    Ok(Some(TargetFileCall {
        tool: call.tool,
        path: target,
    }))
}

fn create_empty_document(path: &Path) -> Result<(), McpServeError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| McpServeError::Document(format!("create {}: {e}", parent.display())))?;
    }
    let version = de_editor_core::document_version::PRODUCT_VERSION;
    std::fs::write(
        path,
        format!("{{\n  \"version\": \"{version}\",\n  \"children\": []\n}}\n"),
    )
    .map_err(|e| McpServeError::Document(format!("write {}: {e}", path.display())))
}

fn same_path(target: &Path, current: &Path) -> bool {
    match (target.canonicalize(), current.canonicalize()) {
        (Ok(target), Ok(current)) => target == current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::process_message_for_file_path_arg;
    use jian_ops_schema::image_thumbs;

    /// A call on another file opens it (its thumbnails are the active ones
    /// while the call runs and saves) and then leaves the thumbnails of the
    /// server's own document in place (#458).
    #[test]
    fn a_call_on_another_file_keeps_the_server_documents_thumbnails() {
        let _guard = crate::test_support::lock_thumbnail_registry();
        let dir = std::env::temp_dir().join(format!(
            "de-file-path-thumbs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let server_id = 9_458_101;
        let other_id = 9_458_102;
        image_thumbs::clear_registry();
        image_thumbs::store_thumb(server_id, vec![0xff, 0xd8, 0x01, 0xff, 0xd9]);
        let other = dir.join("other.de");
        std::fs::write(
            &other,
            format!(
                r#"{{"version":"1.0.0","imageThumbs":{{"{other_id}":"/9j/2Q=="}},"children":[]}}"#
            ),
        )
        .expect("other document");
        let line = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "open_document", "arguments": {"filePath": other.to_string_lossy()}}
        })
        .to_string();

        let response = process_message_for_file_path_arg(Some(&dir.join("server.de")), &line)
            .expect("dispatch")
            .expect("handled as a call on the other file");

        assert!(!response.contains(r#""isError":true"#), "{response}");
        assert!(image_thumbs::thumb_for(server_id).is_some());
        assert!(image_thumbs::thumb_for(other_id).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
