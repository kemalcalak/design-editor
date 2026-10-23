//! `EditorCommand::CommitBooleanResult` apply tests.

use crate::command::EditorCommand;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::test_support::{frame, rect, state_with};
use jian_ops_schema::node::PenNode;

fn id(s: &str) -> NodeId {
    NodeId::new(s)
}

fn square() -> Vec<Vec<(f64, f64)>> {
    vec![vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]]
}

fn commit(s: &EditorState, source_ids: &[&str], new_id: &str) -> EditorCommand {
    EditorCommand::CommitBooleanResult {
        source_ids: source_ids.iter().map(|s| id(s)).collect(),
        contours: square(),
        new_id: id(new_id),
        revision: s.document_revision(),
    }
}

#[test]
fn commit_boolean_result_swaps_sources_selects_and_undoes_in_one_step() {
    let mut s = state_with(vec![
        rect("a", "A", 0.0, 0.0, 10.0, 10.0),
        rect("b", "B", 5.0, 5.0, 15.0, 15.0),
        rect("c", "C", 50.0, 50.0, 5.0, 5.0),
    ]);
    let depth = s.history.past.len();

    assert!(s.apply(commit(&s, &["a", "b"], "n7")));

    let ids: Vec<&str> = s.active_children().iter().map(|n| n.id_str()).collect();
    assert_eq!(ids, ["c", "n7"]);
    assert!(matches!(s.active_children()[1], PenNode::Path(_)));
    assert_eq!(s.selection.set, vec![id("n7")]);
    assert_eq!(s.history.past.len(), depth + 1);

    assert!(s.undo());
    let ids: Vec<&str> = s.active_children().iter().map(|n| n.id_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[test]
fn commit_boolean_result_rejects_a_taken_id_without_mutating() {
    let mut s = state_with(vec![
        rect("a", "A", 0.0, 0.0, 10.0, 10.0),
        rect("b", "B", 5.0, 5.0, 15.0, 15.0),
    ]);
    let before = s.doc.clone();
    assert!(!s.apply(commit(&s, &["a", "b"], "b")));
    assert_eq!(s.doc, before);
    assert!(s.history.past.is_empty());
}

#[test]
fn commit_boolean_result_rejects_a_missing_source_without_mutating() {
    let mut s = state_with(vec![rect("a", "A", 0.0, 0.0, 10.0, 10.0)]);
    let before = s.doc.clone();
    assert!(!s.apply(commit(&s, &["a", "gone"], "n7")));
    assert_eq!(s.doc, before);
}

#[test]
fn commit_boolean_result_rejects_degenerate_contours() {
    let mut s = state_with(vec![
        rect("a", "A", 0.0, 0.0, 10.0, 10.0),
        rect("b", "B", 5.0, 5.0, 15.0, 15.0),
    ]);
    let before = s.doc.clone();
    assert!(!s.apply(EditorCommand::CommitBooleanResult {
        source_ids: vec![id("a"), id("b")],
        contours: vec![vec![(1.0, 1.0)]],
        new_id: id("n7"),
        revision: s.document_revision(),
    }));
    assert_eq!(s.doc, before);
    assert!(s.history.past.is_empty());
}

#[test]
fn commit_boolean_result_rejects_a_stale_revision_without_mutating() {
    let mut s = state_with(vec![
        rect("a", "A", 0.0, 0.0, 10.0, 10.0),
        rect("b", "B", 5.0, 5.0, 15.0, 15.0),
    ]);
    let stale = commit(&s, &["a", "b"], "n7");
    s.mark_document_changed();
    let before = s.doc.clone();
    assert!(!s.apply(stale));
    assert_eq!(s.doc, before);
    assert!(s.history.past.is_empty());
}

#[test]
fn commit_boolean_result_refuses_a_source_with_children_without_mutating() {
    let mut s = state_with(vec![
        frame(
            "f",
            "F",
            0.0,
            0.0,
            40.0,
            40.0,
            vec![rect("inner", "Inner", 0.0, 0.0, 5.0, 5.0)],
        ),
        rect("b", "B", 5.0, 5.0, 15.0, 15.0),
    ]);
    let before = s.doc.clone();
    assert!(!s.apply(commit(&s, &["f", "b"], "n7")));
    assert_eq!(s.doc, before);
    assert!(s.history.past.is_empty());
}

#[test]
fn commit_boolean_result_accepts_an_empty_frame() {
    let mut s = state_with(vec![
        frame("f", "F", 0.0, 0.0, 40.0, 40.0, vec![]),
        rect("b", "B", 5.0, 5.0, 15.0, 15.0),
    ]);
    assert!(s.apply(commit(&s, &["f", "b"], "n7")));
    let ids: Vec<&str> = s.active_children().iter().map(|n| n.id_str()).collect();
    assert_eq!(ids, ["n7"]);
}
