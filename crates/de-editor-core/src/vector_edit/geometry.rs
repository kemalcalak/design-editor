//! Pure subpath geometry: segment access, de Casteljau split, nearest
//! point, delete-and-heal, reverse and join.

use super::{corner, PointRef, Pt, Subpath};
use jian_ops_schema::node::{PenPathAnchor, PenPathHandle};
use std::collections::BTreeSet;

const EPS: f64 = 1e-9;

fn add(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}
fn minus(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}
fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}
fn pos(a: &PenPathAnchor) -> Pt {
    (a.x, a.y)
}
fn handle_of(h: Option<&PenPathHandle>) -> Pt {
    h.map_or((0.0, 0.0), |h| (h.x, h.y))
}
fn to_handle(v: Pt) -> Option<PenPathHandle> {
    (v.0.abs() > EPS || v.1.abs() > EPS).then_some(PenPathHandle { x: v.0, y: v.1 })
}

/// Number of segments of a subpath.
pub fn segment_count(sub: &Subpath) -> usize {
    let n = sub.anchors.len();
    match (n, sub.closed) {
        (0 | 1, _) => 0,
        (n, true) => n,
        (n, false) => n - 1,
    }
}

/// `[p0, p1, p2, p3]` of segment `seg` (a straight segment has
/// `p1 = p0`, `p2 = p3`).
pub fn segment_controls(sub: &Subpath, seg: usize) -> Option<[Pt; 4]> {
    if seg >= segment_count(sub) {
        return None;
    }
    let a = &sub.anchors[seg];
    let b = &sub.anchors[(seg + 1) % sub.anchors.len()];
    Some([
        pos(a),
        add(pos(a), handle_of(a.handle_out.as_ref())),
        add(pos(b), handle_of(b.handle_in.as_ref())),
        pos(b),
    ])
}

/// Point of the cubic at `t`.
pub fn cubic_point(c: [Pt; 4], t: f64) -> Pt {
    let mt = 1.0 - t;
    let (a, b, cc, d) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
    (
        a * c[0].0 + b * c[1].0 + cc * c[2].0 + d * c[3].0,
        a * c[0].1 + b * c[1].1 + cc * c[2].1 + d * c[3].1,
    )
}

/// de Casteljau split at `t`: the two halves trace the original curve
/// exactly.
pub fn split_cubic(c: [Pt; 4], t: f64) -> ([Pt; 4], [Pt; 4]) {
    let p01 = lerp(c[0], c[1], t);
    let p12 = lerp(c[1], c[2], t);
    let p23 = lerp(c[2], c[3], t);
    let p012 = lerp(p01, p12, t);
    let p123 = lerp(p12, p23, t);
    let mid = lerp(p012, p123, t);
    ([c[0], p01, p012, mid], [mid, p123, p23, c[3]])
}

/// `(t, distance)` of the point of `c` nearest to `p`: a coarse sample
/// pass, then a bisection-style refinement around the best sample.
pub fn nearest_on_segment(c: [Pt; 4], p: Pt) -> (f64, f64) {
    let dist = |t: f64| {
        let q = cubic_point(c, t);
        (q.0 - p.0).hypot(q.1 - p.1)
    };
    const SAMPLES: usize = 48;
    let mut best_t = 0.0;
    let mut best = f64::INFINITY;
    for i in 0..=SAMPLES {
        let t = i as f64 / SAMPLES as f64;
        let d = dist(t);
        if d < best {
            best = d;
            best_t = t;
        }
    }
    let mut step = 1.0 / SAMPLES as f64;
    for _ in 0..30 {
        step /= 2.0;
        for t in [best_t - step, best_t + step] {
            let t = t.clamp(0.0, 1.0);
            let d = dist(t);
            if d < best {
                best = d;
                best_t = t;
            }
        }
    }
    (best_t, best)
}

/// Insert an anchor on `seg` at `t`, keeping the curve shape exactly.
/// Returns the new anchor's index.
pub fn split_segment(sub: &mut Subpath, seg: usize, t: f64) -> Option<usize> {
    let c = segment_controls(sub, seg)?;
    let n = sub.anchors.len();
    let next = (seg + 1) % n;
    let straight = sub.anchors[seg].handle_out.is_none() && sub.anchors[next].handle_in.is_none();
    let (l, r) = split_cubic(c, t);
    let mut anchor = corner(l[3]);
    if !straight {
        sub.anchors[seg].handle_out = to_handle(minus(l[1], l[0]));
        sub.anchors[next].handle_in = to_handle(minus(r[2], r[3]));
        anchor.handle_in = to_handle(minus(l[2], l[3]));
        anchor.handle_out = to_handle(minus(r[1], r[0]));
    }
    let at = seg + 1;
    sub.anchors.insert(at, anchor);
    Some(at)
}

/// Reverse a subpath's direction (anchor order and in / out handles).
pub fn reverse_subpath(sub: &mut Subpath) {
    sub.anchors.reverse();
    for a in &mut sub.anchors {
        std::mem::swap(&mut a.handle_in, &mut a.handle_out);
    }
    if sub.closed && !sub.anchors.is_empty() {
        // Keep the start anchor where it was: rotate it back to front.
        sub.anchors.rotate_right(1);
    }
}

/// Unit tangent leaving `c[0]` (`forward`) or arriving at `c[3]`, from
/// the first non-degenerate control vector.
fn tangent(c: [Pt; 4], forward: bool) -> Pt {
    let cands = if forward {
        [minus(c[1], c[0]), minus(c[2], c[0]), minus(c[3], c[0])]
    } else {
        [minus(c[2], c[3]), minus(c[1], c[3]), minus(c[0], c[3])]
    };
    for v in cands {
        let len = v.0.hypot(v.1);
        if len > EPS {
            return (v.0 / len, v.1 / len);
        }
    }
    (0.0, 0.0)
}

/// Fit ONE cubic from `p0` to `p3` through the chain `segs` (consecutive
/// cubics), keeping the end tangents: least-squares handle lengths over
/// chord-length-parameterised samples (one Schneider step). A chain of
/// straight segments heals into a straight segment.
fn fit_chain(segs: &[[Pt; 4]]) -> (Option<PenPathHandle>, Option<PenPathHandle>) {
    let all_straight = segs.iter().all(|c| c[1] == c[0] && c[2] == c[3]);
    if all_straight {
        return (None, None);
    }
    let p0 = segs[0][0];
    let p3 = segs[segs.len() - 1][3];
    let t0 = tangent(segs[0], true);
    let t1 = tangent(segs[segs.len() - 1], false);
    let mut samples: Vec<Pt> = vec![p0];
    for c in segs {
        for i in 1..=16 {
            samples.push(cubic_point(*c, i as f64 / 16.0));
        }
    }
    let mut lens = vec![0.0];
    for w in samples.windows(2) {
        let d = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        lens.push(lens.last().copied().unwrap_or(0.0) + d);
    }
    let total = lens.last().copied().unwrap_or(0.0).max(EPS);
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (q, l) in samples.iter().zip(&lens) {
        let u = l / total;
        let mu = 1.0 - u;
        let b0 = mu * mu * mu;
        let b1 = 3.0 * mu * mu * u;
        let b2 = 3.0 * mu * u * u;
        let b3 = u * u * u;
        let a1 = (t0.0 * b1, t0.1 * b1);
        let a2 = (t1.0 * b2, t1.1 * b2);
        c00 += a1.0 * a1.0 + a1.1 * a1.1;
        c01 += a1.0 * a2.0 + a1.1 * a2.1;
        c11 += a2.0 * a2.0 + a2.1 * a2.1;
        let r = (
            q.0 - (p0.0 * (b0 + b1) + p3.0 * (b2 + b3)),
            q.1 - (p0.1 * (b0 + b1) + p3.1 * (b2 + b3)),
        );
        x0 += a1.0 * r.0 + a1.1 * r.1;
        x1 += a2.0 * r.0 + a2.1 * r.1;
    }
    let det = c00 * c11 - c01 * c01;
    let chord = (p3.0 - p0.0).hypot(p3.1 - p0.1);
    let (mut l0, mut l1) = if det.abs() > EPS {
        ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
    } else {
        (chord / 3.0, chord / 3.0)
    };
    // A negative or vanishing length flips the tangent; fall back to
    // the classic third-of-the-chord heuristic.
    if l0 < EPS || l1 < EPS {
        l0 = chord / 3.0;
        l1 = chord / 3.0;
    }
    (
        to_handle((t0.0 * l0, t0.1 * l0)),
        to_handle((t1.0 * l1, t1.1 * l1)),
    )
}

/// Delete `points` (indices into `subs`) and heal: every run of removed
/// middle anchors is replaced by one segment fitted to the curve it
/// covered, so the shape changes as little as possible. Open ends just
/// drop. A subpath left with fewer than two anchors disappears. Returns
/// the new subpath list.
pub fn delete_points(subs: &[Subpath], points: &BTreeSet<PointRef>) -> Vec<Subpath> {
    let mut out = Vec::with_capacity(subs.len());
    for (si, sub) in subs.iter().enumerate() {
        let n = sub.anchors.len();
        let removed: Vec<bool> = (0..n)
            .map(|i| points.contains(&PointRef::new(si, i)))
            .collect();
        if !removed.iter().any(|r| *r) {
            out.push(sub.clone());
            continue;
        }
        let kept: Vec<usize> = (0..n).filter(|i| !removed[*i]).collect();
        if kept.len() < 2 {
            continue;
        }
        let mut anchors: Vec<PenPathAnchor> =
            kept.iter().map(|i| sub.anchors[*i].clone()).collect();
        let pairs = if sub.closed {
            kept.len()
        } else {
            kept.len() - 1
        };
        for k in 0..pairs {
            let a = kept[k];
            let b = kept[(k + 1) % kept.len()];
            let gap = if b > a { b - a } else { b + n - a };
            if gap <= 1 {
                continue;
            }
            let chain: Vec<[Pt; 4]> = (0..gap)
                .filter_map(|s| segment_controls(sub, (a + s) % n))
                .collect();
            let (out_h, in_h) = fit_chain(&chain);
            let kb = (k + 1) % kept.len();
            anchors[k].handle_out = out_h;
            anchors[kb].handle_in = in_h;
            anchors[k].point_type = None;
            anchors[kb].point_type = None;
        }
        if !sub.closed {
            anchors[0].handle_in = None;
            if let Some(last) = anchors.last_mut() {
                last.handle_out = None;
            }
        }
        out.push(Subpath {
            anchors,
            closed: sub.closed,
        });
    }
    out
}

/// Join two OPEN endpoints `a` and `b` with a straight segment. The same
/// subpath's two ends close it; ends of two subpaths merge them into one
/// (stored at the lower index). `merge` joins coincident ends into ONE
/// anchor instead of adding a segment (an end dropped onto an end).
/// Returns the joined subpath's index, or `None` when `a` / `b` are not
/// two distinct open endpoints.
pub fn join_endpoints(
    subs: &mut Vec<Subpath>,
    a: PointRef,
    b: PointRef,
    merge: bool,
) -> Option<usize> {
    if a == b {
        return None;
    }
    let ends = |p: PointRef| subs.get(p.sub).is_some_and(|s| s.is_endpoint(p.idx));
    if !ends(a) || !ends(b) {
        return None;
    }
    if a.sub == b.sub {
        let sub = &mut subs[a.sub];
        if sub.anchors.len() < 2 {
            return None;
        }
        sub.closed = true;
        let last = sub.anchors.len() - 1;
        sub.anchors[0].handle_in = None;
        sub.anchors[last].handle_out = None;
        if merge {
            let incoming = sub.anchors[last].handle_in.clone();
            sub.anchors.pop();
            sub.anchors[0].handle_in = incoming;
        }
        return Some(a.sub);
    }
    let (lo, hi) = if a.sub < b.sub { (a, b) } else { (b, a) };
    let mut second = subs.remove(hi.sub);
    let first = &mut subs[lo.sub];
    // `first` must end at its joined point, `second` start at its own.
    if lo.idx == 0 && first.anchors.len() > 1 {
        reverse_subpath(first);
    }
    if hi.idx != 0 && second.anchors.len() > 1 {
        reverse_subpath(&mut second);
    }
    if let Some(last) = first.anchors.last_mut() {
        last.handle_out = None;
    }
    second.anchors[0].handle_in = None;
    if merge {
        let out = second.anchors[0].handle_out.clone();
        if let Some(last) = first.anchors.last_mut() {
            last.handle_out = out;
        }
        second.anchors.remove(0);
    }
    first.anchors.extend(second.anchors);
    Some(lo.sub)
}
