//! Simplify (#388): remove anchors the path does not need.
//!
//! An anchor goes when the path without it stays within `tolerance` doc
//! px of the original: a corner on a straight run, a duplicate point, or
//! a smooth point between two curves that one cubic can replace (the
//! merged cubic inverts a de Casteljau split at the ratio of the middle
//! anchor's handles, so a curve that was split once merges back exactly). Endpoints of open
//! subpaths stay. Runs until nothing more can go.

use super::geometry::{cubic_point, segment_controls};
use super::{Pt, Subpath};
use jian_ops_schema::node::PenPathHandle;

/// Samples per segment when measuring the deviation.
const SAMPLES: usize = 16;

/// `subs` with redundant anchors removed (see the module docs).
/// `tolerance` is in the subpaths' own units; negative is treated as 0.
pub fn simplify_subpaths(subs: &[Subpath], tolerance: f64) -> Vec<Subpath> {
    let tolerance = tolerance.max(0.0);
    subs.iter()
        .map(|s| {
            let mut s = s.clone();
            drop_duplicates(&mut s);
            while remove_one(&mut s, tolerance) {}
            s
        })
        .collect()
}

/// Total anchor count, for reporting what a simplify removed.
pub fn anchor_count(subs: &[Subpath]) -> usize {
    subs.iter().map(|s| s.anchors.len()).sum()
}

fn dist(a: Pt, b: Pt) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Merge consecutive anchors at the same spot (incoming handle of the
/// first, outgoing of the second).
fn drop_duplicates(s: &mut Subpath) {
    let mut i = 0;
    while s.anchors.len() > 2 && i + 1 < s.anchors.len() {
        let (a, b) = (&s.anchors[i], &s.anchors[i + 1]);
        if dist((a.x, a.y), (b.x, b.y)) < 1e-6 {
            let out = s.anchors[i + 1].handle_out.clone();
            s.anchors[i].handle_out = out;
            s.anchors.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

/// Try each removable anchor; drop the first whose removal stays within
/// tolerance. True when one went.
fn remove_one(s: &mut Subpath, tolerance: f64) -> bool {
    let n = s.anchors.len();
    let min = if s.closed { 3 } else { 2 };
    if n <= min {
        return false;
    }
    let candidates: Vec<usize> = if s.closed {
        (0..n).collect()
    } else {
        (1..n - 1).collect()
    };
    for i in candidates {
        let prev_seg = (i + n - 1) % n;
        let (Some(a), Some(b)) = (segment_controls(s, prev_seg), segment_controls(s, i)) else {
            continue;
        };
        let merged = merge(a, b);
        if deviation(&[a, b], merged) <= tolerance {
            let (p, q) = ((i + n - 1) % n, (i + 1) % n);
            s.anchors[p].handle_out = handle(merged[0], merged[1]);
            s.anchors[q].handle_in = handle(merged[3], merged[2]);
            s.anchors.remove(i);
            return true;
        }
    }
    false
}

/// One cubic standing in for `a` then `b`.
fn merge(a: [Pt; 4], b: [Pt; 4]) -> [Pt; 4] {
    let (p0, p3) = (a[0], b[3]);
    let straight = |c: [Pt; 4]| c[1] == c[0] && c[2] == c[3];
    if straight(a) && straight(b) {
        return [p0, p0, p3, p3];
    }
    // A de Casteljau split at `t` leaves the middle anchor's handles in
    // the ratio t : (1 − t); without both handles, use the chords.
    let (h_in, h_out) = (dist(a[2], a[3]), dist(b[0], b[1]));
    let (l1, l2) = if h_in > 1e-9 && h_out > 1e-9 {
        (h_in, h_out)
    } else {
        (dist(a[0], a[3]), dist(b[0], b[3]))
    };
    let t = if l1 + l2 > 1e-9 { l1 / (l1 + l2) } else { 0.5 };
    let t = t.clamp(0.05, 0.95);
    let c1 = (p0.0 + (a[1].0 - p0.0) / t, p0.1 + (a[1].1 - p0.1) / t);
    let c2 = (
        p3.0 + (b[2].0 - p3.0) / (1.0 - t),
        p3.1 + (b[2].1 - p3.1) / (1.0 - t),
    );
    [p0, c1, c2, p3]
}

/// Largest distance from the original segments to the merged cubic.
fn deviation(original: &[[Pt; 4]], merged: [Pt; 4]) -> f64 {
    let dense: Vec<Pt> = (0..=SAMPLES * 4)
        .map(|k| cubic_point(merged, k as f64 / (SAMPLES * 4) as f64))
        .collect();
    let mut worst: f64 = 0.0;
    for seg in original {
        for k in 0..=SAMPLES {
            let p = cubic_point(*seg, k as f64 / SAMPLES as f64);
            let near = dense
                .windows(2)
                .map(|w| point_segment_distance(p, w[0], w[1]))
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(near);
        }
    }
    worst
}

fn point_segment_distance(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-18 {
        return dist(p, a);
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}

fn handle(from: Pt, to: Pt) -> Option<PenPathHandle> {
    let (x, y) = (to.0 - from.0, to.1 - from.1);
    (x.abs() > 1e-9 || y.abs() > 1e-9).then_some(PenPathHandle { x, y })
}

#[cfg(test)]
mod tests {
    use super::super::{corner, split_segment};
    use super::*;

    #[test]
    fn collinear_corners_go_and_real_corners_stay() {
        let s = Subpath::polyline(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.05), (10.0, 10.0)]);
        let out = simplify_subpaths(&[s], 0.1);
        let pts: Vec<Pt> = out[0].anchors.iter().map(|a| (a.x, a.y)).collect();
        assert_eq!(pts, vec![(0.0, 0.0), (10.0, 0.05), (10.0, 10.0)]);
    }

    #[test]
    fn zero_tolerance_keeps_a_bent_run() {
        let s = Subpath::polyline(&[(0.0, 0.0), (5.0, 1.0), (10.0, 0.0)]);
        assert_eq!(simplify_subpaths(&[s], 0.0)[0].anchors.len(), 3);
    }

    #[test]
    fn a_split_curve_merges_back() {
        let mut a = corner((0.0, 0.0));
        a.handle_out = Some(PenPathHandle { x: 30.0, y: 40.0 });
        let mut b = corner((100.0, 0.0));
        b.handle_in = Some(PenPathHandle { x: -30.0, y: 40.0 });
        let mut s = Subpath {
            anchors: vec![a, b],
            closed: false,
        };
        let before = segment_controls(&s, 0).unwrap();
        assert!(split_segment(&mut s, 0, 0.4).is_some());
        assert_eq!(s.anchors.len(), 3);
        let out = simplify_subpaths(&[s], 0.05);
        assert_eq!(out[0].anchors.len(), 2, "merged back into one cubic");
        let after = segment_controls(&out[0], 0).unwrap();
        for (p, q) in before.iter().zip(after.iter()) {
            assert!(dist(*p, *q) < 0.01, "{before:?} vs {after:?}");
        }
    }

    #[test]
    fn duplicates_collapse_and_closed_paths_keep_three() {
        let s = Subpath {
            anchors: vec![
                corner((0.0, 0.0)),
                corner((0.0, 0.0)),
                corner((10.0, 0.0)),
                corner((5.0, 0.0)),
            ],
            closed: true,
        };
        let out = simplify_subpaths(&[s], 100.0);
        assert_eq!(out[0].anchors.len(), 3);
    }
}
