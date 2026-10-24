use jian_ops_schema::node::{AnnotationKind, PenNode};

use crate::command_node::build_leaf_node;
use crate::test_support::state_with;
use crate::{EditorState, NodeId};

fn with_note() -> EditorState {
    let note = build_leaf_node("note", "n1", "Note", 10, 20, 240, 160).expect("note builds");
    let mut state = state_with(vec![note]);
    state.set_single_selection(NodeId::new("n1"));
    state
}

fn body(state: &EditorState) -> (AnnotationKind, jian_ops_schema::node::NoteNode) {
    let node = state.selected_node().expect("selected");
    let (kind, body) = node.annotation().expect("annotation");
    (kind, body.clone())
}

#[test]
fn a_note_builds_at_its_box_and_switches_kind_keeping_its_body() {
    let mut state = with_note();
    assert!(state.set_selected_annotation_content("Ship on Friday"));
    assert!(state.set_selected_annotation_kind(AnnotationKind::Prompt));
    assert!(!state.set_selected_annotation_kind(AnnotationKind::Prompt));
    let (kind, note) = body(&state);
    assert_eq!(kind, AnnotationKind::Prompt);
    assert_eq!(note.plain_content(), "Ship on Friday");
    assert_eq!(note.base.x, Some(10.0));
    assert!(matches!(state.selected_node(), Some(PenNode::Prompt(_))));
    let bounds = crate::geometry::aggregate_bounds(state.selected_node().unwrap());
    assert_eq!(
        (bounds.x, bounds.y, bounds.w, bounds.h),
        (10.0, 20.0, 240.0, 160.0)
    );
}

#[test]
fn the_model_is_trimmed_and_a_blank_one_clears_it() {
    let mut state = with_note();
    assert!(state.set_selected_annotation_model("  claude-opus  "));
    assert_eq!(body(&state).1.model.as_deref(), Some("claude-opus"));
    assert!(!state.set_selected_annotation_model("claude-opus"));
    assert!(state.set_selected_annotation_model(" "));
    assert_eq!(body(&state).1.model, None);
}

#[test]
fn unchanged_content_and_other_kinds_write_nothing() {
    let mut state = with_note();
    assert!(!state.set_selected_annotation_content(""));
    let rect = build_leaf_node("rect", "r1", "Rect", 0, 0, 10, 10).unwrap();
    let mut other = state_with(vec![rect]);
    other.set_single_selection(NodeId::new("r1"));
    assert!(!other.set_selected_annotation_content("x"));
    assert!(!other.set_selected_annotation_kind(AnnotationKind::Note));
    assert_eq!(other.selected_annotation_kind(), None);
}

#[test]
fn a_note_is_edited_inline_like_text() {
    let mut state = with_note();
    assert!(state.start_text_edit(NodeId::new("n1")));
    assert!(state.text_edit_insert("Hi", 1_000));
    assert!(state.text_edit_commit());
    assert_eq!(body(&state).1.plain_content(), "Hi");
}
