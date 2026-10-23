//! Home previews for documents an MCP client wrote, so a design an agent
//! saved shows on Home's Recent card without being opened in the app.

use std::path::{Path, PathBuf};

use crate::home_preview::queue;

/// Queue a preview of the `.op` file a transport just saved.
pub(super) fn after_save(path: &Path) {
    queue(path);
}

/// The file a successful `save_document` call in `line` wrote, read from
/// its `response`. `None` for any other tool or a failed save.
pub(crate) fn saved_document_path(line: &str, response: &str) -> Option<PathBuf> {
    // Every live request passes through here; skip the parse for the rest.
    if !line.contains("save_document")
        || de_mcp::parse_tool_call(line.trim())?.tool != "save_document"
    {
        return None;
    }
    saved_path_from_reply(response)
}

/// The `filePath` of a successful `save_document` reply.
fn saved_path_from_reply(response: &str) -> Option<PathBuf> {
    let reply: serde_json::Value = serde_json::from_str(response.trim()).ok()?;
    let text = reply["result"]["content"][0]["text"].as_str()?;
    let result: serde_json::Value = serde_json::from_str(text).ok()?;
    if result["ok"].as_str() != Some("true") {
        return None;
    }
    result["filePath"].as_str().map(PathBuf::from)
}

/// Queue a preview of the file a `save_document` call wrote, when its
/// `response` reports success.
pub(super) fn after_save_document(response: Option<&str>) {
    if let Some(path) = response.and_then(saved_path_from_reply) {
        after_save(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(tool: &str) -> String {
        format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool}","arguments":{{"filePath":"/tmp/a.op"}}}}}}"#
        )
    }

    fn reply(text: &str) -> String {
        serde_json::json!({"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":text}]}})
            .to_string()
    }

    #[test]
    fn only_a_successful_save_document_names_its_file() {
        let ok = reply(r#"{"ok":"true","filePath":"/tmp/a.op"}"#);
        assert_eq!(
            saved_document_path(&call("save_document"), &ok),
            Some(PathBuf::from("/tmp/a.op"))
        );
        assert_eq!(saved_document_path(&call("get_node"), &ok), None);
        let failed = reply("save document failed: denied");
        assert_eq!(saved_document_path(&call("save_document"), &failed), None);
    }

    #[test]
    fn an_mcp_save_writes_the_files_preview() {
        let dir = std::env::temp_dir().join(format!("de-mcp-preview-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let doc = dir.join("agent.op");
        let state = crate::doc_io::load_editor_state_from_source(
            r##"{"version":"1.0.0","children":[{"id":"r","type":"rectangle","x":0,"y":0,"width":80,"height":40,"fill":[{"type":"solid","color":"#123456"}]}]}"##,
            de_editor_core::Locale::EnUs,
        )
        .expect("loads");
        super::super::save_editor_state(&state, &doc).expect("saves");
        assert!(crate::home_preview::wait_idle(
            std::time::Duration::from_secs(30)
        ));
        let thumbs = crate::home_preview::previews_dir().expect("test root");
        let preview = crate::home_preview::preview_path(&thumbs, &doc);
        assert!(preview.exists(), "the save queued a preview");
        assert!(!crate::home_preview::is_stale(&doc, &preview));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_file(preview);
    }
}
