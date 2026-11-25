//! End-to-end coverage for the dynamic `insert_<component>` kit tools:
//! a name advertised by `tools/list` must also dispatch on `tools/call`.

use super::*;
use de_editor_core::PenNodeExt;

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

fn call_tool(state: &mut EditorState, id: u32, name: &str, arguments: &str) -> String {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{arguments}}}}}"#
    );
    let response =
        process_message_with_applier(state, &line, |_, state, cmd| state.apply(cmd.clone()))
            .expect("dispatch")
            .expect("response");
    tool_text(&response)
}

fn components_page(state: &EditorState) -> &[jian_ops_schema::node::PenNode] {
    state
        .doc
        .pages
        .iter()
        .flatten()
        .find(|page| page.name == de_editor_core::page_mutators::COMPONENTS_PAGE_NAME)
        .map(|page| page.children.as_slice())
        .unwrap_or_default()
}

/// #365: kit tools insert refs to one master on the Components page, and
/// root placement moves the instances, never the master.
#[test]
fn kit_tools_insert_linked_instances_of_one_unplaced_master() {
    let mut state = EditorState::new();
    for id in 1..=2 {
        let text = call_tool(&mut state, id, "insert_shadcn_btn_primary", "{}");
        assert!(text.contains(r#""linked":"true""#), "{text}");
    }
    let masters = components_page(&state);
    assert_eq!(masters.len(), 1);
    assert_eq!(masters[0].base().x, Some(0.0), "the master is not placed");
    let roots = state.active_children();
    assert_eq!(roots.len(), 2);
    for root in roots {
        let jian_ops_schema::node::PenNode::Ref(reference) = root else {
            panic!("expected a ref, got {root:?}")
        };
        assert_eq!(reference.target, masters[0].id_str());
    }
    assert_ne!(
        (roots[0].base().x, roots[0].base().y),
        (roots[1].base().x, roots[1].base().y),
        "placement keeps the two instances apart"
    );
    let text = call_tool(&mut state, 3, "insert_shadcn_btn_primary", r#"{"detach":true}"#);
    assert!(text.contains(r#""linked":"false""#), "{text}");
    assert!(matches!(
        state.active_children()[0],
        jian_ops_schema::node::PenNode::Frame(_)
    ));
}

/// #365 "done when": a kit-heavy screen is smaller linked than copied, in
/// the saved document and in `batch_get`. `--nocapture` prints the numbers.
#[test]
fn twenty_linked_kit_buttons_are_smaller_than_twenty_copies() {
    let measure = |detach: bool| {
        let mut state = EditorState::new();
        let screen = r#"{"script":"const s = I(null, {type:'frame',name:'Screen',width:800,height:1200,layout:'vertical'}); for (let i = 0; i < 20; i++) K('shadcn/btn-primary', s, {label:'Item ' + i, detach: DETACH});"}"#
            .replace("DETACH", if detach { "true" } else { "false" });
        let text = call_tool(&mut state, 1, "batch_design", &screen);
        assert!(!text.contains("\"errors\""), "{text}");
        let saved = serde_json::to_string(&state.doc).unwrap().len();
        let root = state.active_children()[0].id_str().to_string();
        let read = call_tool(
            &mut state,
            2,
            "batch_get",
            &format!(r#"{{"nodeIds":["{root}"],"readDepth":5}}"#),
        )
        .len();
        (saved, read)
    };
    let (linked_saved, linked_read) = measure(false);
    let (copied_saved, copied_read) = measure(true);
    eprintln!(
        "20 shadcn buttons: saved {linked_saved} B linked vs {copied_saved} B copied; batch_get {linked_read} B vs {copied_read} B"
    );
    assert!(linked_saved < copied_saved);
    assert!(linked_read < copied_read);
}

/// A master added to an existing Components page is not a new root to
/// place either.
#[test]
fn a_second_kit_master_keeps_its_slot_on_the_components_page() {
    let mut state = EditorState::new();
    call_tool(&mut state, 1, "insert_shadcn_btn_primary", "{}");
    call_tool(&mut state, 2, "insert_shadcn_btn_secondary", "{}");
    let masters = components_page(&state);
    assert_eq!(masters.len(), 2);
    let right = masters[0].base().x.unwrap() + masters[0].width_px().unwrap();
    assert_eq!(masters[1].base().x, Some(right + 40.0));
    assert_eq!(masters[1].base().y, Some(0.0));
    assert_eq!(state.ui.active_page_index, 0);
}
