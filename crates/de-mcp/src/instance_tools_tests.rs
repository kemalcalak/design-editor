//! `instantiate_component` / `detach_instance` / `swap_instance` /
//! `list_instances` (#365).

use std::collections::BTreeMap;

use de_editor_core::{EditorCommand, EditorState, InstanceCommand, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;
use serde_json::Value;

use super::*;
use crate::test_fixtures::{frame, state_with, text};

/// Page 1: `screen`. Page 2 ("Kit"): masters `btn` (with `btn-label`)
/// and `alt`. Page 1 active.
fn state() -> EditorState {
    let mut s = state_with(vec![frame(
        "screen",
        "Screen",
        0.0,
        0.0,
        400.0,
        300.0,
        Vec::new(),
    )]);
    let label = text("btn-label", "Label", 8.0, 8.0, 80.0, 20.0, "Button");
    let btn = frame("btn", "Button", 0.0, 0.0, 120.0, 40.0, vec![label]);
    let alt = frame("alt", "Alt", 200.0, 0.0, 120.0, 40.0, Vec::new());
    s.add_page_with_name_and_children(Some("Kit".into()), Some(vec![btn, alt]))
        .expect("kit page");
    for (id, name) in [("btn", "Button"), ("alt", "Alt")] {
        assert!(s.apply(EditorCommand::CreateComponent {
            node_id: NodeId::new(id),
            name: name.into(),
        }));
    }
    assert!(s.apply(EditorCommand::SetActivePage { index: 0 }));
    s
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// The JSON result and the command of a write tool.
fn write(outcome: ToolOutcome) -> (Value, EditorCommand) {
    match outcome {
        ToolOutcome::OkJsonWithCommand(json, command) => {
            (serde_json::from_str(&json).expect("json"), command)
        }
        other => panic!("expected a write, got {other:?}"),
    }
}

fn error(outcome: ToolOutcome) -> String {
    match outcome {
        ToolOutcome::Err(_, message) => message,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn page_children(s: &EditorState, index: usize) -> &[PenNode] {
    &s.doc.pages.as_ref().unwrap()[index].children
}

#[test]
fn instantiate_inserts_a_linked_instance_at_the_predicted_id() {
    let mut s = state();
    let tool = instantiate_component_snapshot(&s);
    let (result, command) = write(tool.call(&args(&[
        ("component_id", "btn"),
        ("parent", "screen"),
        ("descendants", r#"{"btn-label":{"content":"Save"}}"#),
    ])));
    assert_eq!(result["linked"], true);
    assert_eq!(result["overridable"][0]["id"], "btn-label");
    assert_eq!(result["overridable"][0]["type"], "text");
    assert!(result["hint"].as_str().unwrap().contains("get_guidelines"));
    assert!(s.apply(command));
    let predicted = result["instanceId"].as_str().unwrap();
    let screen = de_editor_core::walkers::find_node(s.active_children(), &NodeId::new("screen"))
        .unwrap();
    let instance = &screen.children().unwrap()[0];
    assert_eq!(instance.id_str(), predicted);
    let PenNode::Ref(reference) = instance else {
        panic!("expected a ref")
    };
    assert_eq!(reference.target, "btn");
    assert_eq!(
        reference.descendants.as_ref().unwrap()["btn-label"]["content"],
        "Save"
    );
}

#[test]
fn instantiate_into_a_parent_on_another_page_keeps_the_active_page() {
    let mut s = state();
    assert!(s.apply(EditorCommand::InsertSubtree {
        nodes: vec![frame("other", "Other", 0.0, 0.0, 10.0, 10.0, Vec::new())],
        parent_id: NodeId::NONE,
        page_id: Some(s.doc.pages.as_ref().unwrap()[1].id.clone()),
    }));
    let (result, command) = write(
        instantiate_component_snapshot(&s)
            .call(&args(&[("component_id", "btn"), ("parent", "other")])),
    );
    assert_eq!(result["pageId"], s.doc.pages.as_ref().unwrap()[1].id.as_str());
    assert!(s.apply(command));
    assert_eq!(s.ui.active_page_index, 0);
    let other = de_editor_core::walkers::find_node(page_children(&s, 1), &NodeId::new("other"))
        .unwrap();
    assert_eq!(other.children().unwrap().len(), 1);
}

#[test]
fn instantiate_refuses_cycles_unknown_masters_and_bad_descendants() {
    let s = state();
    let tool = instantiate_component_snapshot(&s);
    assert!(error(tool.call(&args(&[("component_id", "btn"), ("parent", "btn")])))
        .contains("contain itself"));
    assert!(error(tool.call(&args(&[("component_id", "btn"), ("parent", "btn-label")])))
        .contains("cannot hold children"));
    assert!(error(tool.call(&args(&[("component_id", "nope")]))).contains("not found"));
    assert!(error(tool.call(&args(&[("component_id", "screen")]))).contains("not a component"));
    assert!(error(tool.call(&args(&[("component_id", "btn"), ("descendants", "[1]")])))
        .contains("descendants"));
    assert!(error(tool.call(&args(&[]))).contains("component_id"));
}

#[test]
fn instantiate_detach_copies_like_before() {
    let mut s = state();
    let (result, command) = write(
        instantiate_component_snapshot(&s)
            .call(&args(&[("component_id", "btn"), ("detach", "true")])),
    );
    assert_eq!(result["linked"], false);
    assert!(matches!(
        command,
        EditorCommand::Instance(InstanceCommand::Copy { .. })
    ));
    assert!(s.apply(command));
    let copy = s.active_children().last().unwrap();
    assert!(matches!(copy, PenNode::Frame(_)));
    assert_eq!(copy.id_str(), result["nodeId"].as_str().unwrap());
}

#[test]
fn detach_instance_reports_the_ids_its_apply_gives() {
    let mut s = state();
    let (inserted, command) = write(
        instantiate_component_snapshot(&s)
            .call(&args(&[("component_id", "btn"), ("parent", "screen")])),
    );
    assert!(s.apply(command));
    let instance = inserted["instanceId"].as_str().unwrap().to_string();
    assert!(s.apply(EditorCommand::SetActivePage { index: 1 }));

    let (result, command) =
        write(detach_instance_snapshot(&s).call(&args(&[("nodeId", &instance)])));
    assert_eq!(result["nodeId"], instance.as_str());
    assert_eq!(result["componentId"], "btn");
    assert!(s.apply(command));
    let detached =
        de_editor_core::walkers::find_node(page_children(&s, 0), &NodeId::new(&instance))
            .expect("root keeps the id");
    let label = &detached.children().unwrap()[0];
    assert_eq!(result["nodes"][0]["id"], label.id_str());
    assert_eq!(result["nodes"][0]["name"], "Label");

    let again = detach_instance_snapshot(&s).call(&args(&[("nodeId", &instance)]));
    assert!(error(again).contains("not a component instance"));
}

#[test]
fn swap_and_list_instances() {
    let mut s = state();
    for _ in 0..2 {
        let (_, command) = write(
            instantiate_component_snapshot(&s)
                .call(&args(&[("component_id", "btn"), ("parent", "screen")])),
        );
        assert!(s.apply(command));
    }
    let listed = match list_instances_snapshot(&s).call(&args(&[("component_id", "btn")])) {
        ToolOutcome::OkJson(json) => serde_json::from_str::<Value>(&json).unwrap(),
        other => panic!("expected json, got {other:?}"),
    };
    assert_eq!(listed["count"], 2);
    assert_eq!(listed["name"], "Button");
    let first = listed["instances"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(
        listed["instances"][0]["pageId"],
        s.doc.pages.as_ref().unwrap()[0].id.as_str()
    );

    let (result, command) = write(
        swap_instance_snapshot(&s).call(&args(&[("nodeId", &first), ("component_id", "alt")])),
    );
    assert_eq!(result["previousComponentId"], "btn");
    assert!(s.apply(command));
    let tool = list_instances_snapshot(&s);
    for (component, count) in [("btn", 1), ("alt", 1)] {
        let ToolOutcome::OkJson(json) = tool.call(&args(&[("component_id", component)])) else {
            panic!("json")
        };
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap()["count"], count);
    }
    assert!(error(
        swap_instance_snapshot(&s).call(&args(&[("nodeId", &first), ("component_id", "alt")]))
    )
    .contains("already"));
    assert!(error(list_instances_snapshot(&s).call(&args(&[("component_id", "nope")])))
        .contains("not found"));
}
