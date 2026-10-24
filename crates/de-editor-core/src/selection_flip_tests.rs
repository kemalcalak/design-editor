use super::*;
use crate::node_id::NodeId;
use crate::test_support::{rect, state_with};
use crate::walkers::find_node;

fn flips(state: &EditorState, id: &str) -> (Option<bool>, Option<bool>) {
    let base = find_node(state.active_children(), &NodeId::new(id))
        .expect("node")
        .base();
    (base.flip_x, base.flip_y)
}

#[test]
fn flip_selection_toggles_every_selected_node_as_one_undo_step() {
    let mut s = state_with(vec![
        rect("a", "A", 0.0, 0.0, 10.0, 10.0),
        rect("b", "B", 20.0, 0.0, 10.0, 10.0),
    ]);
    s.set_single_selection(NodeId::new("a"));
    s.toggle_selection(NodeId::new("b"));

    assert!(flip_selection(&mut s, FlipAxis::Horizontal));
    assert_eq!(flips(&s, "a"), (Some(true), None));
    assert_eq!(flips(&s, "b"), (Some(true), None));

    assert!(s.undo());
    assert_eq!(flips(&s, "a"), (None, None));
    assert_eq!(flips(&s, "b"), (None, None));
    assert!(!s.undo(), "the flip is a single history entry");
}

#[test]
fn flipping_twice_clears_the_field() {
    let mut s = state_with(vec![rect("a", "A", 0.0, 0.0, 10.0, 10.0)]);
    s.set_single_selection(NodeId::new("a"));
    assert!(s.flip_selected(FlipAxis::Vertical));
    assert_eq!(flips(&s, "a"), (None, Some(true)));
    assert!(node_flipped(
        find_node(s.active_children(), &NodeId::new("a")).unwrap(),
        FlipAxis::Vertical
    ));
    assert!(s.flip_selected(FlipAxis::Vertical));
    assert_eq!(flips(&s, "a"), (None, None));
}

#[test]
fn flip_skips_locked_nodes_and_an_empty_selection() {
    let mut locked = rect("a", "A", 0.0, 0.0, 10.0, 10.0);
    locked.base_mut().locked = Some(true);
    let mut s = state_with(vec![locked]);
    assert!(!flip_selection(&mut s, FlipAxis::Horizontal));
    s.set_single_selection(NodeId::new("a"));
    assert!(!flip_selection(&mut s, FlipAxis::Horizontal));
    assert_eq!(flips(&s, "a"), (None, None));
    assert!(!s.undo(), "a rejected flip leaves no history entry");
}
