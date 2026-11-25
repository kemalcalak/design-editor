//! Component properties over MCP (#367): declare / rename / remove, the
//! `get_component` / `list_components` reads, `props` writes through
//! `batch_design` and `update_node` with validation, the compact
//! `batch_get` read, and the size of a 20-button screen.

use std::collections::BTreeMap;

use de_editor_core::ref_resolve::resolve_refs_for_canvas;
use de_editor_core::{EditorCommand, EditorState, NodeId};
use serde_json::{json, Value};

use crate::batch_program_test_support::{binding_id, call_operations};
use crate::{
    batch_get_snapshot, get_component_snapshot, list_components_snapshot,
    remove_component_property_snapshot, update_node_snapshot_for,
    upsert_component_property_snapshot, McpTool, ToolOutcome,
};

/// `Screen` (active) holds `b1`; the button master `btn` sits on the
/// `Components` page.
fn state() -> EditorState {
    let doc = json!({"version":"1.0.0","children":[],"pages":[
        {"id":"p1","name":"Screen","children":[
            {"type":"frame","id":"screen","width":800,"height":600,"children":[
                {"type":"ref","id":"b1","ref":"btn","props":{"label":"Save"}}]}]},
        {"id":"p2","name":"Components","children":[
            {"type":"frame","id":"icon-a","reusable":true,"width":16,"height":16},
            {"type":"frame","id":"btn","name":"Button","reusable":true,"width":120,"height":40,
             "componentProperties":{
               "label":{"type":"text","default":"Button",
                        "bind":[{"node":"btn-label","field":"content"}]}},
             "children":[
               {"type":"text","id":"btn-label","content":"Button"},
               {"type":"ref","id":"btn-icon","ref":"icon-a"}]}]}
    ]});
    EditorState::from_document(jian_ops_schema::load_str(&doc.to_string()).unwrap().value)
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn ok_command(outcome: ToolOutcome) -> (Value, EditorCommand) {
    match outcome {
        ToolOutcome::OkJsonWithCommand(json, command) => {
            (serde_json::from_str(&json).unwrap(), command)
        }
        ToolOutcome::OkWithCommand(map, command) => (json!(map), command),
        other => panic!("expected a command, got {other:?}"),
    }
}

fn error(outcome: ToolOutcome) -> String {
    match outcome {
        ToolOutcome::Err(_, message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn instance(state: &EditorState, id: &str) -> Value {
    let node =
        de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id)).unwrap();
    serde_json::to_value(node).unwrap()
}

fn rendered_label(state: &EditorState, instance: &str) -> Value {
    let resolved = resolve_refs_for_canvas(&state.doc);
    let pages = resolved.pages.unwrap();
    let node = de_editor_core::walkers::find_node(
        &pages[0].children,
        &NodeId::new(format!("{instance}__btn-label")),
    )
    .unwrap();
    serde_json::to_value(node).unwrap()["content"].clone()
}

#[test]
fn upsert_declares_a_property_and_reads_report_it() {
    let mut state = state();
    let (result, command) = ok_command(upsert_component_property_snapshot(&state).call(&args(&[
        ("component_id", "btn"),
        ("name", "showIcon"),
        ("type", "boolean"),
        ("bind", r#"[{"node":"btn-icon","field":"visible"}]"#),
    ])));
    assert_eq!(result["default"], true, "takes the bound node's value");
    assert!(state.apply(command));

    let listed = match list_components_snapshot(&state).call(&BTreeMap::new()) {
        ToolOutcome::Ok(map) => map["components"].clone(),
        other => panic!("{other:?}"),
    };
    assert!(
        listed.contains("Button|btn|label:text,showIcon:boolean"),
        "{listed}"
    );
    assert!(listed.contains("icon-a|icon-a") && !listed.contains("icon-a|icon-a|"));

    let detail = match get_component_snapshot(&state).call(&args(&[("component_id", "btn")])) {
        ToolOutcome::Ok(map) => map,
        other => panic!("{other:?}"),
    };
    let properties: Value = serde_json::from_str(&detail["properties"]).unwrap();
    assert_eq!(
        properties,
        json!([
            {"name":"label","type":"text","default":"Button",
             "bind":[{"node":"btn-label","field":"content"}]},
            {"name":"showIcon","type":"boolean","default":true,
             "bind":[{"node":"btn-icon","field":"visible"}]}
        ])
    );
}

#[test]
fn upsert_refusals_explain_themselves() {
    let state = state();
    let tool = upsert_component_property_snapshot(&state);
    let missing_type = error(tool.call(&args(&[("component_id", "btn"), ("name", "fresh")])));
    assert!(missing_type.contains("give a type"), "{missing_type}");
    let bad_type = error(tool.call(&args(&[
        ("component_id", "btn"),
        ("name", "x"),
        ("type", "slot"),
    ])));
    assert!(
        bad_type.contains("allowed: text, boolean, instance, color"),
        "{bad_type}"
    );
    let outside = error(tool.call(&args(&[
        ("component_id", "btn"),
        ("name", "x"),
        ("type", "boolean"),
        ("bind", r#"[{"node":"screen","field":"visible"}]"#),
    ])));
    assert!(outside.contains("not inside component btn"), "{outside}");
    let no_master = error(tool.call(&args(&[
        ("component_id", "b1"),
        ("name", "x"),
        ("type", "text"),
    ])));
    assert!(no_master.contains("component b1 not found"), "{no_master}");
}

#[test]
fn rename_and_remove_keep_instances_looking_the_same() {
    let mut state = state();
    let (_, rename) = ok_command(upsert_component_property_snapshot(&state).call(&args(&[
        ("component_id", "btn"),
        ("name", "label"),
        ("newName", "caption"),
    ])));
    assert!(state.apply(rename));
    assert_eq!(instance(&state, "b1")["props"], json!({"caption": "Save"}));
    assert_eq!(rendered_label(&state, "b1"), "Save");

    let (result, remove) = ok_command(
        remove_component_property_snapshot(&state)
            .call(&args(&[("component_id", "btn"), ("name", "caption")])),
    );
    assert_eq!(result["instancesKeptAsOverrides"], 1);
    assert!(state.apply(remove));
    assert_eq!(
        instance(&state, "b1")["descendants"]["btn-label"]["content"],
        "Save"
    );
    assert_eq!(rendered_label(&state, "b1"), "Save");
}

#[test]
fn batch_design_insert_checks_props() {
    let mut state = state();
    let (envelope, command) = call_operations(
        &state,
        "b=I(\"screen\", {\"type\":\"ref\",\"ref\":\"btn\",\"props\":{\"label\":\"Send\"}})",
    );
    assert!(envelope.get("errors").is_none(), "{envelope}");
    assert!(state.apply(command.unwrap()));
    let id = binding_id(&envelope, "b");
    assert_eq!(rendered_label(&state, &id), "Send");

    let (envelope, command) = call_operations(
        &state,
        "b=I(\"screen\", {\"type\":\"ref\",\"ref\":\"btn\",\"props\":{\"labl\":\"Send\"}})",
    );
    assert!(command.is_none());
    let errors = envelope["errors"].to_string();
    assert!(
        errors.contains("no property \\\"labl\\\"") && errors.contains("label (text)"),
        "{errors}"
    );

    let (envelope, _) = call_operations(
        &state,
        "b=I(\"screen\", {\"type\":\"ref\",\"ref\":\"btn\",\"props\":{\"label\":true}})",
    );
    assert!(
        envelope["errors"]
            .to_string()
            .contains("takes a text value, got a boolean"),
        "{envelope}"
    );
}

#[test]
fn batch_design_update_merges_props() {
    let mut state = state();
    let (envelope, command) = call_operations(
        &state,
        "U(\"b1\", {\"props\":{\"label\":\"Store\"},\"x\":10})",
    );
    assert!(envelope.get("errors").is_none(), "{envelope}");
    assert!(state.apply(command.unwrap()));
    assert_eq!(instance(&state, "b1")["props"], json!({"label": "Store"}));
    assert_eq!(instance(&state, "b1")["x"], 10.0);
    assert_eq!(rendered_label(&state, "b1"), "Store");
}

#[test]
fn update_node_props_merge_and_validate() {
    let mut state = state();
    let tool = update_node_snapshot_for(&state);
    let (_, command) = ok_command(tool.call(&args(&[
        ("nodeId", "b1"),
        ("data", r#"{"props":{"label":"Keep"},"name":"Primary"}"#),
    ])));
    assert!(state.apply(command));
    assert_eq!(instance(&state, "b1")["props"], json!({"label": "Keep"}));
    assert_eq!(instance(&state, "b1")["name"], "Primary");

    let tool = update_node_snapshot_for(&state);
    let unknown = error(tool.call(&args(&[
        ("nodeId", "b1"),
        ("data", r##"{"props":{"tone":"#fff"}}"##),
    ])));
    assert!(unknown.contains("allowed: label (text)"), "{unknown}");
    let (_, clear) = ok_command(tool.call(&args(&[
        ("nodeId", "b1"),
        ("data", r#"{"props":{"label":null}}"#),
    ])));
    assert!(state.apply(clear));
    assert!(instance(&state, "b1").get("props").is_none());
}

#[test]
fn batch_get_on_an_instance_returns_ref_and_props() {
    let state = state();
    let outcome = batch_get_snapshot(&state).call(&args(&[("nodeIds", r#"["b1"]"#)]));
    let ToolOutcome::OkJson(json) = outcome else {
        panic!("{outcome:?}");
    };
    let value: Value = serde_json::from_str(&json).unwrap();
    let node = &value["nodes"][0];
    assert_eq!(node["type"], "ref");
    assert_eq!(node["ref"], "btn");
    assert_eq!(node["props"], json!({"label": "Save"}));
    assert!(
        node.get("children").is_none(),
        "not the expanded tree: {node}"
    );
}

/// The issue asks for the token size of a 20-button screen before and
/// after: instances written with `descendants` keyed by master node ids
/// against the same instances written with `props`. Bytes / 4 is the
/// usual token estimate; the PR records the printed numbers.
#[test]
fn twenty_button_screen_is_smaller_with_props() {
    let labels: Vec<String> = (1..=20).map(|i| format!("Action {i}")).collect();
    let screen = |instance: &dyn Fn(usize, &str) -> Value| {
        let children: Vec<Value> = labels
            .iter()
            .enumerate()
            .map(|(i, l)| instance(i, l))
            .collect();
        json!({"type":"frame","id":"screen","width":800,"height":600,"children":children})
    };
    let before = screen(&|i, label| {
        json!({"type":"ref","id":format!("b{i}"),"ref":"btn",
               "descendants":{"btn-label":{"content":label},"btn-icon":{"visible":i % 2 == 0},
                              "btn-bg":{"fill":[{"type":"solid","color":"$primary"}]}}})
    });
    let after = screen(&|i, label| {
        json!({"type":"ref","id":format!("b{i}"),"ref":"btn",
               "props":{"label":label,"showIcon":i % 2 == 0,"tone":"$primary"}})
    });
    let write_before: String = labels
        .iter()
        .enumerate()
        .map(|(i, l)| format!(
            "b{i}=I(\"screen\",{{\"type\":\"ref\",\"ref\":\"btn\",\"descendants\":{{\"btn-label\":{{\"content\":\"{l}\"}},\"btn-icon\":{{\"visible\":{}}},\"btn-bg\":{{\"fill\":[{{\"type\":\"solid\",\"color\":\"$primary\"}}]}}}}}})\n",
            i % 2 == 0
        ))
        .collect();
    let write_after: String = labels
        .iter()
        .enumerate()
        .map(|(i, l)| format!(
            "b{i}=I(\"screen\",{{\"type\":\"ref\",\"ref\":\"btn\",\"props\":{{\"label\":\"{l}\",\"showIcon\":{},\"tone\":\"$primary\"}}}})\n",
            i % 2 == 0
        ))
        .collect();
    let read_before = before.to_string().len();
    let read_after = after.to_string().len();
    eprintln!(
        "20-button screen: read {read_before} -> {read_after} bytes (~{} -> ~{} tokens); \
         write {} -> {} bytes (~{} -> ~{} tokens)",
        read_before / 4,
        read_after / 4,
        write_before.len(),
        write_after.len(),
        write_before.len() / 4,
        write_after.len() / 4,
    );
    assert!(
        read_after * 10 < read_before * 7,
        "props cut the read by > 30%"
    );
    assert!(write_after.len() < write_before.len());
}
