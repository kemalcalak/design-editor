//! Desktop canvas path operations (#388): a boolean makes an editable
//! boolean group, Flatten (Cmd+E) bakes it with curves kept, and the
//! command's refusals still hold. Ids come from the host counter.

use super::WidgetHostNative;
use de_editor_core::path_ops::PathOperation;
use de_editor_core::{BooleanOp, NodeId, PenNodeExt};
use jian_ops_schema::node::PenNode;

fn host_with(json: &str, selected: &[&str]) -> WidgetHostNative {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
    select(&mut host, selected);
    host
}

fn select(host: &mut WidgetHostNative, ids: &[&str]) {
    host.editor_state_mut().selection.set = ids.iter().map(|id| NodeId::new(*id)).collect();
    host.editor_state_mut().selection.anchor = NodeId::new(ids[0]);
}

fn ids(host: &WidgetHostNative) -> Vec<String> {
    host.editor_state()
        .active_children()
        .iter()
        .map(|node| node.id_str().to_string())
        .collect()
}

const CIRCLES: &str = r#"{"version":"1.0.0","children":[
  {"type":"ellipse","id":"a","name":"a","x":0,"y":0,"width":100,"height":100},
  {"type":"ellipse","id":"b","name":"b","x":60,"y":0,"width":100,"height":100}
]}"#;

#[test]
fn canvas_boolean_makes_a_boolean_group_with_the_host_counter_id() {
    let mut host = host_with(CIRCLES, &["b", "a"]);
    host.next_node_id = 50;
    let depth = host.editor_state().history.past.len();

    assert!(host.apply_boolean_op(BooleanOp::Union));

    assert_eq!(ids(&host), ["n50"]);
    assert_eq!(host.next_node_id, 51, "counter advances past the new id");
    assert_eq!(host.editor_state().selection.set, vec![NodeId::new("n50")]);
    let group = &host.editor_state().active_children()[0];
    assert!(matches!(group, PenNode::Boolean(_)));
    let kids: Vec<&str> = group
        .children()
        .unwrap()
        .iter()
        .map(|c| c.id_str())
        .collect();
    assert_eq!(kids, ["a", "b"], "sources stay editable, in paint order");
    assert_eq!(host.editor_state().history.past.len(), depth + 1);

    // The scene paints the computed path, curves kept.
    host.refresh_layout_scene();
    let page = host.layout_scene.active_page().unwrap();
    let node = page.find("n50").unwrap();
    let d = node.svg_path.as_deref().expect("computed outline");
    assert!(d.matches('C').count() <= 8 && !d.contains('L'), "{d}");

    // The same shortcut on the selected group switches its op.
    assert!(host.apply_boolean_op(BooleanOp::Subtract));
    assert_eq!(
        host.editor_state().boolean_op_of(&NodeId::new("n50")),
        Some(BooleanOp::Subtract)
    );

    assert!(host.editor_state_mut().undo());
    assert!(host.editor_state_mut().undo());
    assert_eq!(ids(&host), ["a", "b"], "undo restores both sources");
}

#[test]
fn flatten_bakes_a_boolean_group_into_one_curved_path() {
    let mut host = host_with(CIRCLES, &["a", "b"]);
    host.next_node_id = 50;
    assert!(host.apply_boolean_op(BooleanOp::Union));
    assert!(host.apply_path_operation(PathOperation::Flatten));
    assert_eq!(ids(&host), ["n51"]);
    let PenNode::Path(path) = &host.editor_state().active_children()[0] else {
        panic!("flatten leaves a path")
    };
    assert_eq!(path.base.name.as_deref(), Some("Union"));
    let d = path.d.as_deref().unwrap();
    assert!(d.matches('C').count() <= 8, "{d}");
}

#[test]
fn flatten_refuses_a_frame_with_children() {
    let mut host = host_with(
        r#"{"version":"1.0.0","children":[
          {"type":"frame","id":"f","name":"f","x":0,"y":0,"width":40,"height":40,
           "children":[{"type":"rectangle","id":"inner","x":0,"y":0,"width":5,"height":5}]},
          {"type":"rectangle","id":"b","name":"b","x":20,"y":0,"width":40,"height":40}
        ]}"#,
        &["f", "b"],
    );
    host.next_node_id = 50;
    let before = host.editor_state().doc.clone();

    assert!(
        host.apply_path_operation(PathOperation::Flatten),
        "handled: the notice repaints"
    );

    assert_eq!(host.editor_state().doc, before, "frame keeps its child");
    assert!(host.editor_state().history.past.is_empty());
    assert_eq!(host.next_node_id, 50, "no id minted");
    let toast = host
        .editor_state()
        .editor_ui
        .visible_toast(host.now_ms)
        .expect("the refusal raises a notice");
    assert_eq!(
        toast.i18n_key,
        de_editor_core::host_support::BOOLEAN_CONTAINER_TOAST_KEY
    );
}

#[test]
fn outline_stroke_of_a_stroked_line_replaces_it() {
    let mut host = host_with(
        r##"{"version":"1.0.0","children":[
          {"type":"line","id":"l","x":0,"y":10,"width":50,"height":0,
           "stroke":{"thickness":4,"cap":"round","fill":[{"type":"solid","color":"#112233"}]}}]}"##,
        &["l"],
    );
    host.next_node_id = 50;
    assert!(host.apply_path_operation(PathOperation::OutlineStroke));
    let PenNode::Path(path) = &host.editor_state().active_children()[0] else {
        panic!()
    };
    assert!(path.stroke.is_none());
    assert_eq!(path.base.x, Some(-2.0), "round cap reaches past the end");
}
