use super::*;
use crate::NodeId;

#[test]
fn grid_shows_from_four_hundred_percent_unless_hidden() {
    assert!(!pixel_grid_visible(3.99, false));
    assert!(pixel_grid_visible(4.0, false));
    assert!(pixel_grid_visible(32.0, false));
    assert!(!pixel_grid_visible(8.0, true));
}

#[test]
fn rect_edges_round_to_whole_pixels() {
    assert_eq!(snap_rect_edges(1.4, 2.6, 10.2, 3.3), (1.0, 3.0, 11.0, 3.0));
    // A sliver keeps one pixel.
    assert_eq!(snap_rect_edges(5.1, 5.1, 0.2, 0.2), (5.0, 5.0, 1.0, 1.0));
}

#[test]
fn snap_offset_lands_on_the_nearest_pixel() {
    assert!((snap_offset(10.25) + 0.25).abs() < 1e-12);
    assert!((snap_offset(10.75) - 0.25).abs() < 1e-12);
    assert_eq!(snap_offset(7.0), 0.0);
}

#[test]
fn a_handle_only_rounds_the_edges_it_moves() {
    // Right handle: x stays fractional, the right edge rounds.
    let r = snap_moving_edges((0.3, 0.3, 10.4, 5.0), [false, false, true, false]);
    assert!((r.0 - 0.3).abs() < 1e-9);
    assert!((r.0 + r.2 - 11.0).abs() < 1e-9);
    // Top-left: both leading edges round, the far edges stay.
    let r = snap_moving_edges((0.6, 0.4, 10.0, 10.0), [true, true, false, false]);
    assert_eq!((r.0, r.1), (1.0, 0.0));
    assert!((r.0 + r.2 - 10.6).abs() < 1e-9);
}

#[test]
fn slow_drags_still_leave_the_pixel() {
    // Quarter-pixel steps at 400 %: the node must move every fourth-ish
    // step instead of rounding back to where it started.
    let (mut pos, mut residual) = (10.0_f64, 0.0_f64);
    let mut seen = Vec::new();
    for _ in 0..8 {
        pos += 0.25;
        let (correction, next) = snap_drag_axis(pos, residual);
        pos += correction;
        residual = next;
        seen.push(pos);
    }
    assert_eq!(seen, vec![10.0, 11.0, 11.0, 11.0, 11.0, 12.0, 12.0, 12.0]);
}

#[test]
fn snapping_follows_the_toggle_and_the_zoom() {
    let mut state = EditorState::starter();
    state.viewport.zoom = 1.0;
    assert!(!state.pixel_snap_active());
    state.viewport.zoom = 4.0;
    assert!(state.pixel_snap_active());
    state.ui.pixel_grid_snap_off = true;
    assert!(!state.pixel_snap_active());
}

#[test]
fn anchor_position_reads_the_authored_coordinates() {
    let mut state = EditorState::starter();
    let id = state
        .active_children()
        .first()
        .map(|n| NodeId::new(n.id_str()))
        .expect("starter frame");
    assert_eq!(state.anchor_authored_position(), None);
    state.set_single_selection(id);
    assert!(state.anchor_authored_position().is_some());
}
