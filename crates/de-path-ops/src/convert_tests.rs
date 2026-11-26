use super::*;
use skia_safe::Rect as SkRect;

fn circle(cx: f32, cy: f32, r: f32) -> Path {
    let mut b = PathBuilder::new();
    b.add_oval(
        SkRect::from_xywh(cx - r, cy - r, 2.0 * r, 2.0 * r),
        None,
        None,
    );
    b.detach()
}

#[test]
fn a_circle_becomes_four_cubic_arcs() {
    let subs = path_to_subpaths(&circle(50.0, 50.0, 50.0));
    assert_eq!(subs.len(), 1);
    assert!(subs[0].closed);
    assert_eq!(subs[0].anchors.len(), 4, "{:?}", subs[0].anchors);
    assert_eq!(cubic_segment_count(&subs), 4);
    // Kappa: the handle of a quarter arc is 0.5523 r long.
    let h = subs[0].anchors[0].handle_out.as_ref().expect("handle");
    let len = (h.x * h.x + h.y * h.y).sqrt();
    assert!((len - 27.614).abs() < 0.05, "handle length {len}");
}

#[test]
fn round_trip_keeps_the_shape() {
    let original = circle(10.0, 20.0, 30.0);
    let back = subpaths_to_path(&path_to_subpaths(&original), false);
    let (a, b) = (original.compute_tight_bounds(), back.compute_tight_bounds());
    assert!((a.left() - b.left()).abs() < 0.05);
    assert!((a.width() - b.width()).abs() < 0.05);
    assert!((a.height() - b.height()).abs() < 0.05);
}

#[test]
fn lines_stay_corners_and_closing_seam_folds() {
    let mut b = PathBuilder::new();
    b.move_to((0.0, 0.0));
    b.line_to((10.0, 0.0));
    b.line_to((10.0, 10.0));
    b.line_to((0.0, 0.0));
    b.close();
    let subs = path_to_subpaths(&b.detach());
    assert_eq!(subs[0].anchors.len(), 3);
    assert_eq!(cubic_segment_count(&subs), 0);
}

#[test]
fn quads_are_raised_exactly() {
    let mut b = PathBuilder::new();
    b.move_to((0.0, 0.0));
    b.quad_to((10.0, 20.0), (20.0, 0.0));
    let subs = path_to_subpaths(&b.detach());
    let a0 = &subs[0].anchors[0];
    let out = a0.handle_out.as_ref().unwrap();
    // Exact up to the 1/1000 px rounding.
    assert!((out.x - 20.0 / 3.0).abs() < 1e-3 && (out.y - 40.0 / 3.0).abs() < 1e-3);
}

#[test]
fn wide_conics_are_split() {
    let mut b = PathBuilder::new();
    b.move_to((0.0, 0.0));
    // A 180° conic (weight 0) is a degenerate case; 120° has w = 0.5.
    b.conic_to((10.0, 10.0), (20.0, 0.0), 0.5);
    let subs = path_to_subpaths(&b.detach());
    assert!(subs[0].anchors.len() >= 3, "split into ≥ 2 cubics");
}

#[test]
fn even_odd_fill_round_trips() {
    let subs = path_to_subpaths(&circle(0.0, 0.0, 5.0));
    assert_eq!(
        subpaths_to_path(&subs, true).fill_type(),
        PathFillType::EvenOdd
    );
    assert_eq!(
        subpaths_to_path(&subs, false).fill_type(),
        PathFillType::Winding
    );
}
