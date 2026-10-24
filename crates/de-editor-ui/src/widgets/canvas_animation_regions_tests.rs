//! Tests for the animated-region record. Each `#[test]` runs on its own
//! thread, so the thread-local record starts fresh in every test.

use super::*;
use crate::widgets::canvas_shader_pointer::{self, NodeTransform};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::xywh(x, y, w, h)
}

fn start_canvas_frame() {
    canvas_shader_pointer::begin_frame(None, false);
    begin_frame();
}

#[test]
fn records_only_inside_a_canvas_frame() {
    record(rect(1.0, 2.0, 3.0, 4.0));
    assert!(snapshot().rects.is_empty(), "no frame: nothing recorded");

    start_canvas_frame();
    record(rect(1.0, 2.0, 3.0, 4.0));
    assert_eq!(snapshot().rects, vec![rect(1.0, 2.0, 3.0, 4.0)]);
    assert!(snapshot().located);

    canvas_shader_pointer::end_frame();
    record(rect(9.0, 9.0, 1.0, 1.0));
    assert_eq!(snapshot().rects.len(), 1, "after the frame: nothing more");
}

#[test]
fn begin_frame_clears_the_record() {
    start_canvas_frame();
    record(rect(0.0, 0.0, 1.0, 1.0));
    mark_backdrop();
    begin_frame();
    assert_eq!(snapshot(), AnimationRegions {
        rects: Vec::new(),
        located: true
    });
}

#[test]
fn unlocated_animation_and_backdrop_reads_are_not_located() {
    start_canvas_frame();
    mark_unlocated();
    assert!(!snapshot().located);

    start_canvas_frame();
    mark_backdrop();
    assert!(!snapshot().located);
}

#[test]
fn the_placeholder_sweep_marks_the_frame_unlocated() {
    start_canvas_frame();
    crate::widgets::canvas_shader_clock::mark_animated(50);
    assert!(!snapshot().located);
}

#[test]
fn a_time_shader_alone_stays_located() {
    start_canvas_frame();
    crate::widgets::canvas_shader_clock::begin_frame(0);
    assert!(crate::widgets::canvas_shader_clock::time_uniform_value("uniform float time;").is_some());
    assert!(snapshot().located);
}

#[test]
fn boxes_map_through_the_node_transforms() {
    start_canvas_frame();
    // Quarter turn about (10, 10): the 4 x 2 box at (10, 10) stands up.
    let outer = canvas_shader_pointer::enter_node_transform(NodeTransform {
        pop_scale: None,
        flip_x: false,
        flip_y: false,
        rotation: std::f32::consts::FRAC_PI_2,
        pivot: Point2D::new(10.0, 10.0),
    });
    record(rect(10.0, 10.0, 4.0, 2.0));
    canvas_shader_pointer::leave_node_transform(outer);
    let got = snapshot().rects[0];
    assert!((got.origin.x - 8.0).abs() < 1e-4, "{got:?}");
    assert!((got.origin.y - 10.0).abs() < 1e-4, "{got:?}");
    assert!((got.size.x - 2.0).abs() < 1e-4, "{got:?}");
    assert!((got.size.y - 4.0).abs() < 1e-4, "{got:?}");

    // Popped: identity again.
    begin_frame();
    record(rect(10.0, 10.0, 4.0, 2.0));
    assert_eq!(snapshot().rects, vec![rect(10.0, 10.0, 4.0, 2.0)]);
}

#[test]
fn the_forward_map_inverts_the_pointer_map() {
    // A pointer at a box corner maps into the node's space and back.
    let transform = NodeTransform {
        pop_scale: Some(0.5),
        flip_x: true,
        flip_y: false,
        rotation: 0.7,
        pivot: Point2D::new(40.0, 30.0),
    };
    let screen = Point2D::new(55.0, 12.0);
    canvas_shader_pointer::begin_frame(Some(screen), false);
    begin_frame();
    let outer = canvas_shader_pointer::enter_node_transform(transform);
    let local = canvas_shader_pointer::mouse_uniform_value(
        "uniform float2 mouse;",
        rect(0.0, 0.0, 1.0, 1.0),
        &[],
    )
    .expect("declares mouse");
    let mapped = canvas_shader_pointer::map_to_canvas(rect(local[0], local[1], 0.0, 0.0))
        .expect("finite");
    canvas_shader_pointer::leave_node_transform(outer);
    assert!((mapped.origin.x - screen.x).abs() < 1e-3, "{mapped:?}");
    assert!((mapped.origin.y - screen.y).abs() < 1e-3, "{mapped:?}");
}

#[test]
fn snap_out_grows_to_whole_device_pixels() {
    let snapped = snap_out(rect(10.3, 4.6, 5.1, 0.2), 2.0);
    assert_eq!(snapped, rect(10.0, 4.5, 5.5, 0.5));
}

#[test]
fn overlapping_rects_merge_and_separate_ones_stay() {
    let merged = merge_overlapping(vec![
        rect(0.0, 0.0, 10.0, 10.0),
        rect(100.0, 100.0, 5.0, 5.0),
        rect(5.0, 5.0, 10.0, 10.0),
    ]);
    assert_eq!(merged.len(), 2);
    assert!(merged.contains(&rect(0.0, 0.0, 15.0, 15.0)));
    assert!(merged.contains(&rect(100.0, 100.0, 5.0, 5.0)));
}
