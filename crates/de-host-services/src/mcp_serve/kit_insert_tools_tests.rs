//! End-to-end coverage for the dynamic `insert_<component>` kit tools:
//! a name advertised by `tools/list` must also dispatch on `tools/call`.

use super::*;

/// Names that `tools/list` advertises and that are backed by a kit
/// component, in listing order.
fn listed_kit_tool_names(state: &mut EditorState) -> Vec<String> {
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let response = process_message_with_applier(state, line, |_, _, _| false)
        .expect("dispatch")
        .expect("response");
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or(&response);
    let json: serde_json::Value = serde_json::from_str(body).expect("tools/list json");
    let tools = json["result"]["tools"].as_array().expect("tools array");
    let kit_names: Vec<String> = de_mcp::element_tools::insert_kit_component_tools(state)
        .iter()
        .map(|tool| tool.name().to_string())
        .collect();
    tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .filter(|name| kit_names.iter().any(|kit| kit == name))
        .map(str::to_string)
        .collect()
}

#[test]
fn listed_kit_tool_inserts_component_over_mcp() {
    let mut state = EditorState::new();
    let names = listed_kit_tool_names(&mut state);
    assert!(
        names.iter().any(|name| name == "insert_btn_primary"),
        "tools/list must advertise insert_btn_primary: {names:?}"
    );
    let before = state.active_children().len();
    let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"insert_btn_primary","arguments":{"x":"10","y":"20"}}}"#;
    let response =
        process_message_with_applier(&mut state, line, |_, state, cmd| state.apply(cmd.clone()))
            .expect("dispatch")
            .expect("response");
    let text = tool_text(&response);
    assert!(!text.contains("unknown tool"), "{response}");
    assert!(text.contains("btn-primary"), "{response}");
    assert_eq!(
        state.active_children().len(),
        before + 1,
        "the kit component must land on the active page"
    );
}

#[test]
fn every_listed_kit_tool_dispatches() {
    let mut state = EditorState::new();
    let names = listed_kit_tool_names(&mut state);
    assert!(
        !names.is_empty(),
        "builtin kits must advertise element tools"
    );
    assert_eq!(
        names.len(),
        de_mcp::element_tools::insert_kit_component_tools(&state).len(),
        "tools/list must advertise every kit tool"
    );
    for (id, name) in names.iter().enumerate() {
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{{}}}}}}"#
        );
        let before = state.active_children().len();
        let response = process_message_with_applier(&mut state, &line, |_, state, cmd| {
            state.apply(cmd.clone())
        })
        .expect("dispatch")
        .expect("response");
        assert!(
            !tool_text(&response).contains("unknown tool"),
            "{name}: {response}"
        );
        assert_eq!(
            state.active_children().len(),
            before + 1,
            "{name} inserted nothing"
        );
    }
}
