//! End shapes of an open stroke. Each head is built at its end point
//! `tip` with `dir` the unit direction pointing *out of* the path, and
//! reports how much of the centre line it covers (the trim), so the line
//! never pokes out of a pointed head.

use super::{push_cmd, StrokeGeometry};
use crate::vector_edit::Pt;
use jian_ops_schema::style::StrokeEndCap;

/// Length of a head along the line for a stroke `width` wide: arrowheads
/// grow with the stroke but stay readable on hairlines.
pub fn head_length(width: f64) -> f64 {
    4.0 + 3.0 * width
}

/// Half-angle between a line arrow's arms and the line.
const LINE_ARROW_ANGLE_DEG: f64 = 40.0;
/// Circle radius as a fraction of the head length.
const CIRCLE_FACTOR: f64 = 0.4;

/// Add the shape `cap` draws at `tip` to `out`; returns how far the
/// centre line must be shortened at that end.
pub(super) fn push_head(
    out: &mut StrokeGeometry,
    tip: Pt,
    dir: Pt,
    width: f64,
    cap: StrokeEndCap,
) -> f64 {
    let s = head_length(width);
    let half_w = width / 2.0;
    let n = (-dir.1, dir.0);
    let at = |along: f64, across: f64| {
        (
            tip.0 + dir.0 * along + n.0 * across,
            tip.1 + dir.1 * along + n.1 * across,
        )
    };
    match cap {
        StrokeEndCap::None => 0.0,
        StrokeEndCap::Square => {
            polygon(
                &mut out.fill_d,
                &[
                    at(0.0, half_w),
                    at(half_w, half_w),
                    at(half_w, -half_w),
                    at(0.0, -half_w),
                ],
            );
            0.0
        }
        StrokeEndCap::Round => {
            // Half a circle centred on the end point, bulging outward: two
            // quarter arcs turning from the normal through `dir` (each a
            // -90° turn, so sweep 0 in y-down coordinates).
            push_cmd(&mut out.fill_d, 'M', &[at(0.0, half_w)]);
            push_arc(&mut out.fill_d, half_w, at(half_w, 0.0), false);
            push_arc(&mut out.fill_d, half_w, at(0.0, -half_w), false);
            out.fill_d.push_str(" Z");
            0.0
        }
        StrokeEndCap::LineArrow => {
            let (sin, cos) = LINE_ARROW_ANGLE_DEG.to_radians().sin_cos();
            push_cmd(&mut out.arms_d, 'M', &[at(-s * cos, s * sin)]);
            push_cmd(&mut out.arms_d, 'L', &[tip, at(-s * cos, -s * sin)]);
            0.0
        }
        StrokeEndCap::TriangleArrow => {
            polygon(&mut out.fill_d, &[tip, at(-s, s / 2.0), at(-s, -s / 2.0)]);
            // Width at distance d from the tip is d: end the line just
            // inside the base, never nearer the tip than its own width.
            (s - 0.5).max(width).min(s)
        }
        StrokeEndCap::ReversedTriangle => {
            polygon(
                &mut out.fill_d,
                &[at(0.0, s / 2.0), at(-s, 0.0), at(0.0, -s / 2.0)],
            );
            // Width at distance d from the base is s - d.
            (s - width).max(0.0)
        }
        StrokeEndCap::Circle => {
            let r = s * CIRCLE_FACTOR;
            push_cmd(&mut out.fill_d, 'M', &[at(r, 0.0)]);
            push_arc(&mut out.fill_d, r, at(-r, 0.0), false);
            push_arc(&mut out.fill_d, r, at(r, 0.0), false);
            out.fill_d.push_str(" Z");
            if r > half_w {
                (r * r - half_w * half_w).sqrt()
            } else {
                0.0
            }
        }
        StrokeEndCap::Diamond => {
            let a = s / 2.0;
            polygon(
                &mut out.fill_d,
                &[at(a, 0.0), at(0.0, a), at(-a, 0.0), at(0.0, -a)],
            );
            (a * (1.0 - width / (2.0 * a))).max(0.0)
        }
    }
}

fn polygon(d: &mut String, pts: &[Pt]) {
    push_cmd(d, 'M', &pts[..1]);
    push_cmd(d, 'L', &pts[1..]);
    d.push_str(" Z");
}

/// Append a minor circular arc of radius `r` to `to`.
fn push_arc(d: &mut String, r: f64, to: Pt, sweep: bool) {
    use std::fmt::Write;
    let r = super::num(r);
    let _ = write!(
        d,
        " A {r} {r} 0 0 {} {} {}",
        u8::from(sweep),
        super::num(to.0),
        super::num(to.1)
    );
}
