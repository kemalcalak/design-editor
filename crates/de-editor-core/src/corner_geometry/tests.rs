use super::*;
use crate::vector_edit::{parse_path_data, subpaths_bounds};

fn close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

fn pts(sub: &Subpath) -> Vec<Pt> {
    sub.anchors.iter().map(|a| (a.x, a.y)).collect()
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The figma-squircle numbers for one 90° corner, `r = 10`, `s = 0.6`,
/// unconstrained: p = 16, a = 5.6005, b = 2.8003, c = 2.1391, d = 1.0899,
/// arc section 4.3702.
#[test]
fn a_right_angle_matches_figma_squircle() {
    let sub = rect_outline(100.0, 100.0, [10.0; 4], 0.6).unwrap();
    // Top-right corner: P1 on the top edge 16 from the vertex.
    let tr = sub
        .anchors
        .iter()
        .position(|a| close(a.x, 84.0, 1e-9) && close(a.y, 0.0, 1e-9))
        .expect("P1 at p = 16");
    let p1 = &sub.anchors[tr];
    let h = p1.handle_out.as_ref().unwrap();
    assert!(close(h.x, 5.6005, 1e-3) && close(h.y, 0.0, 1e-9));
    let p4 = &sub.anchors[tr + 1];
    assert!(close(p4.x, 100.0 - 5.4601, 1e-3), "{}", p4.x);
    assert!(close(p4.y, 1.0899, 1e-3), "{}", p4.y);
    // P3 = P1 + (a + b) along the edge.
    let hin = p4.handle_in.as_ref().unwrap();
    assert!(close(p4.x + hin.x, 84.0 + 8.4008, 1e-3));
    assert!(close(p4.y + hin.y, 0.0, 1e-9));
    // The arc spans 36° and lands mirrored on the right edge.
    let q4 = &sub.anchors[tr + 2];
    assert!(close(q4.x, 100.0 - 1.0899, 1e-3) && close(q4.y, 5.4601, 1e-3));
    assert!(close(q4.x - p4.x, 4.3702, 1e-3));
    let q1 = &sub.anchors[tr + 3];
    assert!(close(q1.x, 100.0, 1e-9) && close(q1.y, 16.0, 1e-9));
}

#[test]
fn zero_smoothing_is_a_circular_arc() {
    let sub = rect_outline(100.0, 60.0, [12.0; 4], 0.0).unwrap();
    let ring = flatten_outline(&sub, 16);
    // Every sampled point of the top-left corner lies on the circle.
    let centre = (12.0, 12.0);
    for p in ring.iter().filter(|p| p.0 < 12.0 && p.1 < 12.0) {
        assert!(close(dist(*p, centre), 12.0, 0.01), "{p:?}");
    }
    // Plain corners: two anchors per corner.
    assert_eq!(sub.anchors.len(), 8);
}

#[test]
fn every_join_is_tangent_continuous() {
    for s in [0.0, 0.3, 0.6, 1.0] {
        let sub = rect_outline(120.0, 80.0, [8.0, 20.0, 0.0, 30.0], s).unwrap();
        for a in &sub.anchors {
            if let (Some(hi), Some(ho)) = (&a.handle_in, &a.handle_out) {
                let cross = hi.x * ho.y - hi.y * ho.x;
                let dot = hi.x * ho.x + hi.y * ho.y;
                let scale = hi.x.hypot(hi.y) * ho.x.hypot(ho.y);
                assert!(
                    cross.abs() <= 1e-9 * scale.max(1.0),
                    "kink at {a:?} (s = {s})"
                );
                assert!(dot < 0.0, "handles point the same way at {a:?}");
            }
        }
    }
}

#[test]
fn a_tight_box_trades_smoothing_before_radius() {
    // 100 × 40, r = 10: a corner may use 20 of the short edge, so s = 1
    // just fits (p = 20).
    let fits = rect_outline(100.0, 40.0, [10.0; 4], 1.0).unwrap();
    assert!(fits
        .anchors
        .iter()
        .any(|a| close(a.x, 0.0, 1e-9) && close(a.y, 20.0, 1e-9)));
    // 100 × 30: only 15 is left, so smoothing drops to 0.5 (p = 15).
    let tight = rect_outline(100.0, 30.0, [10.0; 4], 1.0).unwrap();
    assert!(tight
        .anchors
        .iter()
        .any(|a| close(a.x, 0.0, 1e-9) && close(a.y, 15.0, 1e-9)));
    // A square whose radius already fills the side keeps no smoothing:
    // four quarter arcs meeting mid-side, merged into four anchors.
    let circle = rect_outline(20.0, 20.0, [10.0; 4], 0.8).unwrap();
    assert_eq!(circle.anchors.len(), 4);
    for p in flatten_outline(&circle, 16) {
        assert!(close(dist(p, (10.0, 10.0)), 10.0, 0.01), "{p:?}");
    }
    // An oversized radius is capped by the edge too.
    let capped = rect_outline(20.0, 20.0, [50.0; 4], 0.0).unwrap();
    assert_eq!(pts(&capped).len(), 4);
}

#[test]
fn a_rect_outline_fills_its_box_exactly() {
    for (radii, s) in [
        ([10.0; 4], 0.6),
        ([0.0, 40.0, 5.0, 25.0], 1.0),
        ([100.0; 4], 0.5),
    ] {
        let sub = rect_outline(90.0, 50.0, radii, s).unwrap();
        let d = outline_path_data(&[sub]);
        let parsed = parse_path_data(&d).unwrap();
        let (x, y, w, h) = subpaths_bounds(&parsed);
        assert!(close(x, 0.0, 1e-3) && close(y, 0.0, 1e-3), "{d}");
        assert!(close(w, 90.0, 1e-3) && close(h, 50.0, 1e-3), "{d}");
    }
}

#[test]
fn smoothing_is_mirror_symmetric() {
    let sub = rect_outline(60.0, 60.0, [15.0; 4], 0.7).unwrap();
    let ring = flatten_outline(&sub, 8);
    for p in &ring {
        let mirrored = (60.0 - p.0, p.1);
        let nearest = ring
            .iter()
            .map(|q| dist(*q, mirrored))
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 1e-6, "{p:?}");
    }
}

#[test]
fn other_angles_round_on_the_plain_circle() {
    // Equilateral triangle: interior 60°, turn 120°; t = r·tan(60°).
    let h = 3f64.sqrt() / 2.0 * 100.0;
    let tri = [(0.0, h), (50.0, 0.0), (100.0, h)];
    let r = 6.0;
    let sub = polygon_outline(&tri, r, 0.0).unwrap();
    let t = r * 3f64.sqrt();
    // The apex's arc starts t along both edges.
    let apex = (50.0, 0.0);
    assert!(sub
        .anchors
        .iter()
        .any(|a| close(dist((a.x, a.y), apex), t, 1e-9) && a.x < 50.0));
    // Arc samples near the apex sit r from the incentre-side centre.
    let centre = (50.0, r.hypot(t));
    for p in flatten_outline(&sub, 16).iter().filter(|p| p.1 < 12.0) {
        assert!(close(dist(*p, centre), r, 0.01), "{p:?}");
    }
    // Smoothing at a sharp corner still produces a finite, closed outline.
    let smooth = polygon_outline(&tri, r, 1.0).unwrap();
    assert!(smooth.closed);
    assert!(smooth
        .anchors
        .iter()
        .all(|a| a.x.is_finite() && a.y.is_finite()));
}

#[test]
fn open_ends_and_curved_joins_stay_sharp() {
    let mut sub = Subpath::polyline(&[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)]);
    let rounded = round_corners(&sub, &[9.0; 4], 0.0);
    assert_eq!(
        rounded.anchors.first().map(|a| (a.x, a.y)),
        Some((0.0, 0.0))
    );
    assert_eq!(
        rounded.anchors.last().map(|a| (a.x, a.y)),
        Some((0.0, 50.0))
    );
    assert_eq!(rounded.anchors.len(), 2 + 2 * 2);
    // A handle on either side of a vertex keeps it as it is.
    sub.anchors[2].handle_out = Some(jian_ops_schema::node::PenPathHandle { x: -10.0, y: 0.0 });
    let partly = round_corners(&sub, &[9.0; 4], 0.0);
    assert!(partly.anchors.iter().any(|a| a.x == 50.0 && a.y == 50.0));
    assert_eq!(partly.anchors.len(), 2 + 2 + 1);
}

#[test]
fn nothing_to_round_returns_the_input() {
    let sub = rect_outline(10.0, 10.0, [0.0; 4], 0.5).unwrap();
    assert_eq!(
        pts(&sub),
        vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
    );
    assert!(rect_outline(0.0, 10.0, [2.0; 4], 0.5).is_none());
    assert!(polygon_outline(&[(0.0, 0.0), (1.0, 1.0)], 2.0, 0.0).is_none());
    assert_eq!(clamp_smoothing(f64::NAN), 0.0);
    assert_eq!(clamp_smoothing(7.0), 1.0);
    assert!(!has_smoothing(0.0) && has_smoothing(0.6));
}

#[test]
fn path_data_writes_lines_curves_and_a_close() {
    let sub = rect_outline(40.0, 20.0, [5.0; 4], 0.5).unwrap();
    let d = outline_path_data(&[sub]);
    assert!(d.starts_with("M "), "{d}");
    assert!(d.ends_with(" Z"), "{d}");
    assert!(d.contains(" L ") && d.contains(" C "), "{d}");
    assert!(!d.contains("NaN") && !d.contains("inf"), "{d}");
    // Plain square corners keep straight closing edges.
    let square = rect_outline(4.0, 4.0, [0.0; 4], 0.0).unwrap();
    assert_eq!(outline_path_data(&[square]), "M 0 0 L 4 0 L 4 4 L 0 4 Z");
}
