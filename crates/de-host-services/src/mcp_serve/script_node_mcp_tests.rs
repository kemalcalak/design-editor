//! A script node written over file-backed MCP is saved with its generated
//! children, and an input edit regenerates them (#19).

use super::*;
use de_editor_core::PenNodeExt;

fn call(state: &mut EditorState, path: &Path, tool: &str, arguments: serde_json::Value) {
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    })
    .to_string();
    let response = process_message(state, path, &line)
        .expect("dispatch")
        .expect("response");
    assert!(!response.contains(r#""isError":true"#), "{response}");
}

fn saved_children(path: &Path) -> usize {
    let saved = load_editor_state(path).expect("saved document");
    saved.active_children()[0]
        .children()
        .map_or(0, |children| children.len())
}

#[test]
fn file_mcp_saves_a_script_node_with_its_generated_children() {
    let path = std::env::temp_dir().join(format!(
        "design-editor-mcp-script-node-{}-{}.de",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::write(&path, r#"{"version":"1.0.0","children":[]}"#).expect("seed document");
    let mut state = load_editor_state(&path).expect("load document");
    let operations = r#"chart=I(null,{"type":"script","name":"Bars","x":0,"y":0,"width":120,"height":40,
        "script":"// @input bars: number = 2\nreturn Array.from({length: pencil.input.bars}, (_, i) => ({type: 'rectangle', x: i * 30, width: 20, height: pencil.height}));"})"#;
    call(
        &mut state,
        &path,
        "batch_design",
        serde_json::json!({ "operations": operations }),
    );
    assert_eq!(saved_children(&path), 2);

    let id = state.active_children()[0].id_str().to_string();
    call(
        &mut state,
        &path,
        "update_node",
        serde_json::json!({ "nodeId": id, "data": { "inputs": { "bars": 4 } } }),
    );
    assert_eq!(saved_children(&path), 4);
    let _ = std::fs::remove_file(&path);
}
