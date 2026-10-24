use de_editor_core::ref_resolve::resolve_refs_for_canvas;
use de_editor_core::{EditorState, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use crate::batch_program_test_support::{binding_id, call_operations};

fn state() -> EditorState {
    let doc = json!({"version":"1.0.0","children":[
        {"type":"frame","id":"card","name":"Card","reusable":true,
         "width":200,"height":120,"children":[
            {"type":"text","id":"title","content":"Hello"},
            {"type":"frame","id":"slot","name":"Content","children":[
                {"type":"text","id":"placeholder","content":"Drop here"}]}]},
        {"type":"frame","id":"button","name":"Button","reusable":true,
         "width":120,"height":40,"children":[{"type":"text","id":"label","content":"OK"}]},
        {"type":"frame","id":"toolbar","name":"Toolbar","reusable":true,
         "width":300,"height":48,"children":[{"type":"ref","id":"save","ref":"button"}]},
        {"type":"frame","id":"page","width":800,"height":600,"children":[
            {"type":"ref","id":"inst","ref":"card","x":0,"y":0},
            {"type":"ref","id":"bar","ref":"toolbar","x":0,"y":200}]}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn run(state: &mut EditorState, operations: &str) -> Value {
    let (envelope, command) = call_operations(state, operations);
    assert!(envelope.get("errors").is_none(), "{envelope}");
    assert!(
        state.apply(command.expect("a command")),
        "the batch applies"
    );
    envelope
}

fn rendered<'a>(resolved: &'a [PenNode], id: &str) -> &'a PenNode {
    de_editor_core::walkers::find_node(resolved, &NodeId::new(id))
        .unwrap_or_else(|| panic!("{id} renders"))
}

fn content(node: &PenNode) -> Value {
    serde_json::to_value(node).expect("json")["content"].clone()
}

#[test]
fn update_reaches_master_children_and_nested_instances() {
    let mut state = state();
    run(
        &mut state,
        concat!(
            "U(\"inst/title\", {\"content\":\"Account\"})\n",
            "U(\"bar/save/label\", {\"content\":\"Store\"})"
        ),
    );
    let resolved = resolve_refs_for_canvas(&state.doc).children;
    assert_eq!(content(rendered(&resolved, "inst__title")), "Account");
    assert_eq!(content(rendered(&resolved, "bar__save__label")), "Store");
}

#[test]
fn replace_a_slot_then_insert_into_the_replacement() {
    let mut state = state();
    let envelope = run(
        &mut state,
        concat!(
            "card=I(\"page\", {\"type\":\"ref\",\"ref\":\"card\"})\n",
            "content=R(card+\"/slot\", {\"type\":\"frame\",\"name\":\"Custom\",\"layout\":\"vertical\"})\n",
            "item=I(content, {\"type\":\"text\",\"content\":\"Item 1\"})\n",
            "U(item, {\"content\":\"Item one\"})"
        ),
    );
    let card = binding_id(&envelope, "card");
    let content_path = binding_id(&envelope, "content");
    let item_path = binding_id(&envelope, "item");
    assert!(content_path.starts_with(&format!("{card}/")));
    let item_id = item_path.rsplit('/').next().unwrap();
    let resolved = resolve_refs_for_canvas(&state.doc).children;
    let item = rendered(&resolved, &format!("{card}__{item_id}"));
    assert_eq!(content(item), "Item one");
    let Some(PenNode::Ref(reference)) =
        de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(&card))
    else {
        panic!("the card stays an instance");
    };
    let descendants = reference.descendants.as_ref().expect("descendants");
    assert_eq!(descendants.len(), 1, "everything lives in the slot's entry");
    assert_eq!(descendants["slot"]["type"], "frame");
}

#[test]
fn insert_fills_a_slot_of_a_nested_path() {
    let mut state = state();
    run(
        &mut state,
        "a=I(\"inst/slot\", {\"type\":\"text\",\"content\":\"Mine\"})",
    );
    let resolved = resolve_refs_for_canvas(&state.doc).children;
    let slot = rendered(&resolved, "inst__slot");
    let kids = slot.children().expect("slot children");
    assert_eq!(kids.len(), 1, "the placeholder is replaced");
    assert_eq!(content(&kids[0]), "Mine");
}

#[test]
fn an_unknown_instance_path_fails_the_line() {
    let state = state();
    let (envelope, command) = call_operations(&state, "U(\"inst/nope\", {\"content\":\"x\"})");
    assert!(command.is_none());
    assert!(
        envelope["errors"].to_string().contains("inst/nope"),
        "{envelope}"
    );
}

#[test]
fn batch_get_reads_an_instance_descendant_by_path() {
    use crate::{batch_get_snapshot, McpTool, ToolOutcome};
    let mut state = state();
    run(&mut state, "U(\"bar/save/label\", {\"content\":\"Store\"})");
    let tool = batch_get_snapshot(&state);
    let args = std::collections::BTreeMap::from([(
        "nodeIds".to_string(),
        "[\"bar/save/label\",\"page/inst/title\"]".to_string(),
    )]);
    let ToolOutcome::OkJson(json) = tool.call(&args) else {
        panic!("batch_get answers");
    };
    let envelope: Value = serde_json::from_str(&json).expect("json");
    let nodes = envelope["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), 2, "{envelope}");
    assert_eq!(nodes[0]["id"], "bar/save/label");
    assert_eq!(nodes[0]["content"], "Store");
    assert_eq!(nodes[1]["content"], "Hello");
}

#[test]
fn update_node_patches_an_instance_descendant() {
    use crate::{update_node_snapshot, McpTool, ToolOutcome};
    let mut state = state();
    let args = std::collections::BTreeMap::from([
        ("node_id".to_string(), "inst/title".to_string()),
        ("data".to_string(), "{\"content\":\"Account\"}".to_string()),
        ("fill_hex".to_string(), "#ff0000".to_string()),
    ]);
    let ToolOutcome::OkWithCommand(_, command) = update_node_snapshot().call(&args) else {
        panic!("update_node emits a command");
    };
    assert!(state.apply(command));
    let resolved = resolve_refs_for_canvas(&state.doc).children;
    let title = rendered(&resolved, "inst__title");
    assert_eq!(content(title), "Account");
    assert_eq!(
        de_editor_core::fills::first_solid_fill_hex(title),
        Some("#ff0000")
    );
}
