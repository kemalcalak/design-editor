//! Rounded and smoothed corners (#389): the one outline every consumer of
//! `cornerRadius` + `cornerSmoothing` draws — the canvas, its clips and
//! hit-test, SVG / HTML / PPTX export and the boolean / outline path ops.
//!
//! Smoothing follows Figma's squircle (the `figma-squircle` construction):
//! a corner of radius `r` whose plain arc would start `t` from the vertex
//! starts `p = (1 + s)·t` from it instead; a circular arc on the plain
//! corner's circle covers the middle `(1 − s)` of the turn, and a cubic on
//! each side eases from the straight edge into it (zero curvature at the
//! edge, the arc's tangent where it meets the arc). At 90° (`t = r`) this is
//! Figma's corner exactly; other angles use the same construction with the
//! turn `τ` in place of 90° (`t = r·tan(τ/2)`, transitions turning `τ·s/2`).
//!
//! When neighbouring corners ask for more edge than there is, each corner's
//! share is fixed largest-first ([`budget`], Figma's distribution); a corner
//! over its share loses smoothing first, then radius.
//!
//! Smoothing `0` produces plain circular corners, so one code path serves
//! polygons and paths with a radius but no smoothing. Rectangles with
//! smoothing `0` never come here: the painters' rounded-rect primitives
//! already draw them.

mod budget;
mod corner;

use crate::stroke_geometry::push_cmd;
use crate::vector_edit::{segment_controls, segment_count, Pt, Subpath};

/// `s` as a smoothing factor: finite values clamp to `0..=1`, anything
/// else is `0` (plain corners).
pub fn clamp_smoothing(s: f64) -> f64 {
    if s.is_finite() {
        s.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Whether `s` asks for any smoothing at all.
pub fn has_smoothing(s: f64) -> bool {
    clamp_smoothing(s) > 1e-4
}

/// Round the corners of one subpath. `radii[i]` is anchor `i`'s radius
/// (missing entries are `0`). An anchor rounds when it is a sharp vertex
/// between two straight segments with a real turn: every anchor of a
/// closed subpath, the inner ones of an open subpath. Curved joins and the
/// ends of an open subpath stay as they are.
pub fn round_corners(sub: &Subpath, radii: &[f64], smoothing: f64) -> Subpath {
    let n = sub.anchors.len();
    if n < 3 {
        return sub.clone();
    }
    let vertices: Vec<Option<corner::Vertex>> = (0..n)
        .map(|i| corner::Vertex::at(sub, i, radii.get(i).copied().unwrap_or(0.0)))
        .collect();
    if vertices.iter().all(Option::is_none) {
        return sub.clone();
    }
    let plans = budget::distribute(&vertices, sub.closed, clamp_smoothing(smoothing));
    let mut anchors = Vec::with_capacity(n * 5);
    for (i, anchor) in sub.anchors.iter().enumerate() {
        match (&vertices[i], plans[i]) {
            (Some(vertex), Some(plan)) => corner::push_corner(&mut anchors, vertex, plan),
            _ => anchors.push(anchor.clone()),
        }
    }
    corner::merge_coincident(&mut anchors, sub.closed);
    Subpath {
        anchors,
        closed: sub.closed,
    }
}

/// The outline of a `w × h` rectangle at the origin with corner radii in
/// top-left, top-right, bottom-right, bottom-left order. `None` for an
/// empty or non-finite box.
pub fn rect_outline(w: f64, h: f64, radii: [f64; 4], smoothing: f64) -> Option<Subpath> {
    if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
        return None;
    }
    let mut rect = Subpath::polyline(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]);
    rect.closed = true;
    Some(round_corners(&rect, &radii, smoothing))
}

/// The closed outline through `points` with every vertex rounded by
/// `radius`. `None` for fewer than three points.
pub fn polygon_outline(points: &[Pt], radius: f64, smoothing: f64) -> Option<Subpath> {
    if points.len() < 3 {
        return None;
    }
    let mut ring = Subpath::polyline(points);
    ring.closed = true;
    Some(round_corners(&ring, &vec![radius; points.len()], smoothing))
}

/// `subs` as SVG path data, coordinates rounded to 1/1000 px: `L` for
/// straight segments, `C` for curves, `Z` closing a closed run (its last
/// segment written only when it is curved).
pub fn outline_path_data(subs: &[Subpath]) -> String {
    let mut d = String::new();
    for sub in subs.iter().filter(|s| !s.anchors.is_empty()) {
        let first = &sub.anchors[0];
        push_cmd(&mut d, 'M', &[(first.x, first.y)]);
        let count = segment_count(sub);
        for seg in 0..count {
            let Some(c) = segment_controls(sub, seg) else {
                continue;
            };
            let straight = c[1] == c[0] && c[2] == c[3];
            let closing = sub.closed && seg + 1 == count;
            if straight && closing {
                break;
            }
            if straight {
                push_cmd(&mut d, 'L', &[c[3]]);
            } else {
                push_cmd(&mut d, 'C', &[c[1], c[2], c[3]]);
            }
        }
        if sub.closed {
            d.push_str(" Z");
        }
    }
    d
}

/// Path data `d` moved by `(dx, dy)` and rewritten like
/// [`outline_path_data`]; empty when `d` does not parse. Exports place a
/// node-local outline in document coordinates with it.
pub fn translate_path_data(d: &str, dx: f64, dy: f64) -> String {
    let Some(mut subs) = crate::vector_edit::parse_path_data(d) else {
        return String::new();
    };
    for anchor in subs.iter_mut().flat_map(|s| s.anchors.iter_mut()) {
        anchor.x += dx;
        anchor.y += dy;
    }
    outline_path_data(&subs)
}

/// `sub` flattened to a polyline: each curved segment sampled in
/// `steps` pieces, straight ones kept as they are. A closed subpath does
/// not repeat its first point.
pub fn flatten_outline(sub: &Subpath, steps: usize) -> Vec<Pt> {
    let steps = steps.max(1);
    let mut out = Vec::new();
    if let Some(first) = sub.anchors.first() {
        out.push((first.x, first.y));
    }
    let count = segment_count(sub);
    for seg in 0..count {
        let Some(c) = segment_controls(sub, seg) else {
            continue;
        };
        let straight = c[1] == c[0] && c[2] == c[3];
        let pieces = if straight { 1 } else { steps };
        for k in 1..=pieces {
            out.push(crate::vector_edit::cubic_point(c, k as f64 / pieces as f64));
        }
    }
    if sub.closed && out.len() > 1 {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests;
