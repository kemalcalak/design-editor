//! "Create component from similar layers" (#369): the row shows only when
//! there are copies, and its dispatch converts them in one undo step.

use de_editor_core::editor_ui_state::{LayerContextMenuState, LayerContextTarget};
use de_editor_core::{EditorState, NodeId};
use jian_ops_schema::node::PenNode;
use serde_json::json;

use super::{LayerContextAction, LayerContextMenu};
use crate::widgets::layer_context_flow::apply_layer_context_action;

fn row(id: &str, label: &str) -> serde_json::Value {
    json!({"type": "frame", "id": id, "layout": "horizontal", "width": 200, "height": 32,
        "children": [
            {"type": "text", "id": format!("{id}-t"), "content": label, "fontSize": 14},
            {"type": "frame", "id": format!("{id}-dot"), "width": 8, "height": 8}]})
}

pub(super) fn state() -> EditorState {
    let mut s = EditorState::new();
    s.doc.pages = None;
    s.doc.children = [
        json!({"type": "frame", "id": "list", "layout": "vertical", "width": 200,
               "height": "fit_content",
               "children": [row("a", "A"), row("b", "B"), row("c", "C")]}),
        json!({"type": "frame", "id": "lone", "width": 10, "height": 10}),
    ]
    .into_iter()
    .map(|value| serde_json::from_value(value).expect("node"))
    .collect();
    s
}

pub(super) fn menu_for(s: &EditorState, id: &str) -> LayerContextMenu {
    LayerContextMenu::for_state(
        s,
        LayerContextMenuState {
            target: LayerContextTarget::Layer(NodeId::new(id)),
            anchor_x: 0.0,
            anchor_y: 0.0,
            menu: Default::default(),
        },
    )
}

pub(super) fn offers(menu: &LayerContextMenu) -> bool {
    menu.rows
        .iter()
        .any(|row| row.action == LayerContextAction::CreateComponentFromSimilar)
}

#[test]
fn the_row_shows_only_for_a_frame_with_copies() {
    let s = state();
    assert!(offers(&menu_for(&s, "b")));
    assert!(!offers(&menu_for(&s, "lone")));
    assert!(!offers(&menu_for(&s, "b-t")));
}

#[test]
fn the_row_converts_the_copies_with_the_clicked_frame_as_master_source() {
    let mut s = state();
    let depth = s.history.past.len();
    let mut next_id = 1000;
    apply_layer_context_action(
        &mut s,
        &mut next_id,
        LayerContextAction::CreateComponentFromSimilar,
        LayerContextTarget::Layer(NodeId::new("b")),
        0,
    );
    assert_eq!(s.history.past.len(), depth + 1, "one undo step");
    let pages = s
        .doc
        .pages
        .as_ref()
        .expect("the Components page made pages");
    let PenNode::Frame(list) = &pages[0].children[0] else {
        panic!("list")
    };
    let targets: Vec<&str> = list
        .children
        .as_ref()
        .unwrap()
        .iter()
        .map(|node| match node {
            PenNode::Ref(instance) => instance.target.as_str(),
            _ => panic!("every row is an instance"),
        })
        .collect();
    assert!(targets.iter().all(|target| *target == targets[0]));
    // The master is a copy of "b" (unnamed, so "Component"): its text says B.
    let master = s
        .components
        .find_by_id(&NodeId::new(targets[0]))
        .expect("registered");
    assert_eq!(master.name, "Component");
    let root = s
        .components
        .resolved_root(&s.doc, &master.id)
        .expect("master node");
    let master_json = serde_json::to_value(root).unwrap();
    assert_eq!(master_json["children"][0]["content"], "B");
    assert!(!offers(&menu_for(&s, "lone")));
    let toast = s.editor_ui.editor_toast.as_ref().expect("a toast says it");
    assert_eq!(toast.i18n_key, "convertInstances.done");
    assert_eq!(toast.args, vec![("converted".to_string(), "3".to_string())]);
}

#[test]
fn copies_that_differ_are_named_in_the_toast() {
    let mut s = state();
    // "c" holds a link to its own inner node, so it stays a frame.
    let Some(PenNode::Frame(list)) = s.doc.children.first_mut() else {
        panic!("list")
    };
    let rows = list.children.as_mut().unwrap();
    rows[2] = serde_json::from_value(json!({"type": "frame", "id": "c", "layout": "horizontal",
        "width": 200, "height": 32,
        "events": {"onTap": [{"scroll_to": {"target": "c-dot"}}]},
        "children": [
            {"type": "text", "id": "c-t", "content": "C", "fontSize": 14},
            {"type": "frame", "id": "c-dot", "width": 8, "height": 8}]}))
    .unwrap();
    apply_layer_context_action(
        &mut s,
        &mut 1000,
        LayerContextAction::CreateComponentFromSimilar,
        LayerContextTarget::Layer(NodeId::new("a")),
        0,
    );
    let toast = s.editor_ui.editor_toast.as_ref().expect("toast");
    assert_eq!(toast.i18n_key, "convertInstances.partial");
    assert!(toast
        .args
        .contains(&("skipped".to_string(), "1".to_string())));
}
