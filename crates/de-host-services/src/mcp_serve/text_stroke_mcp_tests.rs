//! Outlined text over MCP (#282): the stroke tools used to report a write
//! on text and change nothing, since text had no stroke to write.

use super::*;
use jian_ops_schema::node::PenNode;

fn call(state: &mut EditorState, path: &Path, tool: &str, arguments: serde_json::Value) -> String {
    let line = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    })
    .to_string();
    process_message(state, path, &line)
        .expect("dispatch")
        .expect("response")
}

#[test]
fn stroke_tools_outline_a_text_node() {
    let path = std::env::temp_dir().join(format!(
        "design-editor-mcp-text-stroke-{}-{}.de",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::write(
        &path,
        r#"{"version":"1.0.0","children":[{"type":"text","id":"t","x":0,"y":0,"content":"Outlined"}]}"#,
    )
    .expect("seed document");
    let mut state = load_editor_state(&path).expect("load document");
    let hex = call(
        &mut state,
        &path,
        "set_node_stroke_hex",
        serde_json::json!({"node_id": "t", "hex": "#FF00FF"}),
    );
    assert!(!hex.contains(r#""isError":true"#), "{hex}");
    let width = call(
        &mut state,
        &path,
        "set_node_stroke_width",
        serde_json::json!({"node_id": "t", "width": "3"}),
    );
    assert!(!width.contains(r#""isError":true"#), "{width}");

    let saved = load_editor_state(&path).expect("saved document");
    let PenNode::Text(text) = &saved.active_children()[0] else {
        panic!("text node");
    };
    let stroke = text.stroke.as_ref().expect("the outline was saved");
    assert_eq!(
        de_editor_core::first_solid_stroke_hex(&saved.active_children()[0]),
        Some("#FF00FF")
    );
    assert!(matches!(
        stroke.thickness,
        jian_ops_schema::style::StrokeThickness::Uniform(w) if w == 3.0
    ));
    let _ = std::fs::remove_file(&path);
}
