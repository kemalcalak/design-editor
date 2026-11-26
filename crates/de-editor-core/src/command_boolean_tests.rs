//! `EditorCommand::Path(PathCommand::Commit)` apply tests (a boolean
//! committed destructively, as `boolean_op` with `mode: "flatten"` does).

use crate::command::EditorCommand;
use crate::node_id::NodeId;
use crate::path_ops::{PathCommand, PathCommit, PathCommitKind};
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::test_support::{frame, rect, state_with};
use crate::vector_edit::Subpath;
use jian_ops_schema::node::PenNode;

fn id(s: &str) -> NodeId {
    NodeId::new(s)
}

fn square() -> Vec<Subpath> {
    let mut s = Subpath::polyline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]);
    s.closed = true;
    vec![s]
}

fn command(
    source_ids: &[&str],
    subpaths: Vec<Subpath>,
    new_id: &str,
    revision: u64,
) -> EditorCommand {
    EditorCommand::Path(PathCommand::Commit(Box::new(PathCommit {
        source_ids: source_ids.iter().map(|s| id(s)).collect(),
        subpaths,
        even_odd: true,
        origin: (0.0, 0.0),
        new_id: id(new_id),
        revision,
        kind: PathCommitKind::Boolean,
    })))
}

fn commit(s: &EditorState, source_ids: &[&str], new_id: &str) -> EditorCommand {
    command(source_ids, square(), new_id, s.document_revision())
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
    // The result takes the first source's slot.
    assert_eq!(ids, ["n7", "c"]);
    assert!(matches!(s.active_children()[0], PenNode::Path(_)));
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
    let lone = vec![Subpath::polyline(&[(1.0, 1.0)])];
    assert!(!s.apply(command(&["a", "b"], lone, "n7", s.document_revision())));
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
