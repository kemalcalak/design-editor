//! `d` parsing, arc → cubic accuracy and the anchors ↔ `d` round trip.

use super::*;
use std::f64::consts::PI;

fn end(sub: &Subpath) -> Pt {
    let a = sub.anchors.last().unwrap();
    (a.x, a.y)
}

fn close(a: Pt, b: Pt) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

#[test]
fn reads_every_command_absolute_and_relative() {
    let d = "M10 10 h10 v10 H5 V5 l1 1 L2 2 c1 1 2 2 3 3 s1 1 2 2 q1 1 2 2 t1 1 \
             C10 10 12 12 14 14 S20 20 22 22 Q25 25 28 28 T30 30 a5 5 0 0 1 10 0 A5 5 0 0 0 30 30";
    let subs = parse_path_data(d).expect("parses");
    assert_eq!(subs.len(), 1);
    let s = &subs[0];
    assert!(!s.closed);
    // Spot-check positions through the relative chain.
    assert!(close((s.anchors[1].x, s.anchors[1].y), (20.0, 10.0)));
    assert!(close((s.anchors[2].x, s.anchors[2].y), (20.0, 20.0)));
    assert!(close((s.anchors[3].x, s.anchors[3].y), (5.0, 20.0)));
    assert!(close((s.anchors[4].x, s.anchors[4].y), (5.0, 5.0)));
    assert!(close((s.anchors[5].x, s.anchors[5].y), (6.0, 6.0)));
    assert!(close(end(s), (30.0, 30.0)));
    // Lines carry no handles, curves do.
    assert!(s.anchors[1].handle_in.is_none());
    assert!(s.anchors[7].handle_in.is_some());
}

#[test]
fn packed_arc_flags_and_run_together_numbers() {
    let subs = parse_path_data("M0 0a1 1 0 011 1l.5.5-1-1").expect("parses");
    let s = &subs[0];
    // Arc to (1, 1), then (1.5, 1.5), then (0.5, 0.5).
    assert!(close(end(s), (0.5, 0.5)));
}

#[test]
fn compound_paths_become_one_subpath_each() {
    let subs = parse_path_data("M0 0 L10 0 L10 10 Z M20 20 L30 20 m5 5 l1 0").expect("parses");
    assert_eq!(subs.len(), 3);
    assert!(subs[0].closed);
    assert_eq!(subs[0].anchors.len(), 3);
    assert!(!subs[1].closed);
    // `m` after an open subpath is relative to its last point.
    assert!(close(
        (subs[2].anchors[0].x, subs[2].anchors[0].y),
        (35.0, 25.0)
    ));
}

#[test]
fn explicit_closing_segment_folds_into_z() {
    let subs = parse_path_data("M0 0 L10 0 L10 10 L0 0 Z").unwrap();
    assert_eq!(subs[0].anchors.len(), 3);
    assert!(subs[0].closed);
}

#[test]
fn command_after_z_restarts_at_the_subpath_start() {
    let subs = parse_path_data("M5 5 L10 5 L10 10 Z l2 0").unwrap();
    assert_eq!(subs.len(), 2);
    assert!(close(
        (subs[1].anchors[0].x, subs[1].anchors[0].y),
        (5.0, 5.0)
    ));
    assert!(close(end(&subs[1]), (7.0, 5.0)));
}

#[test]
fn malformed_data_returns_what_drew() {
    assert!(parse_path_data("").is_none());
    assert!(parse_path_data("garbage").is_none());
    let subs = parse_path_data("M0 0 L5 5 L oops").unwrap();
    assert_eq!(subs[0].anchors.len(), 2);
}

#[test]
fn quadratics_are_elevated_exactly() {
    let subs = parse_path_data("M0 0 Q10 20 20 0").unwrap();
    let s = &subs[0];
    let c = segment_controls(s, 0).unwrap();
    // Exact quadratic point at t: (1-t)^2 p0 + 2t(1-t) q + t^2 p2.
    for i in 0..=10 {
        let t = i as f64 / 10.0;
        let q = (
            2.0 * t * (1.0 - t) * 10.0 + t * t * 20.0,
            2.0 * t * (1.0 - t) * 20.0,
        );
        assert!(close(cubic_point(c, t), q), "t={t}");
    }
}

/// Max distance of the cubics from the true ellipse, measured in the
/// unit-circle frame (radial error × the larger radius bounds the true
/// distance).
fn arc_error(cx: f64, cy: f64, rx: f64, ry: f64, rot_deg: f64, a0: f64, a1: f64) -> f64 {
    let rot = rot_deg.to_radians();
    let (s, c) = rot.sin_cos();
    let at = |a: f64| {
        let (x, y) = (rx * a.cos(), ry * a.sin());
        (cx + c * x - s * y, cy + s * x + c * y)
    };
    let p0 = at(a0);
    let p1 = at(a1);
    let large = (a1 - a0).abs() > PI;
    let sweep = a1 > a0;
    let cubics = arc_to_cubics(p0, rx, ry, rot_deg, large, sweep, p1);
    let mut start = p0;
    let mut worst: f64 = 0.0;
    for [c1, c2, e] in cubics {
        for i in 0..=64 {
            let q = cubic_point([start, c1, c2, e], i as f64 / 64.0);
            let (dx, dy) = (q.0 - cx, q.1 - cy);
            let (ux, uy) = ((c * dx + s * dy) / rx, (-s * dx + c * dy) / ry);
            let radial = (ux.hypot(uy) - 1.0).abs();
            worst = worst.max(radial * rx.max(ry));
        }
        start = e;
    }
    assert!(close(start, p1), "last cubic ends on the arc end");
    worst
}

#[test]
fn arc_to_cubic_error_stays_under_a_quarter_pixel() {
    let cases = [
        (0.0, 0.0, 10.0, 10.0, 0.0, 0.0, PI / 2.0),
        (50.0, 20.0, 300.0, 120.0, 30.0, 0.2, 2.9),
        (0.0, 0.0, 5000.0, 5000.0, 0.0, 0.0, 1.5 * PI),
        (10.0, -4.0, 800.0, 40.0, 75.0, 3.0, 0.5),
        (0.0, 0.0, 0.5, 0.25, 10.0, 0.0, PI),
    ];
    for (cx, cy, rx, ry, rot, a0, a1) in cases {
        let err = arc_error(cx, cy, rx, ry, rot, a0, a1);
        assert!(err < 0.25, "arc r=({rx},{ry}) error {err}");
    }
}

#[test]
fn arc_with_too_small_radii_scales_up_and_zero_radius_is_a_line() {
    // Radius 1 cannot span a chord of 10: SVG scales it to 5.
    let c = arc_to_cubics((0.0, 0.0), 1.0, 1.0, 0.0, false, true, (10.0, 0.0));
    assert!(!c.is_empty());
    let mut lowest: f64 = 0.0;
    let mut start = (0.0, 0.0);
    for [c1, c2, e] in &c {
        for i in 0..=16 {
            lowest = lowest.min(cubic_point([start, *c1, *c2, *e], i as f64 / 16.0).1);
        }
        start = *e;
    }
    assert!(
        (lowest + 5.0).abs() < 0.05,
        "half circle of radius 5: {lowest}"
    );
    let line = arc_to_cubics((0.0, 0.0), 0.0, 4.0, 0.0, false, true, (3.0, 4.0));
    assert_eq!(line, vec![[(0.0, 0.0), (3.0, 4.0), (3.0, 4.0)]]);
}

#[test]
fn anchors_round_trip_through_d() {
    let d = "M0 0 C10 0 20 10 20 20 L0 20 Z M40 40 C50 40 60 50 60 60";
    let subs = parse_path_data(d).unwrap();
    let again = parse_path_data(&subpaths_to_path_data(&subs)).unwrap();
    assert_eq!(subs, again);
}
