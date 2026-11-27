//! Document save tool used by the Rust HTTP CLI transport.

use std::collections::BTreeMap;
use std::path::PathBuf;

use de_editor_core::EditorState;

use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct SaveDocument {
    document_payload: Vec<u8>,
}

impl McpTool for SaveDocument {
    fn name(&self) -> &str {
        "save_document"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(path) = target_path(args.get("filePath").map(String::as_str)) else {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "filePath is required".into(),
            );
        };
        if !de_util::document_file::is_document_path(&path) {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!(
                    "filePath must name a {} document: {}",
                    de_util::document_file::SUFFIX,
                    path.display()
                ),
            );
        }
        // `singleFile: true` keeps every image embedded, for sharing; the
        // default is the folder layout (`<stem>.assets/`).
        let single_file = match args.get("singleFile").map(String::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            Some(other) => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("singleFile must be true or false, got {other}"),
                );
            }
        };
        let written = if single_file {
            std::fs::write(&path, &self.document_payload).map_err(|e| e.to_string())
        } else {
            de_pen_loader::asset_folder::write_document(&path, self.document_payload.clone())
                .map_err(|e| e.to_string())
        };
        if let Err(e) = written {
            return ToolOutcome::Err(
                ToolErrorCode::ToolFailed,
                format!("save document failed: {e}"),
            );
        }
        let mut out = BTreeMap::new();
        out.insert("ok".into(), "true".into());
        out.insert(
            "filePath".into(),
            de_util::display_path::display_path(&path),
        );
        ToolOutcome::Ok(out)
    }
}

/// The `save_document` snapshot for a call that will write `file_path` (the
/// request's `filePath` argument): library links are written relative to
/// that file's folder, as the host's own Save writes them. Without a
/// target they are written absolute, and the next open makes them
/// relative again.
pub fn save_document_snapshot_to(state: &EditorState, file_path: Option<&str>) -> SaveDocument {
    let target = target_path(file_path);
    // Stream once into the immutable MCP snapshot. The schema writer
    // externalizes shared images while serializing, avoiding both a
    // document-sized `Value` and a second final `String` allocation.
    let mut document_payload = Vec::new();
    // The document's own thumbnails (#469), never another file's.
    let thumbnails = state.image_thumbs.snapshot();
    if jian_ops_schema::image_table::write_document_with_extension(
        &mut document_payload,
        &state.doc,
        &thumbnails,
        "editorMeta",
        // The same metadata desktop Save writes, so an MCP save keeps the
        // scenario, pinned style guide and library links too.
        &de_pen_loader::EditorMeta {
            libraries: de_pen_loader::links_for_document(state, target.as_deref()),
            ..de_pen_loader::EditorMeta::from_state(state)
        },
    )
    .is_err()
    {
        document_payload.clear();
        document_payload.extend_from_slice(b"{}");
    }
    SaveDocument { document_payload }
}

/// The file a `filePath` argument names. The snapshot (for its library
/// links) and the call (for the write) both resolve it here, so the links
/// are relative to the folder the file lands in.
fn target_path(raw: Option<&str>) -> Option<PathBuf> {
    raw.filter(|path| !path.trim().is_empty()).map(resolve_path)
}

fn resolve_path(raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}
