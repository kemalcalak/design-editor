use super::*;

const PLAIN: DrawModifiers = DrawModifiers {
    shift: false,
    alt: false,
};
const SHIFT: DrawModifiers = DrawModifiers {
    shift: true,
    alt: false,
};
const ALT: DrawModifiers = DrawModifiers {
    shift: false,
    alt: true,
};
const BOTH: DrawModifiers = DrawModifiers {
    shift: true,
    alt: true,
};

fn boxed(x: f64, y: f64, w: f64, h: f64) -> DrawnShape {
    DrawnShape::Box { x, y, w, h }
}

fn line(x: f64, y: f64, dx: f64, dy: f64) -> DrawnShape {
    DrawnShape::Line { x, y, dx, dy }
}

fn close(a: DrawnShape, b: DrawnShape) -> bool {
    let parts = |s: DrawnShape| match s {
        DrawnShape::Box { x, y, w, h } => (0, [x, y, w, h]),
        DrawnShape::Line { x, y, dx, dy } => (1, [x, y, dx, dy]),
    };
    let ((ka, a), (kb, b)) = (parts(a), parts(b));
    ka == kb && a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-9)
}

#[test]
fn a_plain_drag_spans_press_to_cursor_in_any_direction() {
    let p = DrawKind::Proportional;
    assert_eq!(
        drawn_shape(p, (10.0, 20.0), (50.0, 30.0), PLAIN),
        boxed(10.0, 20.0, 40.0, 10.0)
    );
    assert_eq!(
        drawn_shape(p, (10.0, 20.0), (0.0, 5.0), PLAIN),
        boxed(0.0, 5.0, 10.0, 15.0)
    );
}

#[test]
fn shift_squares_a_box_toward_the_cursor() {
    let p = DrawKind::Proportional;
    assert_eq!(
        drawn_shape(p, (10.0, 20.0), (50.0, 30.0), SHIFT),
        boxed(10.0, 20.0, 40.0, 40.0)
    );
    // Up and to the left the square grows that way.
    assert_eq!(
        drawn_shape(p, (10.0, 20.0), (5.0, -10.0), SHIFT),
        boxed(-20.0, -10.0, 30.0, 30.0)
    );
    // Text boxes and widgets stay free.
    assert_eq!(
        drawn_shape(DrawKind::Free, (0.0, 0.0), (40.0, 10.0), SHIFT),
        boxed(0.0, 0.0, 40.0, 10.0)
    );
}

#[test]
fn alt_draws_from_the_centre_and_combines_with_shift() {
    let p = DrawKind::Proportional;
    assert_eq!(
        drawn_shape(p, (100.0, 100.0), (130.0, 90.0), ALT),
        boxed(70.0, 90.0, 60.0, 20.0)
    );
    assert_eq!(
        drawn_shape(p, (100.0, 100.0), (130.0, 90.0), BOTH),
        boxed(70.0, 70.0, 60.0, 60.0)
    );
    assert_eq!(
        drawn_shape(DrawKind::Free, (100.0, 100.0), (130.0, 90.0), BOTH),
        boxed(70.0, 90.0, 60.0, 20.0)
    );
}

#[test]
fn a_line_keeps_its_direction_and_shift_snaps_it_to_45_degrees() {
    let l = DrawKind::Line;
    // Drawn up and to the right: the start stays the start.
    assert_eq!(
        drawn_shape(l, (10.0, 50.0), (40.0, 20.0), PLAIN),
        line(10.0, 50.0, 30.0, -30.0)
    );
    // Nearly horizontal snaps flat, keeping its projected length.
    assert!(close(
        drawn_shape(l, (0.0, 0.0), (50.0, 4.0), SHIFT),
        line(0.0, 0.0, 50.0, 0.0)
    ));
    // Near a diagonal snaps onto it.
    let snapped = drawn_shape(l, (0.0, 0.0), (-30.0, 34.0), SHIFT);
    let DrawnShape::Line { dx, dy, .. } = snapped else {
        panic!("{snapped:?}");
    };
    assert!((dx + dy).abs() < 1e-9 && dx < 0.0, "{snapped:?}");
}

#[test]
fn alt_lines_run_through_the_press_point() {
    let l = DrawKind::Line;
    assert_eq!(
        drawn_shape(l, (100.0, 100.0), (120.0, 90.0), ALT),
        line(80.0, 110.0, 40.0, -20.0)
    );
    assert!(close(
        drawn_shape(l, (100.0, 100.0), (130.0, 96.0), BOTH),
        line(70.0, 100.0, 60.0, 0.0)
    ));
}

#[test]
fn draw_kinds_follow_the_tool() {
    assert_eq!(DrawKind::for_tool(Tool::Line), DrawKind::Line);
    for tool in [
        Tool::Rect,
        Tool::Ellipse,
        Tool::Polygon,
        Tool::Star,
        Tool::Frame,
    ] {
        assert_eq!(DrawKind::for_tool(tool), DrawKind::Proportional, "{tool:?}");
    }
    assert_eq!(DrawKind::for_tool(Tool::Text), DrawKind::Free);
}

#[test]
fn a_snapped_shift_diagonal_keeps_equal_legs() {
    // 3.5 / 3.4999 legs round a pixel apart; Shift keeps them equal.
    assert_eq!(
        snap_line(10.0, 10.0, 3.5, -3.4999, true),
        (10.0, 10.0, 4.0, -4.0)
    );
    assert_eq!(
        snap_line(10.0, 10.0, 3.5, -3.4999, false),
        (10.0, 10.0, 4.0, -3.0)
    );
    // A flat Shift line stays flat.
    assert_eq!(snap_line(10.2, 9.8, 6.6, 0.0, true), (10.0, 10.0, 7.0, 0.0));
}
