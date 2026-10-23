//! `import_pen_dev` writes land exactly as authored: the post-write repairs
//! that tidy AI-generated designs (status bars on phone screens, …) must not
//! touch an imported design.

use super::{load_editor_state, process_message, writes_verbatim};
use de_editor_core::PenNodeExt;

#[test]
fn only_imports_and_history_steps_skip_the_write_repairs() {
    assert!(writes_verbatim("import_pen_dev"));
    assert!(writes_verbatim("undo"));
    assert!(writes_verbatim("redo"));
    assert!(!writes_verbatim("import_html"));
    assert!(!writes_verbatim("batch_design"));
}

#[test]
fn an_imported_phone_screen_keeps_its_own_children() {
    let dir = std::env::temp_dir().join(format!(
        "design-editor-mcp-pen-dev-verbatim-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let pen = dir.join("chat.pen");
    // A 360-wide phone screen with no status bar: the mobile repair would
    // insert one into an AI-generated screen of this shape.
    std::fs::write(
        &pen,
        r##"{"version":"2.21","children":[{"type":"frame","id":"phone","name":"Chat",
            "x":0,"y":0,"width":360,"height":900,"layout":"vertical",
            "children":[{"type":"text","id":"t","content":"Hello","fill":"#000000"}]}]}"##,
    )
    .expect("write pen");
    let path = dir.join("doc.de");
    let mut state = de_editor_core::EditorState::new();
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "import_pen_dev", "arguments": {"penPath": pen.display().to_string()}}
    })
    .to_string();

    let response = process_message(&mut state, &path, &line)
        .expect("dispatch")
        .expect("response");
    assert!(!response.contains(r#""isError":true"#), "{response}");

    let saved = load_editor_state(&path).expect("saved document");
    let phone = saved
        .active_children()
        .iter()
        .find(|node| node.id_str() == "phone")
        .expect("imported screen");
    let children = phone.children().expect("children");
    assert_eq!(children.len(), 1, "no status bar was added");
    assert_eq!(children[0].id_str(), "t");
    assert_eq!(phone.height_px(), Some(900.0));
    let _ = std::fs::remove_dir_all(dir);
}
