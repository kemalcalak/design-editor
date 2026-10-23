//! Desktop canvas boolean ops commit through
//! `EditorCommand::CommitBooleanResult` (#113): one undo step, the id
//! from the host counter, and the command's refusals.

use super::WidgetHostNative;
use de_editor_core::{BooleanOp, NodeId, PenNodeExt};

fn host_with(json: &str, selected: &[&str]) -> WidgetHostNative {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
    host.editor_state_mut().selection.set = selected.iter().map(|id| NodeId::new(*id)).collect();
    host.editor_state_mut().selection.anchor = NodeId::new(selected[0]);
    host
}

fn ids(host: &WidgetHostNative) -> Vec<String> {
    host.editor_state()
        .active_children()
        .iter()
        .map(|node| node.id_str().to_string())
        .collect()
}

const OVERLAPPING: &str = r#"{"version":"1.0.0","children":[
  {"type":"rectangle","id":"a","name":"a","x":0,"y":0,"width":40,"height":40},
  {"type":"rectangle","id":"b","name":"b","x":20,"y":0,"width":40,"height":40}
]}"#;

#[test]
fn canvas_boolean_is_one_undo_step_with_the_host_counter_id() {
    let mut host = host_with(OVERLAPPING, &["a", "b"]);
    host.next_node_id = 50;
    let depth = host.editor_state().history.past.len();

    assert!(host.apply_boolean_op(BooleanOp::Union));

    assert_eq!(ids(&host), ["n50"]);
    assert_eq!(host.next_node_id, 51, "counter advances past the new id");
    assert_eq!(host.editor_state().selection.set, vec![NodeId::new("n50")]);
    let result = &host.editor_state().active_children()[0];
    assert_eq!(result.base().name.as_deref(), Some("Boolean Result"));
    assert_eq!(host.editor_state().history.past.len(), depth + 1);

    assert!(host.editor_state_mut().undo());
    assert_eq!(ids(&host), ["a", "b"], "one undo restores both sources");
    assert_eq!(host.editor_state().history.past.len(), depth);
    assert!(host.editor_state_mut().redo());
    assert_eq!(ids(&host), ["n50"]);
}

#[test]
fn canvas_boolean_refuses_a_frame_with_children() {
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

    assert!(!host.apply_boolean_op(BooleanOp::Union));

    assert_eq!(host.editor_state().doc, before, "frame keeps its child");
    assert!(host.editor_state().history.past.is_empty());
    assert_eq!(host.next_node_id, 50, "no id minted");
}
