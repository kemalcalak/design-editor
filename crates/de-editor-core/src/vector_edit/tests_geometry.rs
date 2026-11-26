//! Split, heal, join, reverse and snapping.

use super::*;
use std::collections::BTreeSet;

const CURVE: [Pt; 4] = [(0.0, 0.0), (0.0, 55.0), (45.0, 100.0), (100.0, 100.0)];

/// Max distance from samples of `a` to the nearest point of the chain `b`.
fn deviation(a: &[[Pt; 4]], b: &[[Pt; 4]]) -> f64 {
    let mut worst: f64 = 0.0;
    for c in a {
        for i in 0..=40 {
            let p = cubic_point(*c, i as f64 / 40.0);
            let d = b
                .iter()
                .map(|s| nearest_on_segment(*s, p).1)
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(d);
        }
    }
    worst
}

fn segments(sub: &Subpath) -> Vec<[Pt; 4]> {
    (0..segment_count(sub))
        .filter_map(|s| segment_controls(sub, s))
        .collect()
}

fn curve_subpath() -> Subpath {
    let mut a = corner(CURVE[0]);
    a.handle_out = Some(jian_ops_schema::node::PenPathHandle { x: 0.0, y: 55.0 });
    let mut b = corner(CURVE[3]);
    b.handle_in = Some(jian_ops_schema::node::PenPathHandle { x: -55.0, y: 0.0 });
    Subpath {
        anchors: vec![a, b],
        closed: false,
    }
}

#[test]
fn split_cubic_halves_trace_the_original() {
    let (l, r) = split_cubic(CURVE, 0.3);
    assert!(deviation(&[l, r], &[CURVE]) < 1e-6);
    assert!(deviation(&[CURVE], &[l, r]) < 1e-6);
}

#[test]
fn segment_split_keeps_the_curve() {
    let mut sub = curve_subpath();
    let before = segments(&sub);
    let at = split_segment(&mut sub, 0, 0.42).unwrap();
    assert_eq!(at, 1);
    assert_eq!(sub.anchors.len(), 3);
    assert!(deviation(&segments(&sub), &before) < 1e-6);
    assert!(deviation(&before, &segments(&sub)) < 1e-6);
}

#[test]
fn straight_segment_split_adds_a_corner() {
    let mut sub = Subpath::polyline(&[(0.0, 0.0), (10.0, 0.0)]);
    split_segment(&mut sub, 0, 0.5).unwrap();
    let mid = &sub.anchors[1];
    assert_eq!((mid.x, mid.y), (5.0, 0.0));
    assert!(mid.handle_in.is_none() && mid.handle_out.is_none());
}

#[test]
fn deleting_a_middle_point_heals_the_curve() {
    let mut sub = curve_subpath();
    split_segment(&mut sub, 0, 0.5).unwrap();
    let pts: BTreeSet<PointRef> = [PointRef::new(0, 1)].into();
    let healed = delete_points(&[sub], &pts);
    assert_eq!(healed.len(), 1);
    assert_eq!(healed[0].anchors.len(), 2);
    let dev = deviation(&[CURVE], &segments(&healed[0]));
    assert!(dev < 0.5, "healed curve drifted {dev}");
}

#[test]
fn deleting_from_straight_runs_and_ends() {
    let sub = Subpath::polyline(&[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0), (20.0, 0.0)]);
    let pts: BTreeSet<PointRef> = [PointRef::new(0, 1), PointRef::new(0, 3)].into();
    let out = delete_points(&[sub], &pts);
    let a = &out[0].anchors;
    assert_eq!(a.len(), 2);
    assert!(
        a[0].handle_out.is_none() && a[1].handle_in.is_none(),
        "stays straight"
    );
    assert_eq!((a[1].x, a[1].y), (10.0, 0.0));
    // A subpath left with one anchor disappears.
    let lone = Subpath::polyline(&[(0.0, 0.0), (1.0, 1.0)]);
    let gone = delete_points(&[lone], &[PointRef::new(0, 0)].into());
    assert!(gone.is_empty());
}

#[test]
fn closed_path_delete_heals_across_the_wrap() {
    let sub = Subpath {
        anchors: Subpath::polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]).anchors,
        closed: true,
    };
    let out = delete_points(&[sub], &[PointRef::new(0, 0)].into());
    assert_eq!(out[0].anchors.len(), 3);
    assert!(out[0].closed);
}

#[test]
fn join_two_subpaths_and_close_one() {
    let mut subs = vec![
        Subpath::polyline(&[(0.0, 0.0), (10.0, 0.0)]),
        Subpath::polyline(&[(30.0, 0.0), (20.0, 0.0)]),
    ];
    // End of the first to the END of the second: the second reverses.
    let at = join_endpoints(&mut subs, PointRef::new(0, 1), PointRef::new(1, 1), false).unwrap();
    assert_eq!(at, 0);
    assert_eq!(subs.len(), 1);
    let xs: Vec<f64> = subs[0].anchors.iter().map(|a| a.x).collect();
    assert_eq!(xs, vec![0.0, 10.0, 20.0, 30.0]);
    // Its two ends close it.
    join_endpoints(&mut subs, PointRef::new(0, 0), PointRef::new(0, 3), false).unwrap();
    assert!(subs[0].closed);
    assert_eq!(subs[0].anchors.len(), 4);
    // Closed subpaths have no end points left.
    assert!(join_endpoints(&mut subs, PointRef::new(0, 0), PointRef::new(0, 3), false).is_none());
}

#[test]
fn join_with_merge_fuses_coincident_ends() {
    let mut subs = vec![
        Subpath::polyline(&[(0.0, 0.0), (10.0, 0.0)]),
        Subpath::polyline(&[(10.0, 0.0), (10.0, 10.0)]),
    ];
    join_endpoints(&mut subs, PointRef::new(0, 1), PointRef::new(1, 0), true).unwrap();
    assert_eq!(subs[0].anchors.len(), 3);
}

#[test]
fn reverse_keeps_geometry_and_start() {
    let mut sub = curve_subpath();
    let before = segments(&sub);
    reverse_subpath(&mut sub);
    assert!(deviation(&before, &segments(&sub)) < 1e-6);
    let mut closed = Subpath {
        anchors: Subpath::polyline(&[(0.0, 0.0), (10.0, 0.0), (5.0, 5.0)]).anchors,
        closed: true,
    };
    reverse_subpath(&mut closed);
    assert_eq!((closed.anchors[0].x, closed.anchors[1].x), (0.0, 5.0));
}

#[test]
fn shift_constrains_to_45_degree_steps() {
    let p = constrain_45((0.0, 0.0), (10.0, 1.0));
    assert!((p.1).abs() < 1e-9 && (p.0 - 10.0).abs() < 1e-9);
    let p = constrain_45((0.0, 0.0), (10.0, 9.0));
    assert!((p.0 - p.1).abs() < 1e-9, "diagonal: {p:?}");
    let p = constrain_45((5.0, 5.0), (5.5, -20.0));
    assert!((p.0 - 5.0).abs() < 1e-9 && p.1 < 0.0);
}

#[test]
fn snapping_prefers_points_then_aligns_axes() {
    let cands = [(100.0, 100.0), (50.0, 0.0)];
    let r = snap_point((102.0, 97.0), &cands, 4.0);
    assert!(r.on_point);
    assert_eq!(r.point, (100.0, 100.0));
    let r = snap_point((52.0, 60.0), &cands, 4.0);
    assert!(!r.on_point && r.snapped);
    assert_eq!(r.point, (50.0, 60.0));
    assert_eq!(r.guides.len(), 1);
    let r = snap_point((300.0, 300.0), &cands, 4.0);
    assert!(!r.snapped);
    assert_eq!(snap_to_pixel_grid((1.4, 2.6)), (1.0, 3.0));
}
