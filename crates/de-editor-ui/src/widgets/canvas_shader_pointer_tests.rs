//! Tests for the pointer-reactive shader input. Each `#[test]` runs on its
//! own thread, so the thread-local pointer starts fresh in every test.

use super::*;

const MOUSE_SHADER: &str = "uniform float2 mouse; half4 main(float2 p){ return half4(1); }";

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::xywh(x, y, w, h)
}

fn assert_near(actual: Option<[f32; 2]>, expected: [f32; 2]) {
    let actual = actual.expect("the shader declares mouse");
    assert!(
        (actual[0] - expected[0]).abs() < 1e-3 && (actual[1] - expected[1]).abs() < 1e-3,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn declares_mouse_uniform_accepts_float2_and_half2_with_any_spacing() {
    assert!(declares_mouse_uniform(MOUSE_SHADER));
    assert!(declares_mouse_uniform("uniform half2 mouse;"));
    assert!(declares_mouse_uniform("uniform   float2\n  mouse ;"));
}

#[test]
fn declares_mouse_uniform_rejects_other_names_types_and_non_uniforms() {
    assert!(!declares_mouse_uniform("uniform float2 mousePos;"));
    assert!(!declares_mouse_uniform("uniform float mouse;"));
    assert!(!declares_mouse_uniform("float2 mouse = float2(0);"));
    assert!(!declares_mouse_uniform("uniform float time;"));
}

#[test]
fn a_shader_without_the_uniform_gets_nothing_and_does_not_mark_the_frame() {
    begin_frame(Some(Point2D::new(5.0, 5.0)));
    assert_eq!(
        mouse_uniform_value("uniform half4 tint;", rect(0.0, 0.0, 10.0, 10.0), &[]),
        None
    );
    assert!(!pointer_shader_painted());
}

#[test]
fn the_pointer_is_node_local_screen_pixels_without_a_size_uniform() {
    begin_frame(Some(Point2D::new(130.0, 70.0)));
    assert_near(
        mouse_uniform_value(MOUSE_SHADER, rect(100.0, 50.0, 40.0, 40.0), &[]),
        [30.0, 20.0],
    );
    assert!(pointer_shader_painted());
}

#[test]
fn a_size_uniform_rescales_the_pointer_like_frag_coord() {
    // A 200x100 node painted at zoom 0.5: the box is 100x50 on screen and
    // the backends scale fragCoord back to document pixels.
    begin_frame(Some(Point2D::new(60.0, 35.0)));
    let size = [200.0, 100.0];
    assert_near(
        mouse_uniform_value(
            MOUSE_SHADER,
            rect(10.0, 10.0, 100.0, 50.0),
            &[("size", &size)],
        ),
        [100.0, 50.0],
    );
}

#[test]
fn no_pointer_binds_the_idle_value() {
    begin_frame(None);
    assert_eq!(
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 10.0, 10.0), &[]),
        Some(NO_POINTER)
    );
    assert!(
        pointer_shader_painted(),
        "the shader still listens for moves"
    );
}

#[test]
fn paints_after_the_canvas_frame_see_no_pointer() {
    begin_frame(Some(Point2D::new(5.0, 5.0)));
    end_frame();
    assert_eq!(
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 10.0, 10.0), &[]),
        Some(NO_POINTER)
    );
}

#[test]
fn with_fixed_pointer_pins_the_pointer_and_restores_the_canvas_frame() {
    begin_frame(Some(Point2D::new(5.0, 5.0)));
    let pinned = with_fixed_pointer(None, || {
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 10.0, 10.0), &[])
    });
    assert_eq!(pinned, Some(NO_POINTER));
    assert!(
        !pointer_shader_painted(),
        "a pinned export must not mark the canvas frame"
    );
    assert_near(
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 10.0, 10.0), &[]),
        [5.0, 5.0],
    );
}

#[test]
fn a_rotated_node_sees_the_pointer_in_its_own_axes() {
    // Node box (0,0)-(100,100), pivot at its centre, rotated a quarter turn
    // clockwise. The screen point right of the centre is the node's local
    // point above the centre.
    begin_frame(Some(Point2D::new(90.0, 50.0)));
    let outer = enter_node_transform(NodeTransform {
        pop_scale: None,
        flip_x: false,
        flip_y: false,
        rotation: std::f32::consts::FRAC_PI_2,
        pivot: Point2D::new(50.0, 50.0),
    });
    assert_near(
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 100.0, 100.0), &[]),
        [50.0, 10.0],
    );
    leave_node_transform(outer);
    assert_near(
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 100.0, 100.0), &[]),
        [90.0, 50.0],
    );
}

#[test]
fn the_inverse_undoes_flip_after_rotation_in_skia_order() {
    // Forward: screen = pivot + S_flip · R · (p - pivot). With flip_x and a
    // quarter turn, local (50,10) (above the centre) lands left of it.
    begin_frame(Some(Point2D::new(10.0, 50.0)));
    let outer = enter_node_transform(NodeTransform {
        pop_scale: None,
        flip_x: true,
        flip_y: false,
        rotation: std::f32::consts::FRAC_PI_2,
        pivot: Point2D::new(50.0, 50.0),
    });
    assert_near(
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 100.0, 100.0), &[]),
        [50.0, 10.0],
    );
    leave_node_transform(outer);
}

#[test]
fn a_reveal_pop_scale_is_undone_about_the_pivot() {
    begin_frame(Some(Point2D::new(60.0, 50.0)));
    let outer = enter_node_transform(NodeTransform {
        pop_scale: Some(0.5),
        flip_x: false,
        flip_y: false,
        rotation: 0.0,
        pivot: Point2D::new(50.0, 50.0),
    });
    assert_near(
        mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 100.0, 100.0), &[]),
        [70.0, 50.0],
    );
    leave_node_transform(outer);
}

#[test]
fn track_pointer_owes_a_repaint_only_for_a_change_with_a_listener_on_screen() {
    let mut stored = None;
    begin_frame(None);
    assert!(
        !track_pointer(&mut stored, Some(Point2D::new(1.0, 1.0))),
        "no pointer shader on screen: moves must not repaint"
    );
    assert_eq!(stored, Some(Point2D::new(1.0, 1.0)));

    mouse_uniform_value(MOUSE_SHADER, rect(0.0, 0.0, 10.0, 10.0), &[]);
    assert!(track_pointer(&mut stored, Some(Point2D::new(2.0, 1.0))));
    assert!(!track_pointer(&mut stored, Some(Point2D::new(2.0, 1.0))));
    assert!(
        track_pointer(&mut stored, None),
        "leaving the canvas repaints to the idle value"
    );
}
