//! skia paths ↔ the editor's editable [`Subpath`]s, curves kept.
//!
//! A skia path is a verb stream (move / line / quad / conic / cubic /
//! close). Each verb becomes anchors with cubic handles directly: a quad
//! is raised to the cubic it is exactly, a conic (skia's circle and
//! ellipse arcs) to the standard cubic arc approximation, split first
//! when wider than a quarter turn. Nothing is sampled into polylines, so
//! a boolean of two circles stays a handful of curves.

use de_editor_core::vector_edit::{corner, Subpath};
use jian_ops_schema::node::{PenPathAnchor, PenPathHandle};
use skia_safe::{Path, PathBuilder, PathFillType, PathVerb, Point};

/// Distance under which two points are the same anchor (doc px).
const SAME_POINT: f64 = 1e-3;
/// Conics flatter than this weight (wider than ~90°) are split before
/// being raised to cubics; the cubic approximation is good to ~0.03% of
/// the radius up to a quarter turn.
const MIN_CONIC_WEIGHT: f32 = 0.7;

/// The path's contours as editable subpaths (doc space, handles relative
/// to their anchor). Degenerate contours (a lone move) are dropped.
pub fn path_to_subpaths(path: &Path) -> Vec<Subpath> {
    let points = path.points();
    let weights = path.conic_weights();
    let mut out = Vec::new();
    let mut current: Option<Subpath> = None;
    let (mut pi, mut wi) = (0usize, 0usize);
    let pt = |i: usize| -> (f64, f64) {
        let p = points[i];
        (f64::from(p.x), f64::from(p.y))
    };
    for verb in path.verbs() {
        match verb {
            PathVerb::Move => {
                flush(&mut out, current.take());
                current = Some(Subpath {
                    anchors: vec![corner(pt(pi))],
                    closed: false,
                });
                pi += 1;
            }
            PathVerb::Line => {
                if let Some(sub) = current.as_mut() {
                    push_line(sub, pt(pi));
                }
                pi += 1;
            }
            PathVerb::Quad => {
                if let Some(sub) = current.as_mut() {
                    let p0 = last_pos(sub);
                    let (c, p2) = (pt(pi), pt(pi + 1));
                    let c1 = lerp(p0, c, 2.0 / 3.0);
                    let c2 = lerp(p2, c, 2.0 / 3.0);
                    push_cubic(sub, c1, c2, p2);
                }
                pi += 2;
            }
            PathVerb::Conic => {
                let w = weights.get(wi).copied().unwrap_or(1.0);
                if let Some(sub) = current.as_mut() {
                    let p0 = last_pos(sub);
                    push_conic(sub, p0, pt(pi), pt(pi + 1), w, 0);
                }
                pi += 2;
                wi += 1;
            }
            PathVerb::Cubic => {
                if let Some(sub) = current.as_mut() {
                    push_cubic(sub, pt(pi), pt(pi + 1), pt(pi + 2));
                }
                pi += 3;
            }
            PathVerb::Close => {
                if let Some(mut sub) = current.take() {
                    close(&mut sub);
                    flush(&mut out, Some(sub));
                }
            }
        }
    }
    flush(&mut out, current);
    for sub in &mut out {
        for a in &mut sub.anchors {
            round_anchor(a);
        }
    }
    out
}

/// Skia computes in f32: digits past 1/1000 px are noise, and keeping
/// them only bloats the stored `d`.
fn round_anchor(a: &mut PenPathAnchor) {
    let r = |v: f64| (v * 1000.0).round() / 1000.0;
    a.x = r(a.x);
    a.y = r(a.y);
    for h in [a.handle_in.as_mut(), a.handle_out.as_mut()]
        .into_iter()
        .flatten()
    {
        h.x = r(h.x);
        h.y = r(h.y);
    }
}

/// Build a skia path from editable subpaths (doc space). Each subpath is
/// one contour; `even_odd` sets the fill rule.
pub fn subpaths_to_path(subs: &[Subpath], even_odd: bool) -> Path {
    let mut b = PathBuilder::new();
    for sub in subs.iter().filter(|s| !s.anchors.is_empty()) {
        let a0 = &sub.anchors[0];
        b.move_to(p32((a0.x, a0.y)));
        for pair in sub.anchors.windows(2) {
            segment_to(&mut b, &pair[0], &pair[1]);
        }
        if sub.closed {
            if let (Some(last), Some(first)) = (sub.anchors.last(), sub.anchors.first()) {
                if sub.anchors.len() > 1 && (has_out(last) || has_in(first)) {
                    segment_to(&mut b, last, first);
                }
            }
            b.close();
        }
    }
    let mut path = b.detach();
    path.set_fill_type(if even_odd {
        PathFillType::EvenOdd
    } else {
        PathFillType::Winding
    });
    path
}

/// Count the curve segments of `subs` (segments with at least one
/// handle), for tests and diagnostics.
pub fn cubic_segment_count(subs: &[Subpath]) -> usize {
    subs.iter()
        .map(|s| {
            let n = s.anchors.len();
            let segs = if s.closed { n } else { n.saturating_sub(1) };
            (0..segs)
                .filter(|&i| {
                    let a = &s.anchors[i];
                    let b = &s.anchors[(i + 1) % n];
                    has_out(a) || has_in(b)
                })
                .count()
        })
        .sum()
}

fn segment_to(b: &mut PathBuilder, a: &PenPathAnchor, next: &PenPathAnchor) {
    if has_out(a) || has_in(next) {
        let c1 = add(a, a.handle_out.as_ref());
        let c2 = add(next, next.handle_in.as_ref());
        b.cubic_to(p32(c1), p32(c2), p32((next.x, next.y)));
    } else {
        b.line_to(p32((next.x, next.y)));
    }
}

fn add(a: &PenPathAnchor, h: Option<&PenPathHandle>) -> (f64, f64) {
    match h {
        Some(h) => (a.x + h.x, a.y + h.y),
        None => (a.x, a.y),
    }
}

fn has_out(a: &PenPathAnchor) -> bool {
    a.handle_out
        .as_ref()
        .is_some_and(|h| h.x != 0.0 || h.y != 0.0)
}

fn has_in(a: &PenPathAnchor) -> bool {
    a.handle_in
        .as_ref()
        .is_some_and(|h| h.x != 0.0 || h.y != 0.0)
}

fn p32(p: (f64, f64)) -> Point {
    Point::new(p.0 as f32, p.1 as f32)
}

fn lerp(a: (f64, f64), b: (f64, f64), t: f64) -> (f64, f64) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn last_pos(sub: &Subpath) -> (f64, f64) {
    sub.anchors.last().map_or((0.0, 0.0), |a| (a.x, a.y))
}

fn same(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < SAME_POINT && (a.1 - b.1).abs() < SAME_POINT
}

fn handle(from: (f64, f64), to: (f64, f64)) -> Option<PenPathHandle> {
    let (x, y) = (to.0 - from.0, to.1 - from.1);
    (x.abs() > 1e-9 || y.abs() > 1e-9).then_some(PenPathHandle { x, y })
}

fn push_line(sub: &mut Subpath, p: (f64, f64)) {
    if !same(last_pos(sub), p) {
        sub.anchors.push(corner(p));
    }
}

fn push_cubic(sub: &mut Subpath, c1: (f64, f64), c2: (f64, f64), p3: (f64, f64)) {
    let p0 = last_pos(sub);
    if same(p0, p3) && same(p0, c1) && same(p0, c2) {
        return;
    }
    if let Some(last) = sub.anchors.last_mut() {
        last.handle_out = handle(p0, c1);
    }
    let mut a = corner(p3);
    a.handle_in = handle(p3, c2);
    sub.anchors.push(a);
}

/// Raise a conic to cubics: `c1 = p0 + k (c − p0)`, `c2 = p2 + k (c − p2)`
/// with `k = 4w / (3 (1 + w))` (exact tangents, the standard circular-arc
/// approximation for `w = cos(θ/2)`). Wide conics are halved first.
fn push_conic(sub: &mut Subpath, p0: (f64, f64), c: (f64, f64), p2: (f64, f64), w: f32, depth: u8) {
    if w < MIN_CONIC_WEIGHT && depth < 4 && w > 0.0 {
        let w = f64::from(w);
        let q1 = ((p0.0 + w * c.0) / (1.0 + w), (p0.1 + w * c.1) / (1.0 + w));
        let q3 = ((w * c.0 + p2.0) / (1.0 + w), (w * c.1 + p2.1) / (1.0 + w));
        let mid = (
            (p0.0 + 2.0 * w * c.0 + p2.0) / (2.0 * (1.0 + w)),
            (p0.1 + 2.0 * w * c.1 + p2.1) / (2.0 * (1.0 + w)),
        );
        let half = ((1.0 + w) / 2.0).sqrt() as f32;
        push_conic(sub, p0, q1, mid, half, depth + 1);
        push_conic(sub, mid, q3, p2, half, depth + 1);
        return;
    }
    let w = f64::from(w);
    let k = 4.0 * w / (3.0 * (1.0 + w));
    push_cubic(sub, lerp(p0, c, k), lerp(p2, c, k), p2);
}

/// Mark closed; a final anchor sitting on the first one is folded into it
/// (its incoming handle carried over), so the seam is one anchor.
fn close(sub: &mut Subpath) {
    sub.closed = true;
    if sub.anchors.len() > 1 {
        let first = (sub.anchors[0].x, sub.anchors[0].y);
        let last = sub.anchors.last().map(|a| (a.x, a.y)).unwrap_or(first);
        if same(first, last) {
            if let Some(end) = sub.anchors.pop() {
                sub.anchors[0].handle_in = end.handle_in;
            }
        }
    }
}

fn flush(out: &mut Vec<Subpath>, sub: Option<Subpath>) {
    if let Some(sub) = sub.filter(|s| s.anchors.len() >= 2) {
        out.push(sub);
    }
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;
