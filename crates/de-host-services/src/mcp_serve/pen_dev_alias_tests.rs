//! pen.dev's tool names over the file-backed server, each called with the
//! arguments pen.dev documents (#392).

use super::*;

/// A scratch `.de` document holding one frame, and its folder.
fn document(name: &str) -> (std::path::PathBuf, EditorState) {
    let dir = std::env::temp_dir().join(format!(
        "de-pen-dev-alias-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("doc.de");
    let state = crate::doc_io::load_editor_state_from_source(
        r##"{"version":"1.0.0","children":[{"id":"hero","type":"frame","name":"Hero","x":0,"y":0,"width":200,"height":80,"children":[{"id":"title","type":"text","content":"Hello"}]}]}"##,
        de_editor_core::Locale::EnUs,
    )
    .expect("loads");
    save_editor_state(&state, &path).expect("saves");
    (path, state)
}

/// Call `tool` with `arguments` and return the reply's result text,
/// failing on a tool error.
fn call(state: &mut EditorState, path: &Path, tool: &str, arguments: serde_json::Value) -> String {
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
    assert!(
        !response.contains(r#""isError":true"#),
        "{tool}: {response}"
    );
    tool_text(&response)
}

fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn tools_list_names_every_pen_dev_tool() {
    let state = EditorState::new();
    let response: serde_json::Value =
        serde_json::from_str(&tools_list_response("1", &state, false)).expect("json");
    let tools = response["result"]["tools"].as_array().expect("tools");
    let pen_dev: Vec<&str> = tools
        .iter()
        .filter(|tool| {
            tool["description"]
                .as_str()
                .is_some_and(|d| d.starts_with("pen.dev-compatible name for "))
        })
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        pen_dev,
        [
            "read_skill",
            "get_app_state",
            "get_style",
            "save",
            "import_library",
            "export_html"
        ]
    );
    assert_eq!(
        de_mcp::pen_dev_aliases::PEN_DEV_ALIAS_SCHEMAS.len(),
        6,
        "add a new pen.dev name to this test"
    );
    // pen.dev's names that are ours too stay listed once, under our schema.
    for shared in [
        "execute",
        "browser",
        "spawn_agents",
        "list_libraries",
        "remove_library",
    ] {
        let count = tools
            .iter()
            .filter(|tool| tool["name"].as_str() == Some(shared))
            .count();
        assert_eq!(count, 1, "{shared}");
    }
    // The in-app agent's catalog never sees them.
    for name in pen_dev {
        assert!(
            !TOOL_SCHEMAS
                .iter()
                .any(|schema| schema.contains(&format!(r#""name":"{name}""#))),
            "{name} must stay out of TOOL_SCHEMAS"
        );
    }
}

#[test]
fn read_skill_and_get_style_answer_with_our_guides() {
    let (path, mut state) = document("guides");
    let index = call(&mut state, &path, "read_skill", serde_json::json!({}));
    assert!(index.contains("execute"), "{index}");
    let execute = call(
        &mut state,
        &path,
        "read_skill",
        serde_json::json!({"path": "execute.md"}),
    );
    assert!(execute.contains(r#""topic":"execute""#), "{execute}");
    let style = call(&mut state, &path, "get_style", serde_json::json!({}));
    assert!(!style.is_empty());
    cleanup(&path);
}

#[test]
fn get_app_state_is_the_editor_state() {
    let (path, mut state) = document("state");
    let text = call(&mut state, &path, "get_app_state", serde_json::json!({}));
    assert!(text.contains("hero"), "{text}");
    cleanup(&path);
}

#[test]
fn save_without_a_path_saves_the_document_file() {
    let (path, mut state) = document("save");
    std::fs::remove_file(&path).expect("remove the file to see it written");
    let text = call(&mut state, &path, "save", serde_json::json!({}));
    assert!(text.contains(r#""ok":"true""#), "{text}");
    assert!(path.exists(), "save wrote the bound file");
    // pen.dev's own `.pen` path does not redirect the save.
    std::fs::remove_file(&path).expect("remove again");
    call(
        &mut state,
        &path,
        "save",
        serde_json::json!({"filePath": "/elsewhere/app.pen"}),
    );
    assert!(path.exists());
    cleanup(&path);
}

#[test]
fn import_list_and_remove_a_library_by_pen_dev_id() {
    let (path, mut state) = document("library");
    let library = path.with_file_name("ui.lib.de");
    std::fs::write(
        &library,
        r#"{"version":"1.0.0","children":[{"id":"btn","type":"frame","name":"Button","reusable":true,"width":80,"height":32}]}"#,
    )
    .expect("library file");
    call(
        &mut state,
        &path,
        "import_library",
        serde_json::json!({"path": library.to_string_lossy()}),
    );
    state = load_editor_state(&path).expect("reload");
    let listed: serde_json::Value = serde_json::from_str(&call(
        &mut state,
        &path,
        "list_libraries",
        serde_json::json!({}),
    ))
    .expect("json");
    let id = listed["libraries"][0]["id"]
        .as_str()
        .expect("pen.dev id")
        .to_string();
    assert!(listed["libraries"][0]["uri"].is_string(), "{listed}");
    call(
        &mut state,
        &path,
        "remove_library",
        serde_json::json!({"id": id}),
    );
    state = load_editor_state(&path).expect("reload");
    assert!(state.editor_ui.libraries.entries.is_empty());
    cleanup(&path);
}

#[test]
fn execute_takes_pen_devs_input() {
    let (path, mut state) = document("execute");
    let text = call(
        &mut state,
        &path,
        "execute",
        serde_json::json!({"input": "Print(\"ran\")"}),
    );
    assert!(text.contains("ran"), "{text}");
    cleanup(&path);
}

#[test]
fn export_html_writes_or_returns_the_html() {
    let (path, mut state) = document("export");
    let inline = call(
        &mut state,
        &path,
        "export_html",
        serde_json::json!({"filePath": "/work/app.pen", "nodeIds": ["hero"]}),
    );
    assert!(inline.contains("Hello"), "{inline}");
    let out = path.with_file_name("hero.html");
    let written = call(
        &mut state,
        &path,
        "export_html",
        serde_json::json!({
            "filePath": "/work/app.pen",
            "nodeIds": ["hero"],
            "outputPath": out.to_string_lossy(),
            "format": "html-css",
            "includeHtmlScaffold": true,
            "includeLayerIds": true,
            "includeLayerNames": false
        }),
    );
    assert!(written.contains("filePath"), "{written}");
    assert!(std::fs::read_to_string(&out)
        .expect("written")
        .contains("Hello"));
    cleanup(&path);
}
