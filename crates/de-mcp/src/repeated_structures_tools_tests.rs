use super::*;

use jian_ops_schema::node::PenNode;
use serde_json::json;

fn row(id: &str, label: &str) -> Value {
    json!({"type":"frame","id":id,"name":format!("Row {id}"),"layout":"horizontal","gap":8,
        "width":"fill_container","height":32,"fill":[{"type":"solid","color":"#ffffff"}],
        "children":[
            {"type":"text","id":format!("{id}-label"),"content":label,"fontSize":14,"width":120,"height":20},
            {"type":"frame","id":format!("{id}-dot"),"width":8,"height":8,
             "fill":[{"type":"solid","color":"#22aa55"}]}]})
}

/// Page `home` holds a list of three rows; page `settings` one more row.
fn state() -> EditorState {
    let doc = json!({"version":"1.0.0","children":[],"pages":[
        {"id":"home","name":"Home","children":[
            {"type":"frame","id":"list","layout":"vertical","width":300,"height":"fit_content",
             "children":[row("a","Alpha"),row("b","Beta"),row("c","Gamma")]}]},
        {"id":"settings","name":"Settings","children":[row("d","Delta")]}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn find(state: &EditorState, pairs: &[(&str, &str)]) -> Value {
    match find_repeated_structures_snapshot(state).call(&args(pairs)) {
        ToolOutcome::OkJson(json) => serde_json::from_str(&json).expect("json"),
        other => panic!("find failed: {other:?}"),
    }
}

fn convert(state: &EditorState, pairs: &[(&str, &str)]) -> ToolOutcome {
    convert_to_instances_snapshot(state).call(&args(pairs))
}

#[test]
fn find_reports_groups_across_pages_or_on_one() {
    let state = state();
    let all = find(&state, &[]);
    assert_eq!(all["count"], 1);
    let group = &all["groups"][0];
    assert_eq!(group["nodeIds"], json!(["a", "b", "c", "d"]));
    assert_eq!(
        group["pageIds"],
        json!(["home", "home", "home", "settings"])
    );
    assert_eq!(group["nodesPerMember"], 3);
    assert_eq!(group["convertible"], 4);
    assert!(
        group["estimatedTokenSaving"].as_u64().unwrap() > 0,
        "{group}"
    );

    let settings = find(&state, &[("pageId", "settings")]);
    assert_eq!(settings["count"], 0);
    let home = find(&state, &[("pageId", "0")]);
    assert_eq!(home["groups"][0]["nodeIds"], json!(["a", "b", "c"]));
    assert_eq!(find(&state, &[("minNodes", "4")])["count"], 0);
}

#[test]
fn find_refuses_bad_arguments() {
    let state = state();
    for pairs in [
        [("pageId", "nope")],
        [("minNodes", "0")],
        [("minNodes", "2.5")],
    ] {
        assert!(matches!(
            find_repeated_structures_snapshot(&state).call(&args(&pairs)),
            ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
        ));
    }
}

#[test]
fn dry_run_plans_without_a_command() {
    let state = state();
    let ToolOutcome::OkJson(json) = convert(
        &state,
        &[("nodeIds", r#"["a","b","c"]"#), ("dryRun", "true")],
    ) else {
        panic!("dry run returns JSON only");
    };
    let result: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(result["dryRun"], true);
    assert_eq!(result["placement"], "components");
    assert_eq!(result["converted"].as_array().unwrap().len(), 3);
    assert_eq!(result["converted"][1], json!({"id": "b", "overrides": 1}));
    assert!(result["bytesAfter"].as_u64() < result["bytesBefore"].as_u64());
}

#[test]
fn group_id_converts_with_the_predicted_master_id() {
    let mut state = state();
    let group_id = find(&state, &[])["groups"][0]["groupId"]
        .as_str()
        .unwrap()
        .to_string();
    let ToolOutcome::OkJsonWithCommand(json, command) = convert(&state, &[("groupId", &group_id)])
    else {
        panic!("convert carries the command");
    };
    let result: Value = serde_json::from_str(&json).unwrap();
    let master_id = result["masterId"].as_str().unwrap().to_string();
    let depth = state.history.past.len();
    assert!(state.apply(command));
    assert_eq!(state.history.past.len(), depth + 1, "one undo step");
    assert!(state
        .components
        .find_by_id(&NodeId::new(&master_id))
        .is_some());
    let pages = state.doc.pages.as_ref().unwrap();
    let PenNode::Ref(d) = &pages[1].children[0] else {
        panic!("d on the second page is an instance")
    };
    assert_eq!(d.target, master_id);
    // Nothing repeats any more.
    assert_eq!(find(&state, &[])["count"], 0);
}

#[test]
fn differences_are_reported_and_in_place_keeps_the_master() {
    let mut state = state();
    let mut odd = row("x", "Odd");
    odd["children"][1]["fill"] = json!([{"type":"solid","color":"#ff0000"}]);
    let odd: PenNode = serde_json::from_value(odd).unwrap();
    state.doc.pages.as_mut().unwrap()[0].children.push(odd);
    let ToolOutcome::OkJsonWithCommand(json, command) = convert(
        &state,
        &[
            ("nodeIds", r#"["a","b","x"]"#),
            ("masterFrom", "b"),
            ("placement", "inPlace"),
        ],
    ) else {
        panic!("convert carries the command");
    };
    let result: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(result["masterId"], "b");
    assert_eq!(result["converted"], json!([{"id": "a", "overrides": 1}]));
    assert_eq!(result["skipped"][0]["id"], "x");
    assert!(result["skipped"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("x-dot"));
    assert!(state.apply(command));
    let page = &state.doc.pages.as_ref().unwrap()[0];
    assert!(matches!(&page.children[1], PenNode::Frame(f) if f.base.id == "x"));
}

#[test]
fn convert_refuses_bad_arguments() {
    let state = state();
    let expect_err = |pairs: &[(&str, &str)], needle: &str| match convert(&state, pairs) {
        ToolOutcome::Err(_, message) => assert!(message.contains(needle), "{message}"),
        other => panic!("expected an error for {pairs:?}, got {other:?}"),
    };
    expect_err(&[], "nodeIds or groupId is required");
    expect_err(
        &[("nodeIds", r#"["a","b"]"#), ("groupId", "rs-1")],
        "not both",
    );
    expect_err(&[("groupId", "rs-1")], "groupId not found");
    expect_err(&[("nodeIds", r#"["a"]"#)], "at least 2");
    expect_err(&[("nodeIds", r#"["a","list"]"#)], "inside listed node");
    expect_err(
        &[("nodeIds", r#"["a","b"]"#), ("placement", "here")],
        "placement",
    );
    expect_err(&[("nodeIds", r#"["a","b"]"#), ("dryRun", "yes")], "dryRun");
    expect_err(
        &[("nodeIds", r#"["a","b"]"#), ("masterFrom", "c")],
        "not one of the listed",
    );
}

/// `nodeIds` arrives as a JSON array over the wire (#430).
#[test]
fn convert_node_ids_parse_through_the_wire_parser() {
    let state = state();
    let line = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"convert_to_instances","arguments":{"nodeIds":["a","b","c"],"dryRun":true}}}"#;
    let call = crate::parse_tool_call(line).expect("structured arguments parse");
    let ToolOutcome::OkJson(json) = convert_to_instances_snapshot(&state).call(&call.arguments)
    else {
        panic!("dry run returns JSON only");
    };
    let result: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(result["converted"].as_array().unwrap().len(), 3);
}
