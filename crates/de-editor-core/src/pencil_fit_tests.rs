use super::*;
use jian_ops_schema::node::PenPathPointType;

fn bezier_of(a: &PenPathAnchor, b: &PenPathAnchor) -> Bezier {
    let p0 = V { x: a.x, y: a.y };
    let p3 = V { x: b.x, y: b.y };
    let out = a
        .handle_out
        .as_ref()
        .map_or(p0, |h| p0.add(V { x: h.x, y: h.y }));
    let inn = b
        .handle_in
        .as_ref()
        .map_or(p3, |h| p3.add(V { x: h.x, y: h.y }));
    [p0, out, inn, p3]
}

/// Distance from `p` to the fitted chain, sampled densely.
fn distance_to_chain(anchors: &[PenPathAnchor], p: (f64, f64)) -> f64 {
    let p = V { x: p.0, y: p.1 };
    anchors
        .windows(2)
        .flat_map(|w| {
            let b = bezier_of(&w[0], &w[1]);
            (0..=200).map(move |i| eval(&b, i as f64 / 200.0).dist(p))
        })
        .fold(f64::INFINITY, f64::min)
}

fn circle_arc(samples: usize, radius: f64, sweep: f64) -> Vec<(f64, f64)> {
    (0..samples)
        .map(|i| {
            let a = sweep * i as f64 / (samples - 1) as f64;
            (100.0 + radius * a.cos(), 100.0 + radius * a.sin())
        })
        .collect()
}

#[test]
fn fewer_than_two_distinct_points_fit_nothing() {
    assert!(fit_freehand_path(&[], 2.0).is_empty());
    assert!(fit_freehand_path(&[(5.0, 5.0)], 2.0).is_empty());
    assert!(fit_freehand_path(&[(5.0, 5.0), (5.0, 5.0), (5.0, 5.0)], 2.0).is_empty());
}

#[test]
fn two_points_make_a_straight_corner_segment() {
    let anchors = fit_freehand_path(&[(0.0, 0.0), (10.0, 0.0)], 2.0);
    assert_eq!(anchors.len(), 2);
    assert!(anchors
        .iter()
        .all(|a| a.handle_in.is_none() && a.handle_out.is_none()));
    assert!(anchors
        .iter()
        .all(|a| a.point_type == Some(PenPathPointType::Corner)));
}

#[test]
fn collinear_samples_collapse_to_one_segment() {
    let line: Vec<_> = (0..50).map(|i| (i as f64 * 2.0, i as f64)).collect();
    let anchors = fit_freehand_path(&line, 1.0);
    assert_eq!(anchors.len(), 2, "a straight drag is one bezier");
    assert_eq!((anchors[0].x, anchors[0].y), (0.0, 0.0));
    assert_eq!((anchors[1].x, anchors[1].y), (98.0, 49.0));
}

#[test]
fn every_sample_stays_within_tolerance() {
    let samples = circle_arc(120, 80.0, std::f64::consts::PI * 1.5);
    for tolerance in [0.5, 2.5, 6.0] {
        let anchors = fit_freehand_path(&samples, tolerance);
        assert!(anchors.len() >= 2);
        for p in &samples {
            let d = distance_to_chain(&anchors, *p);
            // Dense sampling of the curve over-estimates the true
            // distance slightly; allow that slack only.
            assert!(
                d <= tolerance + 0.05,
                "sample {p:?} is {d} away (tol {tolerance})"
            );
        }
    }
}

#[test]
fn far_fewer_anchors_than_samples_and_looser_tolerance_means_fewer() {
    let samples = circle_arc(200, 80.0, std::f64::consts::PI * 1.8);
    let tight = fit_freehand_path(&samples, 0.5).len();
    let loose = fit_freehand_path(&samples, 6.0).len();
    assert!(tight < samples.len() / 4, "tight fit kept {tight} anchors");
    assert!(loose <= tight, "loose {loose} vs tight {tight}");
}

#[test]
fn endpoints_are_kept_exactly_and_joints_are_smooth() {
    let samples = circle_arc(150, 60.0, std::f64::consts::PI * 1.9);
    let anchors = fit_freehand_path(&samples, 0.3);
    assert!(anchors.len() > 2, "a near-full circle needs a split");
    let first = anchors.first().unwrap();
    let last = anchors.last().unwrap();
    assert_eq!((first.x, first.y), samples[0]);
    assert_eq!((last.x, last.y), *samples.last().unwrap());
    assert!(first.handle_in.is_none() && last.handle_out.is_none());
    for a in &anchors[1..anchors.len() - 1] {
        let (hi, ho) = (
            a.handle_in.as_ref().unwrap(),
            a.handle_out.as_ref().unwrap(),
        );
        // Collinear, opposite directions: the cross product vanishes.
        let cross = hi.x * ho.y - hi.y * ho.x;
        let scale = hi.x.hypot(hi.y) * ho.x.hypot(ho.y);
        assert!(cross.abs() <= 1e-6 * scale.max(1.0), "joint not G1");
        assert!(
            hi.x * ho.x + hi.y * ho.y < 0.0,
            "handles point the same way"
        );
        assert_ne!(a.point_type, Some(PenPathPointType::Corner));
    }
}

#[test]
fn non_finite_samples_are_ignored() {
    let anchors = fit_freehand_path(
        &[
            (0.0, 0.0),
            (f64::NAN, 1.0),
            (5.0, 5.0),
            (f64::INFINITY, 0.0),
            (10.0, 0.0),
        ],
        1.0,
    );
    assert!(anchors.len() >= 2);
    assert!(anchors.iter().all(|a| a.x.is_finite() && a.y.is_finite()));
}

#[test]
fn a_jittery_back_and_forth_scribble_terminates() {
    let samples: Vec<_> = (0..400)
        .map(|i| {
            let x = (i % 7) as f64 * 3.0;
            let y = (i % 5) as f64 * 2.0;
            (x, y)
        })
        .collect();
    let anchors = fit_freehand_path(&samples, 0.5);
    assert!(anchors.len() >= 2);
    assert!(anchors.iter().all(|a| a.x.is_finite() && a.y.is_finite()));
}
