//! SVG elliptical arc (`A` / `a`) → cubic Béziers.
//!
//! Endpoint parameterisation → centre parameterisation per the SVG 1.1
//! implementation notes (F.6.5 / F.6.6, including the radius scale-up for
//! radii too small to span the chord), then one cubic per arc slice with
//! the classic `k = 4/3 · tan(φ/4)` handle length. Slices are at most 90°
//! and are subdivided further until the analytic radial error bound,
//! scaled by the larger radius, stays under [`MAX_ARC_ERROR`] — so the
//! approximation error stays below a quarter pixel for any arc size.

use super::Pt;
use std::f64::consts::{FRAC_PI_2, PI};

/// Upper bound on the distance between the true arc and its cubics, in
/// path units. The issue's budget is 0.25 px; half of it leaves room for
/// the extra rounding the write-back adds.
pub(crate) const MAX_ARC_ERROR: f64 = 0.125;

/// Upper bound of the radial error of one cubic approximating a unit
/// circle slice of `phi` radians with `k = 4/3 · tan(phi/4)`.
fn unit_slice_error(phi: f64) -> f64 {
    let q = phi.abs() / 4.0;
    let s = q.sin();
    4.0 / 27.0 * s.powi(6) / q.cos().powi(2)
}

/// Convert one arc command starting at `p0` into cubic segments, each
/// returned as `[c1, c2, end]`. A degenerate arc (zero radius) becomes a
/// straight segment (`c1 = p0`, `c2 = end`); a zero-length one is empty.
#[allow(clippy::too_many_arguments)]
pub fn arc_to_cubics(
    p0: Pt,
    rx: f64,
    ry: f64,
    x_axis_rotation_deg: f64,
    large_arc: bool,
    sweep: bool,
    p1: Pt,
) -> Vec<[Pt; 3]> {
    if (p0.0 - p1.0).abs() < 1e-12 && (p0.1 - p1.1).abs() < 1e-12 {
        return Vec::new();
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-12 || ry < 1e-12 || !rx.is_finite() || !ry.is_finite() {
        return vec![[p0, p1, p1]];
    }
    let phi = x_axis_rotation_deg.to_radians();
    let (sin_p, cos_p) = phi.sin_cos();
    let dx2 = (p0.0 - p1.0) / 2.0;
    let dy2 = (p0.1 - p1.1) / 2.0;
    let x1p = cos_p * dx2 + sin_p * dy2;
    let y1p = -sin_p * dx2 + cos_p * dy2;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let (rx2, ry2) = (rx * rx, ry * ry);
    let num = rx2 * ry2 - rx2 * y1p * y1p - ry2 * x1p * x1p;
    let den = rx2 * y1p * y1p + ry2 * x1p * x1p;
    let mut coef = if den > 0.0 {
        (num / den).max(0.0).sqrt()
    } else {
        0.0
    };
    if large_arc == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;
    let cx = cos_p * cxp - sin_p * cyp + (p0.0 + p1.0) / 2.0;
    let cy = sin_p * cxp + cos_p * cyp + (p0.1 + p1.1) / 2.0;
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let dot = ux * vx + uy * vy;
        let len = (ux.hypot(uy) * vx.hypot(vy)).max(1e-300);
        let a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            -a
        } else {
            a
        }
    };
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = angle(1.0, 0.0, ux, uy);
    let mut dtheta = angle(ux, uy, vx, vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= 2.0 * PI;
    } else if sweep && dtheta < 0.0 {
        dtheta += 2.0 * PI;
    }
    let mut n = ((dtheta.abs() / FRAC_PI_2).ceil() as usize).max(1);
    let radius = rx.max(ry);
    while radius * unit_slice_error(dtheta / n as f64) > MAX_ARC_ERROR && n < 4096 {
        n += 1;
    }
    let map = |x: f64, y: f64| -> Pt {
        (
            cx + cos_p * rx * x - sin_p * ry * y,
            cy + sin_p * rx * x + cos_p * ry * y,
        )
    };
    let step = dtheta / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let a0 = theta1 + step * i as f64;
        let a1 = a0 + step;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let ctrl1 = map(c0 - k * s0, s0 + k * c0);
        let ctrl2 = map(c1 + k * s1, s1 - k * c1);
        let end = if i + 1 == n { p1 } else { map(c1, s1) };
        out.push([ctrl1, ctrl2, end]);
    }
    out
}
