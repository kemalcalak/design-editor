//! Freehand curve fitting for the Pencil tool (#88).
//!
//! Philip J. Schneider, "An Algorithm for Automatically Fitting Digitized
//! Curves" (Graphics Gems, 1990): fit one cubic bezier to the sampled
//! points by least squares over a chord-length parameterisation, refine
//! the parameters with Newton-Raphson, and split at the worst point when
//! the error stays above the tolerance. Joints share one tangent, so the
//! fitted path is G1-continuous and its anchors read as smooth points in
//! the pen tool's point editor.

use crate::path_edit::infer_path_anchor_point_type;
use jian_ops_schema::node::{PenPathAnchor, PenPathHandle};

/// Newton-Raphson reparameterisation passes before a segment is split.
const MAX_REPARAM_ITERATIONS: usize = 4;
/// Recursion guard — a pathological sample set still terminates.
const MAX_SPLIT_DEPTH: usize = 24;
/// Consecutive samples closer than this (doc px) are treated as one.
const DUPLICATE_EPSILON: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq)]
struct V {
    x: f64,
    y: f64,
}

impl V {
    fn add(self, o: V) -> V {
        V {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }
    fn sub(self, o: V) -> V {
        V {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }
    fn scale(self, k: f64) -> V {
        V {
            x: self.x * k,
            y: self.y * k,
        }
    }
    fn dot(self, o: V) -> f64 {
        self.x * o.x + self.y * o.y
    }
    fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    fn dist(self, o: V) -> f64 {
        self.sub(o).len()
    }
    /// Unit vector, or `None` for a zero-length one.
    fn unit(self) -> Option<V> {
        let l = self.len();
        (l > f64::EPSILON).then(|| self.scale(1.0 / l))
    }
}

type Bezier = [V; 4];

/// Fit `points` (document coords, in drawing order) with cubic beziers
/// whose distance from every sample stays under `tolerance` (doc px), and
/// return them as path anchors: positions are absolute, handles are
/// offsets from their anchor (the `PenPathAnchor` convention). Fewer than
/// two distinct points yield no anchors; exactly two yield a straight
/// segment.
pub fn fit_freehand_path(points: &[(f64, f64)], tolerance: f64) -> Vec<PenPathAnchor> {
    let pts = dedup(points);
    if pts.len() < 2 {
        return Vec::new();
    }
    if pts.len() == 2 {
        return pts.iter().map(|p| corner_anchor(*p)).collect();
    }
    let tolerance = if tolerance.is_finite() {
        tolerance.max(0.01)
    } else {
        1.0
    };
    let error_sq = tolerance * tolerance;
    let n = pts.len();
    let t_start = end_tangent(&pts, 0, 1);
    let t_end = end_tangent(&pts, n - 1, n - 2);
    let mut curves = Vec::new();
    fit_cubic(&pts, t_start, t_end, error_sq, 0, &mut curves);
    to_anchors(&curves)
}

fn dedup(points: &[(f64, f64)]) -> Vec<V> {
    let mut out: Vec<V> = Vec::with_capacity(points.len());
    for &(x, y) in points {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let p = V { x, y };
        if out
            .last()
            .is_some_and(|last| last.dist(p) <= DUPLICATE_EPSILON)
        {
            continue;
        }
        out.push(p);
    }
    out
}

fn corner_anchor(p: V) -> PenPathAnchor {
    PenPathAnchor {
        x: p.x,
        y: p.y,
        handle_in: None,
        handle_out: None,
        point_type: Some(jian_ops_schema::node::PenPathPointType::Corner),
    }
}

/// Unit tangent at endpoint `at`, pointing toward its neighbour.
fn end_tangent(pts: &[V], at: usize, toward: usize) -> V {
    pts[toward]
        .sub(pts[at])
        .unit()
        .unwrap_or(V { x: 1.0, y: 0.0 })
}

/// Unit tangent at an interior split point, pointing back along the
/// stroke (Schneider's `ComputeCenterTangent`).
fn center_tangent(pts: &[V], at: usize) -> V {
    pts[at - 1]
        .sub(pts[at + 1])
        .unit()
        .or_else(|| pts[at - 1].sub(pts[at]).unit())
        .unwrap_or(V { x: -1.0, y: 0.0 })
}

fn fit_cubic(pts: &[V], t1: V, t2: V, error_sq: f64, depth: usize, out: &mut Vec<Bezier>) {
    let first = pts[0];
    let last = pts[pts.len() - 1];
    if pts.len() == 2 {
        let d = first.dist(last) / 3.0;
        out.push([first, first.add(t1.scale(d)), last.add(t2.scale(d)), last]);
        return;
    }
    let mut u = chord_length_parameterize(pts);
    let mut bez = generate_bezier(pts, &u, t1, t2);
    let (max_err, mut split) = max_error(pts, &bez, &u);
    if max_err < error_sq {
        out.push(bez);
        return;
    }
    // Close enough that reparameterising is worth a few passes.
    if max_err < error_sq * 4.0 {
        for _ in 0..MAX_REPARAM_ITERATIONS {
            u = reparameterize(pts, &u, &bez);
            bez = generate_bezier(pts, &u, t1, t2);
            let (err, at) = max_error(pts, &bez, &u);
            if err < error_sq {
                out.push(bez);
                return;
            }
            split = at;
        }
    }
    if depth >= MAX_SPLIT_DEPTH {
        out.push(bez);
        return;
    }
    let tc = center_tangent(pts, split);
    fit_cubic(&pts[..=split], t1, tc, error_sq, depth + 1, out);
    fit_cubic(&pts[split..], tc.scale(-1.0), t2, error_sq, depth + 1, out);
}

fn chord_length_parameterize(pts: &[V]) -> Vec<f64> {
    let mut u = Vec::with_capacity(pts.len());
    u.push(0.0);
    for i in 1..pts.len() {
        u.push(u[i - 1] + pts[i].dist(pts[i - 1]));
    }
    let total = *u.last().unwrap_or(&0.0);
    let n = (pts.len() - 1).max(1) as f64;
    for (i, value) in u.iter_mut().enumerate() {
        *value = if total > 0.0 {
            *value / total
        } else {
            i as f64 / n
        };
    }
    u
}

/// Least-squares handle lengths for fixed end tangents.
fn generate_bezier(pts: &[V], u: &[f64], t1: V, t2: V) -> Bezier {
    let first = pts[0];
    let last = pts[pts.len() - 1];
    let mut c = [[0.0_f64; 2]; 2];
    let mut x = [0.0_f64; 2];
    for (p, &t) in pts.iter().zip(u) {
        let a0 = t1.scale(b1(t));
        let a1 = t2.scale(b2(t));
        c[0][0] += a0.dot(a0);
        c[0][1] += a0.dot(a1);
        c[1][1] += a1.dot(a1);
        let fixed = first.scale(b0(t) + b1(t)).add(last.scale(b2(t) + b3(t)));
        let tmp = p.sub(fixed);
        x[0] += a0.dot(tmp);
        x[1] += a1.dot(tmp);
    }
    c[1][0] = c[0][1];
    let det_c0_c1 = c[0][0] * c[1][1] - c[1][0] * c[0][1];
    let det_c0_x = c[0][0] * x[1] - c[1][0] * x[0];
    let det_x_c1 = x[0] * c[1][1] - x[1] * c[0][1];
    let (alpha_l, alpha_r) = if det_c0_c1.abs() > f64::EPSILON {
        (det_x_c1 / det_c0_c1, det_c0_x / det_c0_c1)
    } else {
        (0.0, 0.0)
    };
    let seg_len = first.dist(last);
    let eps = 1e-6 * seg_len;
    // A degenerate or backwards solve falls back to Wu/Barsky's
    // third-of-the-chord heuristic.
    let (alpha_l, alpha_r) = if alpha_l < eps || alpha_r < eps {
        (seg_len / 3.0, seg_len / 3.0)
    } else {
        (alpha_l, alpha_r)
    };
    [
        first,
        first.add(t1.scale(alpha_l)),
        last.add(t2.scale(alpha_r)),
        last,
    ]
}

fn reparameterize(pts: &[V], u: &[f64], bez: &Bezier) -> Vec<f64> {
    pts.iter()
        .zip(u)
        .map(|(p, &t)| newton_root(bez, *p, t))
        .collect()
}

/// One Newton-Raphson step toward the parameter closest to `p`.
fn newton_root(bez: &Bezier, p: V, t: f64) -> f64 {
    let q = eval(bez, t);
    let d1 = derivative(bez, t);
    let d2 = second_derivative(bez, t);
    let diff = q.sub(p);
    let numerator = diff.dot(d1);
    let denominator = d1.dot(d1) + diff.dot(d2);
    if denominator.abs() < f64::EPSILON {
        return t;
    }
    (t - numerator / denominator).clamp(0.0, 1.0)
}

/// Largest squared sample error and the interior index it sits at.
fn max_error(pts: &[V], bez: &Bezier, u: &[f64]) -> (f64, usize) {
    let mut split = pts.len() / 2;
    let mut max = 0.0;
    for i in 1..pts.len() - 1 {
        let d = eval(bez, u[i]).sub(pts[i]);
        let e = d.dot(d);
        if e >= max {
            max = e;
            split = i;
        }
    }
    (max, split.clamp(1, pts.len() - 2))
}

fn b0(t: f64) -> f64 {
    let m = 1.0 - t;
    m * m * m
}
fn b1(t: f64) -> f64 {
    let m = 1.0 - t;
    3.0 * t * m * m
}
fn b2(t: f64) -> f64 {
    3.0 * t * t * (1.0 - t)
}
fn b3(t: f64) -> f64 {
    t * t * t
}

fn eval(b: &Bezier, t: f64) -> V {
    b[0].scale(b0(t))
        .add(b[1].scale(b1(t)))
        .add(b[2].scale(b2(t)))
        .add(b[3].scale(b3(t)))
}

fn derivative(b: &Bezier, t: f64) -> V {
    let m = 1.0 - t;
    b[1].sub(b[0])
        .scale(3.0 * m * m)
        .add(b[2].sub(b[1]).scale(6.0 * m * t))
        .add(b[3].sub(b[2]).scale(3.0 * t * t))
}

fn second_derivative(b: &Bezier, t: f64) -> V {
    let m = 1.0 - t;
    b[2].sub(b[1].scale(2.0))
        .add(b[0])
        .scale(6.0 * m)
        .add(b[3].sub(b[2].scale(2.0)).add(b[1]).scale(6.0 * t))
}

/// Bezier chain → anchors with relative handles; each point type is the
/// one the pen tool's editor would infer from its handles.
fn to_anchors(curves: &[Bezier]) -> Vec<PenPathAnchor> {
    let Some(first) = curves.first() else {
        return Vec::new();
    };
    let handle = |from: V, to: V| {
        let d = to.sub(from);
        Some(PenPathHandle { x: d.x, y: d.y })
    };
    let mut anchors = Vec::with_capacity(curves.len() + 1);
    anchors.push(PenPathAnchor {
        x: first[0].x,
        y: first[0].y,
        handle_in: None,
        handle_out: handle(first[0], first[1]),
        point_type: None,
    });
    for (i, c) in curves.iter().enumerate() {
        anchors.push(PenPathAnchor {
            x: c[3].x,
            y: c[3].y,
            handle_in: handle(c[3], c[2]),
            handle_out: curves.get(i + 1).and_then(|next| handle(next[0], next[1])),
            point_type: None,
        });
    }
    for anchor in &mut anchors {
        anchor.point_type = Some(infer_path_anchor_point_type(anchor));
    }
    anchors
}

#[cfg(test)]
#[path = "pencil_fit_tests.rs"]
mod tests;
