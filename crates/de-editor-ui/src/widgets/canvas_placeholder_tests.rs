use super::{paint, with_overlay, SWEEP_FRAME_INTERVAL_MS};
use crate::layout_scene::{NodeKind, SceneNode};
use crate::widgets::canvas_shader_clock;
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::PaintCx;
use crate::Rect;

fn placeholder_frame() -> SceneNode {
    let mut frame = SceneNode::leaf("busy", NodeKind::Frame);
    frame.bounds = Rect::xywh(0.0, 0.0, 200.0, 120.0);
    frame.placeholder = true;
    frame
}

fn paint_once(node: &SceneNode) -> (bool, CaptureBackend) {
    let mut backend = CaptureBackend::default();
    let painted = {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        paint(&mut cx, node, node.bounds, 1.0, 1_000)
    };
    (painted, backend)
}

#[test]
fn a_placeholder_frame_sweeps_on_the_canvas_and_keeps_frames_coming() {
    canvas_shader_clock::begin_frame(1_000);
    let (painted, backend) = with_overlay(|| paint_once(&placeholder_frame()));
    assert!(painted);
    assert!(
        !backend.rect_fills.is_empty(),
        "the panel wash and the sweep band paint"
    );
    assert_eq!(
        canvas_shader_clock::next_deadline_ms(1_000),
        Some(1_000 + SWEEP_FRAME_INTERVAL_MS)
    );
}

#[test]
fn exports_and_plain_frames_paint_no_overlay() {
    canvas_shader_clock::begin_frame(1_000);
    // Outside the canvas scope (exports, screenshots).
    let (painted, backend) = paint_once(&placeholder_frame());
    assert!(!painted);
    assert!(backend.rect_fills.is_empty());
    // A frame without the flag, on the canvas.
    let mut plain = placeholder_frame();
    plain.placeholder = false;
    let (painted, _) = with_overlay(|| paint_once(&plain));
    assert!(!painted);
    assert_eq!(canvas_shader_clock::next_deadline_ms(1_000), None);
}

#[test]
fn the_overlay_scope_ends_with_the_canvas_paint() {
    with_overlay(|| {});
    let (painted, _) = paint_once(&placeholder_frame());
    assert!(!painted);
}

#[test]
fn a_time_shader_keeps_its_faster_cadence_next_to_a_sweep() {
    canvas_shader_clock::begin_frame(1_000);
    with_overlay(|| paint_once(&placeholder_frame()));
    canvas_shader_clock::time_uniform_value("uniform float time;");
    assert_eq!(
        canvas_shader_clock::next_deadline_ms(1_000),
        Some(1_000 + canvas_shader_clock::FRAME_INTERVAL_MS)
    );
}
