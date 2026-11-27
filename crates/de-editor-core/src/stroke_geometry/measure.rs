//! End frames and trimming of a run of cubics (`[p0, p1, p2, p3]`, a
//! straight segment having `p1 = p0`, `p2 = p3`).

use crate::vector_edit::{cubic_point, split_cubic, Pt};

const EPS: f64 = 1e-9;
/// Samples per segment for arc-length estimates.
const LENGTH_SAMPLES: usize = 32;

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

fn unit(v: Pt) -> Option<Pt> {
    let len = v.0.hypot(v.1);
    (len > EPS).then(|| (v.0 / len, v.1 / len))
}

/// The start point of the run and the unit direction pointing out of the
/// path there (against the direction of travel).
pub(super) fn start_frame(segs: &[[Pt; 4]]) -> Option<(Pt, Pt)> {
    segs.iter().find_map(|c| {
        let dir = [c[1], c[2], c[3]]
            .into_iter()
            .find_map(|p| unit(sub(c[0], p)))?;
        Some((c[0], dir))
    })
}

/// The end point of the run and the unit direction of travel there.
pub(super) fn end_frame(segs: &[[Pt; 4]]) -> Option<(Pt, Pt)> {
    segs.iter().rev().find_map(|c| {
        let dir = [c[2], c[1], c[0]]
            .into_iter()
            .find_map(|p| unit(sub(c[3], p)))?;
        Some((c[3], dir))
    })
}

/// Arc length of one cubic, by sampling.
fn length(c: &[Pt; 4]) -> f64 {
    let mut prev = c[0];
    let mut total = 0.0;
    for i in 1..=LENGTH_SAMPLES {
        let p = cubic_point(*c, i as f64 / LENGTH_SAMPLES as f64);
        total += (p.0 - prev.0).hypot(p.1 - prev.1);
        prev = p;
    }
    total
}

/// Parameter `t` at which the first `dist` of `c`'s length ends.
fn t_at_length(c: &[Pt; 4], dist: f64) -> f64 {
    if c[1] == c[0] && c[2] == c[3] {
        // A straight segment splits linearly (`split_keep_straight`).
        let len = (c[3].0 - c[0].0).hypot(c[3].1 - c[0].1);
        return if len > EPS {
            (dist / len).clamp(0.0, 1.0)
        } else {
            1.0
        };
    }
    let mut prev = c[0];
    let mut total = 0.0;
    for i in 1..=LENGTH_SAMPLES {
        let t = i as f64 / LENGTH_SAMPLES as f64;
        let p = cubic_point(*c, t);
        let step = (p.0 - prev.0).hypot(p.1 - prev.1);
        if total + step >= dist && step > EPS {
            let back = (total + step - dist) / step;
            return t - back / LENGTH_SAMPLES as f64;
        }
        total += step;
        prev = p;
    }
    1.0
}

/// Shorten the run by `dist` at its start. Whole segments shorter than
/// what is left go; a run is never emptied (its last segment keeps at
/// least a sliver, so the stroke stays drawable).
pub(super) fn trim_start(segs: &mut Vec<[Pt; 4]>, mut dist: f64) {
    while dist > EPS && !segs.is_empty() {
        let len = length(&segs[0]);
        if len <= dist && segs.len() > 1 {
            dist -= len;
            segs.remove(0);
            continue;
        }
        let t = t_at_length(&segs[0], dist.min(len * 0.999));
        segs[0] = split_keep_straight(segs[0], t).1;
        return;
    }
}

/// Shorten the run by `dist` at its end (see [`trim_start`]).
pub(super) fn trim_end(segs: &mut Vec<[Pt; 4]>, mut dist: f64) {
    while dist > EPS && !segs.is_empty() {
        let last = segs.len() - 1;
        let len = length(&segs[last]);
        if len <= dist && segs.len() > 1 {
            dist -= len;
            segs.pop();
            continue;
        }
        let keep = (len - dist).max(len * 0.001);
        let t = t_at_length(&segs[last], keep);
        segs[last] = split_keep_straight(segs[last], t).0;
        return;
    }
}

/// [`split_cubic`], keeping a straight segment's `p1 = p0, p2 = p3` form
/// so it is written back as a line.
fn split_keep_straight(c: [Pt; 4], t: f64) -> ([Pt; 4], [Pt; 4]) {
    if c[1] == c[0] && c[2] == c[3] {
        let m = (
            c[0].0 + (c[3].0 - c[0].0) * t,
            c[0].1 + (c[3].1 - c[0].1) * t,
        );
        ([c[0], c[0], m, m], [m, m, c[3], c[3]])
    } else {
        split_cubic(c, t)
    }
}
