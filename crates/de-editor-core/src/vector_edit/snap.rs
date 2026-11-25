//! Snapping for pen placement, point drags and handle drags.
//!
//! All functions are pure and coordinate-frame agnostic; the host calls
//! them in scene (document) coordinates so the pixel grid and other
//! nodes line up. Order used by the host: Shift constraint first, then
//! point / axis snapping (skipped while Shift constrains), then the
//! pixel grid when nothing else snapped. Holding Cmd / Ctrl suspends
//! all of it except the Shift constraint.

use super::Pt;
use std::f64::consts::FRAC_PI_4;

/// A guide line to paint for a snap, scene coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapGuide {
    pub from: Pt,
    pub to: Pt,
}

/// Result of [`snap_point`].
#[derive(Debug, Clone, PartialEq)]
pub struct SnapResult {
    pub point: Pt,
    /// True when the point landed exactly on a candidate point.
    pub on_point: bool,
    /// True when either axis snapped.
    pub snapped: bool,
    pub guides: Vec<SnapGuide>,
}

/// Constrain `p` to the nearest 45° ray from `origin`, keeping the
/// projected distance (Shift while drawing / dragging).
pub fn constrain_45(origin: Pt, p: Pt) -> Pt {
    let v = (p.0 - origin.0, p.1 - origin.1);
    let len = v.0.hypot(v.1);
    if len < 1e-12 {
        return p;
    }
    let angle = (v.1.atan2(v.0) / FRAC_PI_4).round() * FRAC_PI_4;
    let (s, c) = angle.sin_cos();
    let proj = v.0 * c + v.1 * s;
    (origin.0 + c * proj, origin.1 + s * proj)
}

/// Round to whole document pixels.
pub fn snap_to_pixel_grid(p: Pt) -> Pt {
    (p.0.round(), p.1.round())
}

/// Snap `p` to `candidates` within `threshold`: a candidate point closer
/// than the threshold wins outright; otherwise x and y snap
/// independently to the nearest candidate coordinate (smart-guide style
/// alignment), each with a guide from the aligned candidate.
pub fn snap_point(p: Pt, candidates: &[Pt], threshold: f64) -> SnapResult {
    let mut best_point: Option<(f64, Pt)> = None;
    let mut best_x: Option<(f64, Pt)> = None;
    let mut best_y: Option<(f64, Pt)> = None;
    for c in candidates {
        let d = (c.0 - p.0).hypot(c.1 - p.1);
        if d <= threshold && best_point.is_none_or(|(bd, _)| d < bd) {
            best_point = Some((d, *c));
        }
        let dx = (c.0 - p.0).abs();
        if dx <= threshold && best_x.is_none_or(|(bd, _)| dx < bd) {
            best_x = Some((dx, *c));
        }
        let dy = (c.1 - p.1).abs();
        if dy <= threshold && best_y.is_none_or(|(bd, _)| dy < bd) {
            best_y = Some((dy, *c));
        }
    }
    if let Some((_, c)) = best_point {
        return SnapResult {
            point: c,
            on_point: true,
            snapped: true,
            guides: Vec::new(),
        };
    }
    let mut point = p;
    let mut guides = Vec::new();
    if let Some((_, c)) = best_x {
        point.0 = c.0;
    }
    if let Some((_, c)) = best_y {
        point.1 = c.1;
    }
    if let Some((_, c)) = best_x {
        guides.push(SnapGuide {
            from: c,
            to: (c.0, point.1),
        });
    }
    if let Some((_, c)) = best_y {
        guides.push(SnapGuide {
            from: c,
            to: (point.0, c.1),
        });
    }
    SnapResult {
        point,
        on_point: false,
        snapped: best_x.is_some() || best_y.is_some(),
        guides,
    }
}
