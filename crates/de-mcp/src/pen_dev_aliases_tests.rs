use super::*;
use crate::parse_tool_call;

fn call(name: &str, arguments: &str) -> crate::ToolCall {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{name}","arguments":{arguments}}}}}"#
    );
    parse_tool_call(&line).expect("parses")
}

#[test]
fn every_alias_has_one_schema_and_export_html_too() {
    let names: Vec<String> = PEN_DEV_ALIAS_SCHEMAS
        .iter()
        .map(|schema| {
            let value: serde_json::Value = serde_json::from_str(schema).expect("schema is JSON");
            assert!(
                value["description"]
                    .as_str()
                    .is_some_and(|d| d.starts_with("pen.dev-compatible name for ")),
                "{schema}"
            );
            value["name"].as_str().expect("name").to_string()
        })
        .collect();
    let mut expected: Vec<&str> = PEN_DEV_ALIASES.iter().map(|alias| alias.name).collect();
    expected.push("export_html");
    assert_eq!(names, expected);
}

#[test]
fn read_skill_reads_the_topic_its_path_names() {
    let parsed = call("read_skill", r#"{"path":"execute.md"}"#);
    assert_eq!(parsed.tool, "get_guidelines");
    assert_eq!(
        parsed.arguments.get("topic").map(String::as_str),
        Some("execute")
    );
    assert!(!parsed.arguments.contains_key("path"));
    let nested = call("read_skill", r#"{"path":"skills/web-app.md"}"#);
    assert_eq!(
        nested.arguments.get("topic").map(String::as_str),
        Some("web-app")
    );
    let bare = call("read_skill", "{}");
    assert_eq!(bare.tool, "get_guidelines");
    assert!(bare.arguments.is_empty());
}

#[test]
fn get_app_state_drops_pen_devs_schema_flag() {
    let parsed = call("get_app_state", r#"{"include_schema":true}"#);
    assert_eq!(parsed.tool, "get_editor_state");
    assert!(parsed.arguments.is_empty());
}

#[test]
fn renamed_tools_keep_their_arguments() {
    let style = call("get_style", r#"{"name":"minimal"}"#);
    assert_eq!(style.tool, "get_style_guide");
    assert_eq!(
        style.arguments.get("name").map(String::as_str),
        Some("minimal")
    );
    let library = call("import_library", r#"{"path":"./ui.lib.pen"}"#);
    assert_eq!(library.tool, "add_library");
    assert_eq!(
        library.arguments.get("path").map(String::as_str),
        Some("./ui.lib.pen"),
        "a library path is not a document filePath"
    );
    let save = call("save", r#"{"filePath":"/tmp/out.de"}"#);
    assert_eq!(save.tool, "save_document");
    assert_eq!(
        save.arguments.get("filePath").map(String::as_str),
        Some("/tmp/out.de")
    );
}

#[test]
fn a_pen_file_path_is_not_this_servers_document() {
    let save = call("save", r#"{"filePath":"/work/app.pen"}"#);
    assert!(!save.arguments.contains_key("filePath"));
    let export = call(
        "export_html",
        r#"{"filePath":"/work/App.PEN","nodeIds":["a"]}"#,
    );
    assert_eq!(export.tool, "export_html");
    assert!(!export.arguments.contains_key("filePath"));
    // Our own tools keep a `.pen` filePath: only pen.dev's names drop it.
    let own = call("get_selection", r#"{"filePath":"/work/app.pen"}"#);
    assert!(own.arguments.contains_key("filePath"));
}

#[test]
fn shared_names_take_pen_devs_argument_names() {
    let execute = call("execute", r#"{"input":"Print(1)"}"#);
    assert_eq!(execute.tool, "execute");
    assert_eq!(
        execute.arguments.get("script").map(String::as_str),
        Some("Print(1)")
    );
    let ours = call("execute", r#"{"script":"a","input":"b"}"#);
    assert_eq!(ours.arguments.get("script").map(String::as_str), Some("a"));
    let remove = call("remove_library", r#"{"id":"ui.lib.de"}"#);
    assert_eq!(
        remove.arguments.get("path").map(String::as_str),
        Some("ui.lib.de")
    );
}

#[test]
fn other_calls_pass_through() {
    let parsed = call("get_node", r#"{"node_id":"42"}"#);
    assert_eq!(parsed.tool, "get_node");
    assert_eq!(
        parsed.arguments.get("node_id").map(String::as_str),
        Some("42")
    );
}
