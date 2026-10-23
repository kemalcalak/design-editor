//! Document save tool tests.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::test_fixtures::sample;
use super::{save_document_snapshot, McpTool, ToolErrorCode, ToolOutcome};

#[test]
fn save_document_writes_current_pen_document_to_target_path() {
    let mut state = sample();
    state.ui.active_page_index = 3;
    state.editor_ui.preserve_authored_geometry = true;
    let dir = temp_dir("save-document");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let out_path = dir.join("copy.de");

    let tool = save_document_snapshot(&state);
    let mut args = BTreeMap::new();
    args.insert("filePath".into(), out_path.display().to_string());

    match tool.call(&args) {
        ToolOutcome::Ok(out) => {
            assert_eq!(out.get("ok"), Some(&"true".to_string()));
            assert_eq!(out.get("filePath"), Some(&out_path.display().to_string()));
            let saved: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&out_path).expect("saved document"))
                    .expect("saved document json");
            assert_eq!(saved["version"], state.doc.version);
            assert!(saved["children"].is_array() || saved["pages"].is_array());
            assert_eq!(saved["editorMeta"]["activePageIndex"], 3);
            assert_eq!(saved["editorMeta"]["preserveAuthoredGeometry"], true);
        }
        other => panic!("expected save ok, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_document_requires_file_path() {
    let tool = save_document_snapshot(&sample());
    match tool.call(&BTreeMap::new()) {
        ToolOutcome::Err(code, message) => {
            assert_eq!(code, ToolErrorCode::MissingArgument);
            assert!(message.contains("filePath"));
        }
        other => panic!("expected missing argument, got {other:?}"),
    }
}

#[test]
fn save_document_refuses_a_non_document_extension() {
    let dir = temp_dir("save-document-extension");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let tool = save_document_snapshot(&sample());
    for name in ["copy.op", "copy.pen", "copy"] {
        let out_path = dir.join(name);
        let mut args = BTreeMap::new();
        args.insert("filePath".into(), out_path.display().to_string());
        match tool.call(&args) {
            ToolOutcome::Err(code, message) => {
                assert_eq!(code, ToolErrorCode::InvalidArgument);
                assert!(message.contains(".de"), "{message}");
            }
            other => panic!("expected {name} to be refused, got {other:?}"),
        }
        assert!(!out_path.exists(), "{name} must not be written");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn state_with_image() -> de_editor_core::EditorState {
    let url = format!("data:image/png;base64,{}", "iVBORw0KGgoA".repeat(600));
    let src = format!(
        r#"{{"version":"1.0.0","children":[{{"type":"image","id":"a","width":4,"height":4,"src":"{url}"}}]}}"#
    );
    de_editor_core::EditorState::from_document(
        de_pen_loader::load_canonical(&src).expect("fixture").value,
    )
}

#[test]
fn save_document_writes_the_folder_layout_by_default() {
    let dir = temp_dir("save-document-folder");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let out_path = dir.join("copy.de");
    let tool = save_document_snapshot(&state_with_image());
    let mut args = BTreeMap::new();
    args.insert("filePath".into(), out_path.display().to_string());

    assert!(matches!(tool.call(&args), ToolOutcome::Ok(_)));

    let saved = std::fs::read_to_string(&out_path).expect("saved");
    assert!(saved.contains("\"src\": \"copy.assets/"), "{saved}");
    assert!(!saved.contains("base64"));
    assert_eq!(
        std::fs::read_dir(dir.join("copy.assets")).unwrap().count(),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_document_single_file_embeds_images_and_rejects_bad_flags() {
    let dir = temp_dir("save-document-single");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let out_path = dir.join("share.de");
    let tool = save_document_snapshot(&state_with_image());
    let mut args = BTreeMap::new();
    args.insert("filePath".into(), out_path.display().to_string());
    args.insert("singleFile".into(), "true".into());

    assert!(matches!(tool.call(&args), ToolOutcome::Ok(_)));
    assert!(std::fs::read_to_string(&out_path)
        .unwrap()
        .contains("base64"));
    assert!(!dir.join("share.assets").exists());

    args.insert("singleFile".into(), "1".into());
    assert!(matches!(
        tool.call(&args),
        ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "design-editor-document-save-{label}-{}-{nanos}",
        std::process::id()
    ))
}
