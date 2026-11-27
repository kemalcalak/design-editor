//! One rounded (and possibly smoothed) vertex: where it sits, which way its
//! edges leave, and the anchors that replace it.

use super::budget::CornerPlan;
use crate::vector_edit::{Pt, Subpath};
use jian_ops_schema::node::{PenPathAnchor, PenPathHandle};
use std::f64::consts::{FRAC_PI_2, PI};

/// Distances below this are one point (path units).
const EPS: f64 = 1e-9;
/// Turns (radians) below this are a straight line through the vertex, and
/// interior angles below it a spike: neither rounds.
const MIN_ANGLE: f64 = 1e-6;

/// A vertex that can round: a sharp anchor between two straight segments.
#[derive(Debug, Clone, Copy)]
pub(super) struct Vertex {
    pub v: Pt,
    /// Unit vector from the vertex back along the incoming edge.
    pub u_in: Pt,
    /// Unit vector from the vertex along the outgoing edge.
    pub u_out: Pt,
    pub len_in: f64,
    pub len_out: f64,
    /// `tan(τ/2)` for the turn `τ`: the plain arc's tangent distance per
    /// unit of radius.
    pub k: f64,
    /// Requested radius.
    pub radius: f64,
}

impl Vertex {
    /// Anchor `i` of `sub` as a roundable vertex with `radius`, or `None`
    /// when it cannot round (an open end, a curved side, no turn, a
    /// non-positive radius).
    pub fn at(sub: &Subpath, i: usize, radius: f64) -> Option<Self> {
        let n = sub.anchors.len();
        if !(radius.is_finite() && radius > 0.0) || n < 3 {
            return None;
        }
        if !sub.closed && (i == 0 || i + 1 == n) {
            return None;
        }
        let prev = &sub.anchors[(i + n - 1) % n];
        let here = &sub.anchors[i];
        let next = &sub.anchors[(i + 1) % n];
        let straight_in =
            !has_handle(prev.handle_out.as_ref()) && !has_handle(here.handle_in.as_ref());
        let straight_out =
            !has_handle(here.handle_out.as_ref()) && !has_handle(next.handle_in.as_ref());
        if !(straight_in && straight_out) {
            return None;
        }
        let v = (here.x, here.y);
        let (u_in, len_in) = unit(sub_pt((prev.x, prev.y), v))?;
        let (u_out, len_out) = unit(sub_pt((next.x, next.y), v))?;
        let interior = dot(u_in, u_out).clamp(-1.0, 1.0).acos();
        let turn = PI - interior;
        if interior < MIN_ANGLE || turn < MIN_ANGLE {
            return None;
        }
        Some(Self {
            v,
            u_in,
            u_out,
            len_in,
            len_out,
            k: (turn / 2.0).tan(),
            radius,
        })
    }

    /// The plain arc's tangent distance at the requested radius.
    pub fn tangent(&self) -> f64 {
        self.radius * self.k
    }
}

/// Push the anchors that replace `vertex` under `plan`.
pub(super) fn push_corner(out: &mut Vec<PenPathAnchor>, vertex: &Vertex, plan: CornerPlan) {
    let t = plan.tangent;
    if t <= EPS {
        out.push(anchor(vertex.v, None, None));
        return;
    }
    let r = t / vertex.k;
    let turn = 2.0 * vertex.k.atan();
    let s = plan.smoothing;
    let p = (1.0 + s) * t;
    let centre = add(
        vertex.v,
        scale(
            unit(add(vertex.u_in, vertex.u_out)).map_or((0.0, 0.0), |u| u.0),
            r.hypot(t),
        ),
    );
    let t1 = add(vertex.v, scale(vertex.u_in, t));
    let t2 = add(vertex.v, scale(vertex.u_out, t));
    let alpha = turn * s / 2.0;
    let sweep = turn * (1.0 - s);

    // Arc ends and the transition handles into / out of them.
    let (arc_start, arc_end, lead_in, lead_out) = if alpha > MIN_ANGLE {
        let l = r * (alpha / 2.0).tan();
        let along = r * alpha.sin();
        let inward = r * (1.0 - alpha.cos());
        let ab = (p - t) + (along - l * alpha.cos());
        let (b, a) = (ab / 3.0, 2.0 * ab / 3.0);
        // Incoming side travels towards the vertex (−u_in).
        let d1 = scale(vertex.u_in, -1.0);
        let n1 = unit(sub_pt(centre, t1)).map_or((0.0, 0.0), |u| u.0);
        let p1 = add(vertex.v, scale(vertex.u_in, p));
        let p2 = add(p1, scale(d1, a));
        let p3 = add(p1, scale(d1, a + b));
        let p4 = add(add(t1, scale(d1, along)), scale(n1, inward));
        // Outgoing side travels away from the vertex (+u_out).
        let d2 = vertex.u_out;
        let n2 = unit(sub_pt(centre, t2)).map_or((0.0, 0.0), |u| u.0);
        let q1 = add(vertex.v, scale(vertex.u_out, p));
        let q2 = sub_pt(q1, scale(d2, a));
        let q3 = sub_pt(q1, scale(d2, a + b));
        let q4 = add(sub_pt(t2, scale(d2, along)), scale(n2, inward));
        out.push(anchor(p1, None, Some(sub_pt(p2, p1))));
        (
            p4,
            q4,
            Some(sub_pt(p3, p4)),
            Some((q1, sub_pt(q3, q4), sub_pt(q2, q1))),
        )
    } else {
        (t1, t2, None, None)
    };

    // The middle arc, on the plain corner's circle, in ≤ 90° slices.
    let mut arc = Vec::new();
    if sweep > MIN_ANGLE {
        let slices = (sweep / FRAC_PI_2).ceil().max(1.0) as usize;
        let step = sweep / slices as f64;
        let h = 4.0 / 3.0 * (step / 4.0).tan() * r;
        let a0 = sub_pt(arc_start, centre);
        let sign = if cross(a0, sub_pt(arc_end, centre)) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        for j in 0..=slices {
            let point = if j == slices {
                arc_end
            } else {
                add(centre, rotate(a0, sign * step * j as f64))
            };
            let radial = unit(sub_pt(point, centre)).map_or((0.0, 0.0), |u| u.0);
            let tangent = scale((-radial.1, radial.0), sign);
            arc.push((point, scale(tangent, -h), scale(tangent, h)));
        }
    }

    let out_handle = lead_out.map(|(_, h, _)| h);
    if arc.is_empty() {
        // No arc (full smoothing at this turn): both transitions meet.
        out.push(anchor(arc_start, lead_in, out_handle));
    } else {
        let last = arc.len() - 1;
        for (j, (point, hin, hout)) in arc.iter().enumerate() {
            let handle_in = if j == 0 { lead_in } else { Some(*hin) };
            let handle_out = if j == last { out_handle } else { Some(*hout) };
            out.push(anchor(*point, handle_in, handle_out));
        }
    }
    if let Some((q1, _, into_q1)) = lead_out {
        out.push(anchor(q1, Some(into_q1), None));
    }
}

/// Fold anchors that coincide across a straight, zero-length segment (two
/// corners whose shares meet mid-edge) into one.
pub(super) fn merge_coincident(anchors: &mut Vec<PenPathAnchor>, closed: bool) {
    let mut i = 0;
    while anchors.len() > 1 && i < anchors.len() {
        let j = i + 1;
        if j == anchors.len() && !closed {
            break;
        }
        let j = j % anchors.len();
        let (a, b) = (&anchors[i], &anchors[j]);
        let same = (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS;
        if same && !has_handle(a.handle_out.as_ref()) && !has_handle(b.handle_in.as_ref()) {
            let handle_out = anchors[j].handle_out.clone();
            anchors[i].handle_out = handle_out;
            anchors.remove(j);
            if j < i {
                i -= 1;
            }
            continue;
        }
        i += 1;
    }
}

fn anchor(p: Pt, handle_in: Option<Pt>, handle_out: Option<Pt>) -> PenPathAnchor {
    let handle = |h: Option<Pt>| {
        h.filter(|h| h.0.abs() > EPS || h.1.abs() > EPS)
            .map(|(x, y)| PenPathHandle { x, y })
    };
    PenPathAnchor {
        x: p.0,
        y: p.1,
        handle_in: handle(handle_in),
        handle_out: handle(handle_out),
        point_type: None,
    }
}

fn has_handle(h: Option<&PenPathHandle>) -> bool {
    h.is_some_and(|h| h.x.abs() > EPS || h.y.abs() > EPS)
}

fn add(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}

fn sub_pt(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

fn scale(a: Pt, k: f64) -> Pt {
    (a.0 * k, a.1 * k)
}

fn dot(a: Pt, b: Pt) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn cross(a: Pt, b: Pt) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn rotate(a: Pt, angle: f64) -> Pt {
    let (s, c) = angle.sin_cos();
    (a.0 * c - a.1 * s, a.0 * s + a.1 * c)
}

/// `a` normalised, with its length; `None` for a zero vector.
fn unit(a: Pt) -> Option<(Pt, f64)> {
    let len = a.0.hypot(a.1);
    (len > EPS).then(|| ((a.0 / len, a.1 / len), len))
}
