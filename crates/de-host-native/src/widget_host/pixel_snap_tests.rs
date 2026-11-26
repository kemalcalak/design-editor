//! Snap to pixel grid for moves, resizes and drawing, the pixel grid
//! toggle and the Alt measurement modifier (#399).

use super::{HandleDragState, NodeDragState, WidgetHostNative};
use de_editor_core::{NodeId, PenNodeExt};
use de_editor_ui::{widgets::SelectionHandle, Point2D, Rect};

fn seed(host: &mut WidgetHostNative, json: &str) {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
}

/// One rectangle `r` and a far-away `other` (no smart guide in range).
fn host_at_zoom(zoom: f32) -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[
          {"type":"rectangle","id":"r","name":"r","x":10,"y":10,"width":20,"height":20},
          {"type":"rectangle","id":"other","name":"other","x":5000,"y":5000,"width":7,"height":7}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("r"));
    host.editor_state_mut().viewport.zoom = zoom;
    host
}

fn geometry(host: &WidgetHostNative, id: &str) -> (f64, f64, f64, f64) {
    let node =
        de_editor_core::walkers::find_node(host.editor_state().active_children(), &NodeId::new(id))
            .expect("node present");
    (
        node.base().x.unwrap(),
        node.base().y.unwrap(),
        node.width_px().unwrap(),
        node.height_px().unwrap(),
    )
}

fn start_drag(host: &mut WidgetHostNative) {
    host.node_drag = Some(NodeDragState {
        last_screen_x: 500.0,
        last_screen_y: 500.0,
        press_screen_x: 500.0,
        press_screen_y: 500.0,
        moved: true,
        total_dx: 0.0,
        total_dy: 0.0,
        overlay_bounds: None,
    });
}

#[test]
fn node_moves_land_on_whole_pixels_at_high_zoom() {
    let mut host = host_at_zoom(8.0);
    start_drag(&mut host);
    // One screen px at 800 % is 1/8 doc px: walk 13 px right, 3 down.
    for step in 1..=13 {
        host.apply_cursor_move(500.0 + step as f32, 500.0 + (step.min(3)) as f32);
        let (x, y, _, _) = geometry(&host, "r");
        assert_eq!(x.fract(), 0.0, "x {x} after step {step}");
        assert_eq!(y.fract(), 0.0, "y {y} after step {step}");
    }
    // 13 / 8 = 1.625 doc px of travel rounds to 2; 3 / 8 rounds to 0.
    let (x, y, _, _) = geometry(&host, "r");
    assert_eq!((x, y), (12.0, 10.0));
}

#[test]
fn snapping_off_keeps_sub_pixel_moves() {
    let mut host = host_at_zoom(8.0);
    host.editor_state_mut().ui.pixel_grid_snap_off = true;
    start_drag(&mut host);
    host.apply_cursor_move(503.0, 500.0);
    assert_eq!(geometry(&host, "r").0, 10.375);
}

#[test]
fn low_zoom_moves_do_not_snap() {
    let mut host = host_at_zoom(1.5);
    start_drag(&mut host);
    host.apply_cursor_move(501.0, 500.0);
    assert!(geometry(&host, "r").0.fract() != 0.0);
}

#[test]
fn a_resize_handle_rounds_the_edge_it_drags() {
    let mut host = host_at_zoom(4.0);
    host.handle_drag = Some(HandleDragState {
        handle: SelectionHandle::BottomRight,
        start_screen_x: 500.0,
        start_screen_y: 500.0,
        start_bounds: Rect {
            origin: Point2D::new(10.0, 10.0),
            size: Point2D::new(20.0, 20.0),
        },
        start_authored_x: Some(10.0),
        start_authored_y: Some(10.0),
    });
    // +5 / +7 screen px = +1.25 / +1.75 doc px.
    host.apply_cursor_move(505.0, 507.0);
    assert_eq!(geometry(&host, "r"), (10.0, 10.0, 21.0, 22.0));
}

#[test]
fn the_pixel_grid_toggle_flips_and_reports() {
    let mut host = host_at_zoom(1.0);
    assert!(!host.editor_state().editor_ui.canvas_view.pixel_grid_hidden);
    assert!(host.apply_toggle_pixel_grid());
    assert!(host.editor_state().editor_ui.canvas_view.pixel_grid_hidden);
    assert!(host.apply_toggle_pixel_grid());
    assert!(!host.editor_state().editor_ui.canvas_view.pixel_grid_hidden);
}

#[test]
fn alt_arms_measurements_and_repaints_only_with_a_selection() {
    let mut host = host_at_zoom(1.0);
    assert!(host.set_modifier_alt(true), "selection present: repaint");
    assert!(host.editor_state().editor_ui.canvas_view.measure_modifier);
    assert!(!host.set_modifier_alt(true), "no change, no repaint");
    assert!(host.set_modifier_alt(false));
    host.editor_state_mut().clear_selection();
    assert!(
        !host.set_modifier_alt(true),
        "nothing selected: nothing to measure"
    );
    assert!(host.editor_state().editor_ui.canvas_view.measure_modifier);
}
