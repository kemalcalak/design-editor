//! Session operations against an `EditorState`: no change on enter,
//! history per action, continue / close / join, imported icons.

use super::*;
use crate::pen_node_ext::{make_path, PenNodeExt};
use crate::state::EditorState;
use crate::test_support::state_with;
use crate::walkers::find_node;
use jian_ops_schema::node::{PenNode, PenPathPointType};

/// An imported-style icon: `d` only, arcs + a compound hole, in a 24 px
/// native box drawn at 2× (48 px) at (100, 50).
const ICON_D: &str =
    "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20z M12 7a5 5 0 1 1 0 10a5 5 0 0 1 0-10z";

fn icon_state() -> (EditorState, NodeId) {
    let mut node = make_path("icon".into(), "Icon", (100.0, 50.0));
    if let PenNode::Path(p) = &mut node {
        p.anchors = None;
        p.d = Some(ICON_D.into());
    }
    node.set_width_px(40.0);
    node.set_height_px(40.0);
    let s = state_with(vec![node]);
    (s, NodeId::new("icon"))
}

fn open_path_state() -> (EditorState, NodeId) {
    let mut node = make_path("p".into(), "Path", (0.0, 0.0));
    if let PenNode::Path(p) = &mut node {
        p.anchors = Some(Subpath::polyline(&[(0.0, 0.0), (10.0, 0.0), (20.0, 10.0)]).anchors);
        p.closed = Some(false);
    }
    let mut s = state_with(vec![node]);
    s.enter_vector_edit(&NodeId::new("p"));
    (s, NodeId::new("p"))
}

fn node<'a>(s: &'a EditorState, id: &NodeId) -> &'a PenNode {
    find_node(s.active_children(), id).expect("node")
}

fn anchor_xs(s: &EditorState) -> Vec<(f64, f64)> {
    s.vector_subpaths().unwrap()[0]
        .anchors
        .iter()
        .map(|a| (a.x, a.y))
        .collect()
}

#[test]
fn entering_leaves_an_imported_path_untouched() {
    let (mut s, id) = icon_state();
    let before = node(&s, &id).clone();
    let history = s.history_push_count;
    assert!(s.enter_vector_edit(&id));
    assert_eq!(node(&s, &id), &before, "no document change on enter");
    assert_eq!(s.history_push_count, history);
    let subs = s.vector_subpaths().unwrap();
    assert_eq!(subs.len(), 2, "outer ring + hole");
    // Points land in the node's box: 24 px native → 40 px at (100, 50).
    let (x, y, w, h) = subpaths_bounds(&subs);
    assert!(
        (x - 100.0).abs() < 0.05 && (y - 50.0).abs() < 0.05,
        "{x},{y}"
    );
    assert!(
        (w - 40.0).abs() < 0.05 && (h - 40.0).abs() < 0.05,
        "{w},{h}"
    );
}

#[test]
fn first_edit_bakes_d_without_visual_change() {
    let (mut s, id) = icon_state();
    s.enter_vector_edit(&id);
    let before = s.vector_subpaths().unwrap();
    s.vector_select(PointRef::new(1, 0), false);
    assert!(s.vector_nudge(3.0, 0.0));
    assert!(s.vector_nudge(-3.0, 0.0));
    let after = s.vector_subpaths().unwrap();
    assert_eq!(before.len(), after.len());
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(b.closed, a.closed);
        for (p, q) in b.anchors.iter().zip(&a.anchors) {
            assert!((p.x - q.x).abs() < 1e-6 && (p.y - q.y).abs() < 1e-6);
        }
    }
    // The baked path draws at scale 1: its box is its own bounds.
    let n = node(&s, &id);
    let (x, y, w, h) = subpaths_bounds(&after);
    assert!((n.base().x.unwrap() - x).abs() < 1e-9);
    assert!((n.base().y.unwrap() - y).abs() < 1e-9);
    assert!((n.width_px().unwrap() - w).abs() < 1e-9);
    assert!((n.height_px().unwrap() - h).abs() < 1e-9);
    let PenNode::Path(p) = n else { panic!() };
    assert_ne!(p.d.as_deref(), Some(ICON_D), "d re-baked after the edit");
    assert!(p.anchors.is_none(), "compound path keeps d only");
    // Two nudges, two undo steps; undo twice restores the original d.
    s.undo();
    s.undo();
    let PenNode::Path(p) = node(&s, &id) else {
        panic!()
    };
    assert_eq!(p.d.as_deref(), Some(ICON_D));
}

#[test]
fn delete_heals_and_undo_restores() {
    let (mut s, id) = open_path_state();
    s.vector_select(PointRef::new(0, 1), false);
    assert!(s.vector_delete_selected());
    assert_eq!(anchor_xs(&s), vec![(0.0, 0.0), (20.0, 10.0)]);
    s.undo();
    let PenNode::Path(p) = node(&s, &id) else {
        panic!()
    };
    assert_eq!(p.anchors.as_ref().unwrap().len(), 3);
}

#[test]
fn deleting_every_point_deletes_the_path() {
    let (mut s, id) = open_path_state();
    assert!(s.vector_select_all());
    assert!(s.vector_delete_selected());
    assert!(find_node(s.active_children(), &id).is_none());
    assert!(!s.vector_edit_active());
}

#[test]
fn insert_point_selects_it() {
    let (mut s, _) = open_path_state();
    let p = s.vector_insert_point(0, 0, 0.5).unwrap();
    assert_eq!(p, PointRef::new(0, 1));
    assert_eq!(anchor_xs(&s)[1], (5.0, 0.0));
    assert!(s.vector_edit().unwrap().selected.contains(&p));
}

#[test]
fn pen_continues_from_either_end_and_closes() {
    let (mut s, id) = open_path_state();
    s.exit_vector_edit();
    // From the last point: appends.
    assert!(s.continue_path_with_pen(&id, PointRef::new(0, 2)));
    let p = s.vector_pen_add_point((30.0, 30.0)).unwrap();
    assert_eq!(p, PointRef::new(0, 3));
    assert_eq!(anchor_xs(&s).last(), Some(&(30.0, 30.0)));
    // From the first point: prepends.
    assert!(s.vector_pen_continue_from(PointRef::new(0, 0)));
    s.vector_pen_add_point((-10.0, 0.0)).unwrap();
    assert_eq!(anchor_xs(&s)[0], (-10.0, 0.0));
    // A drag mirrors handles in the drawing direction (prepend → in).
    assert!(s.vector_pen_drag_handle(PointRef::new(0, 0), (-20.0, 0.0)));
    let a = &s.vector_subpaths().unwrap()[0].anchors[0];
    assert_eq!(a.handle_in.as_ref().map(|h| h.x), Some(-10.0));
    // Clicking the far end closes.
    assert!(s.vector_pen_close());
    assert!(s.vector_subpaths().unwrap()[0].closed);
}

#[test]
fn pen_starts_a_new_subpath_and_connects() {
    let (mut s, _) = open_path_state();
    s.vector_pen_add_point((50.0, 50.0)).unwrap();
    s.vector_pen_add_point((60.0, 50.0)).unwrap();
    assert_eq!(s.vector_subpaths().unwrap().len(), 2);
    assert!(s.vector_pen_connect(PointRef::new(0, 2)));
    let subs = s.vector_subpaths().unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].anchors.len(), 5);
}

#[test]
fn cmd_j_joins_two_selected_end_points() {
    let (mut s, _) = open_path_state();
    s.vector_select_many([PointRef::new(0, 0), PointRef::new(0, 2)], false);
    assert!(s.vector_join_selected());
    assert!(s.vector_subpaths().unwrap()[0].closed);
    // A middle point is not an end point.
    s.vector_select_many([PointRef::new(0, 0), PointRef::new(0, 1)], false);
    assert!(!s.vector_join_selected());
}

#[test]
fn alt_click_toggles_corner_and_smooth() {
    let (mut s, _) = open_path_state();
    assert!(s.vector_toggle_smooth(PointRef::new(0, 1)));
    let a = &s.vector_subpaths().unwrap()[0].anchors[1];
    assert_eq!(a.point_type, Some(PenPathPointType::Mirrored));
    assert!(s.vector_toggle_smooth(PointRef::new(0, 1)));
    let a = &s.vector_subpaths().unwrap()[0].anchors[1];
    assert!(a.handle_in.is_none() && a.handle_out.is_none());
}

#[test]
fn alt_drag_breaks_handle_symmetry() {
    let (mut s, _) = open_path_state();
    s.vector_toggle_smooth(PointRef::new(0, 1));
    let base = s.vector_subpaths().unwrap();
    let p = PointRef::new(0, 1);
    s.vector_set_handle_from(&base, p, crate::pen::PathHandleSide::Out, (4.0, 4.0), false);
    let a = s.vector_subpaths().unwrap()[0].anchors[1].clone();
    assert_eq!(a.handle_in.as_ref().map(|h| (h.x, h.y)), Some((-4.0, -4.0)));
    let base = s.vector_subpaths().unwrap();
    s.vector_set_handle_from(&base, p, crate::pen::PathHandleSide::Out, (8.0, 0.0), true);
    let a = s.vector_subpaths().unwrap()[0].anchors[1].clone();
    assert_eq!(a.handle_in.as_ref().map(|h| (h.x, h.y)), Some((-4.0, -4.0)));
    assert_eq!(a.point_type, Some(PenPathPointType::Independent));
}

#[test]
fn inspector_position_moves_the_selection() {
    let (mut s, _) = open_path_state();
    s.vector_select_many([PointRef::new(0, 1), PointRef::new(0, 2)], false);
    assert_eq!(s.vector_selection_origin(), Some((10.0, 0.0)));
    assert!(s.vector_set_selection_origin(Some(12.0), None));
    assert_eq!(anchor_xs(&s), vec![(0.0, 0.0), (12.0, 0.0), (22.0, 10.0)]);
    assert!(s.vector_set_point_type(PenPathPointType::Corner));
    assert_eq!(
        s.vector_selection_point_type(),
        Some(PenPathPointType::Corner)
    );
}

#[test]
fn session_drops_when_selection_moves_away() {
    let (mut s, _) = open_path_state();
    assert!(s.vector_edit_active());
    s.clear_selection();
    assert!(!s.vector_edit_active());
    assert!(s.prune_vector_edit());
}
