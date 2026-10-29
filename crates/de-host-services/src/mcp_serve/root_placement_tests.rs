//! #37 over the wire: a root insert through `process_message_with_applier`
//! (the seam every MCP transport shares) lands in free space, in front.

use super::*;
use de_editor_core::pen_node_ext::PenNodeExt;

fn seeded_state() -> EditorState {
    let src = r#"{"version":"1.0.0","children":[
      {"type":"frame","id":"seed","name":"Frame","x":0,"y":0,"width":1200,"height":800,
       "children":[{"type":"text","id":"seed-text","x":40,"y":40,"content":"Existing"}]}
    ]}"#;
    let parsed = jian_ops_schema::load_str(src).expect("parse fixture");
    EditorState::from_document(parsed.value)
}

fn call(state: &mut EditorState, tool: &str, arguments: serde_json::Value) -> String {
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    })
    .to_string();
    let response =
        process_message_with_applier(state, &line, |_, state, cmd| state.apply(cmd.clone()))
            .expect("dispatch")
            .expect("response");
    assert!(!response.contains(r#""isError":true"#), "{response}");
    response
}

fn roots(state: &EditorState) -> Vec<(String, f64, f64)> {
    state
        .active_children()
        .iter()
        .map(|node| {
            let base = node.base();
            (
                base.name.clone().unwrap_or_default(),
                base.x.unwrap_or(0.0),
                base.y.unwrap_or(0.0),
            )
        })
        .collect()
}

#[test]
fn insert_node_over_an_existing_frame_lands_beside_it_in_front() {
    let mut state = seeded_state();
    call(
        &mut state,
        "insert_node",
        serde_json::json!({"kind":"rect","name":"Box","x":"200","y":"200","width":"400","height":"300","fill_hex":"#E5484D"}),
    );
    assert_eq!(
        roots(&state),
        vec![("Box".into(), 1280.0, 0.0), ("Frame".into(), 0.0, 0.0)]
    );
}

#[test]
fn batch_design_root_without_position_lands_beside_existing_content() {
    let mut state = seeded_state();
    call(
        &mut state,
        "batch_design",
        serde_json::json!({"operations": r##"d=I(document,{"type":"frame","name":"Batch","width":600,"height":400})"##}),
    );
    assert_eq!(
        roots(&state),
        vec![("Batch".into(), 1280.0, 0.0), ("Frame".into(), 0.0, 0.0)]
    );
}

#[test]
fn allow_overlap_keeps_the_requested_position() {
    let mut state = seeded_state();
    call(
        &mut state,
        "insert_node",
        serde_json::json!({"kind":"rect","name":"Box","x":"200","y":"200","width":"400","height":"300","allowOverlap":true}),
    );
    assert_eq!(
        roots(&state),
        vec![("Box".into(), 200.0, 200.0), ("Frame".into(), 0.0, 0.0)]
    );
}

#[test]
fn parented_insert_keeps_its_position_inside_the_parent() {
    let mut state = seeded_state();
    call(
        &mut state,
        "insert_node",
        serde_json::json!({"kind":"rect","name":"Inner","x":"200","y":"200","width":"40","height":"30","parent":"seed"}),
    );
    assert_eq!(roots(&state), vec![("Frame".into(), 0.0, 0.0)]);
    let inner = state.active_children()[0]
        .children()
        .expect("seed children")
        .iter()
        .find(|node| node.base().name.as_deref() == Some("Inner"))
        .expect("inner inserted under seed");
    assert_eq!((inner.base().x, inner.base().y), (Some(200.0), Some(200.0)));
}

#[test]
fn tools_list_advertises_allow_overlap_on_placing_tools_only() {
    let state = seeded_state();
    let response = tools_list_response("1", &state, false);
    let parsed: serde_json::Value = serde_json::from_str(&response).expect("tools/list is JSON");
    let tools = parsed["result"]["tools"].as_array().expect("tools array");
    let tool = |name: &str| {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("{name} listed"))
    };
    for name in ["insert_node", "batch_design", "copy_node"] {
        let t = tool(name);
        assert_eq!(
            t["inputSchema"]["properties"]["allowOverlap"]["type"], "boolean",
            "{name}"
        );
        assert!(
            t["description"]
                .as_str()
                .is_some_and(|d| d.contains("free canvas space")),
            "{name}: {}",
            t["description"]
        );
    }
    let kit = tools
        .iter()
        .find(|t| {
            t["name"]
                .as_str()
                .is_some_and(|n| n.starts_with("insert_") && n != "insert_node")
        })
        .expect("a kit insert tool");
    assert_eq!(
        kit["inputSchema"]["properties"]["allowOverlap"]["type"],
        "boolean"
    );
    assert!(tool("update_node")["inputSchema"]["properties"]
        .get("allowOverlap")
        .is_none());
    assert!(tool("find_empty_space")["description"]
        .as_str()
        .is_some_and(|d| d.contains("Optional for new page roots")));
}

#[test]
fn moving_a_child_to_the_root_is_not_treated_as_an_insert() {
    let mut state = seeded_state();
    call(
        &mut state,
        "batch_design",
        serde_json::json!({"operations": r#"M("seed-text",null)"#}),
    );
    let names: Vec<String> = roots(&state).into_iter().map(|(n, _, _)| n).collect();
    assert_eq!(
        names.first().map(String::as_str),
        Some("Frame"),
        "{names:?}"
    );
    let moved = state
        .active_children()
        .iter()
        .find(|n| n.id_str() == "seed-text")
        .expect("moved to root");
    assert_eq!(moved.base().x, Some(40.0), "M() keeps its own position");
}

#[test]
fn replacing_a_root_keeps_its_slot_and_position() {
    let src = r#"{"version":"1.0.0","children":[
      {"type":"frame","id":"seed","name":"Frame","x":0,"y":0,"width":1200,"height":800,
       "children":[{"type":"text","id":"seed-text","x":40,"y":40,"content":"Existing"}]},
      {"type":"rectangle","id":"other","name":"Other","x":1300,"y":0,"width":300,"height":300}
    ]}"#;
    let mut state =
        EditorState::from_document(jian_ops_schema::load_str(src).expect("parse").value);
    call(
        &mut state,
        "batch_design",
        serde_json::json!({"operations": r##"R("other",{"type":"rectangle","name":"Replaced","x":1300,"y":0,"width":300,"height":300})"##}),
    );
    assert_eq!(
        roots(&state),
        vec![("Frame".into(), 0.0, 0.0), ("Replaced".into(), 1300.0, 0.0)]
    );
}

#[test]
fn tool_search_serves_the_same_placement_wording_as_tools_list() {
    let mut state = seeded_state();
    let response = call(
        &mut state,
        "ToolSearch",
        serde_json::json!({"query": "select:insert_node"}),
    );
    assert!(response.contains("allowOverlap"), "{response}");
}

#[test]
fn an_agent_page_starts_empty_so_its_first_board_stands_alone() {
    // #270: `add_page` over MCP used to bring the canvas's blank 1200x800
    // starter, and the agent's own board landed on top of it.
    let mut state = seeded_state();
    call(
        &mut state,
        "add_page",
        serde_json::json!({"name":"Variants"}),
    );
    assert!(state.active_children().is_empty(), "{:?}", roots(&state));
    call(
        &mut state,
        "batch_design",
        serde_json::json!({"operations": r##"b=I(document,{"type":"frame","name":"Board","x":0,"y":0,"width":880,"height":1083})"##}),
    );
    assert_eq!(roots(&state), vec![("Board".into(), 0.0, 0.0)]);
    call(
        &mut state,
        "batch_design",
        serde_json::json!({"operations": r##"b=I(document,{"type":"frame","name":"Second","x":0,"y":0,"width":400,"height":300})"##}),
    );
    assert_eq!(
        roots(&state),
        vec![("Second".into(), 960.0, 0.0), ("Board".into(), 0.0, 0.0)]
    );
}
