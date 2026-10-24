//! Angular (conic) gradient emission for the SVG serializer.
//!
//! SVG has no conic gradient, so the sweep is approximated by a
//! `<pattern>` of thin wedges around the centre, each filled with the
//! stop colour sampled at its mid-angle. The pattern keeps the
//! `gradient-{id}` id the fill attribute already references, so shapes
//! need no special casing.

use super::color_to_rgb;
use crate::layout_scene::SceneGradientStop;
use crate::{Color, Rect};
use std::fmt::Write as _;

/// Wedges per full turn: 2° each, fine enough that banding stays below
/// what a typical 8-bit export shows for smooth stops.
const WEDGES: usize = 180;

/// Fraction of a wedge an opaque wedge overlaps the next one by, so
/// anti-aliased edges between wedges don't show hairline seams.
const SEAM_OVERLAP: f32 = 0.35;

pub(super) struct AngularSpec<'a> {
    pub cx: f32,
    pub cy: f32,
    pub rotation_deg: f32,
    pub opacity: f32,
    pub stops: &'a [SceneGradientStop],
}

pub(super) fn emit_angular_pattern(out: &mut String, id: &str, spec: &AngularSpec<'_>, b: Rect) {
    let (w, h) = (b.size.x.max(0.01), b.size.y.max(0.01));
    let _ = write!(
        out,
        r#"<pattern id="{id}" patternUnits="userSpaceOnUse" x="{}" y="{}" width="{w}" height="{h}">"#,
        b.origin.x, b.origin.y,
    );
    // Pattern content is relative to the tile origin.
    let cx = w * spec.cx.clamp(0.0, 1.0);
    let cy = h * spec.cy.clamp(0.0, 1.0);
    let reach = (cx.max(w - cx)).hypot(cy.max(h - cy)) + 1.0;
    let point = |t: f32| {
        // CSS conic convention: 0° = 12 o'clock, clockwise (y down).
        let a = (spec.rotation_deg + t * 360.0).to_radians();
        (cx + reach * a.sin(), cy - reach * a.cos())
    };
    let opacity = spec.opacity.clamp(0.0, 1.0);
    let mut sorted = spec.stops.to_vec();
    sorted.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    for i in 0..WEDGES {
        let t0 = i as f32 / WEDGES as f32;
        let mid = (i as f32 + 0.5) / WEDGES as f32;
        let color = sample_stops(&sorted, mid);
        let alpha = color.a * opacity;
        if alpha <= 0.0 {
            continue;
        }
        // Overlap only opaque wedges: a translucent overlap would
        // darken the seam instead of hiding it.
        let overlap = if alpha >= 1.0 && i + 1 < WEDGES {
            SEAM_OVERLAP
        } else {
            0.0
        };
        let t1 = (i as f32 + 1.0 + overlap) / WEDGES as f32;
        let (x0, y0) = point(t0);
        let (x1, y1) = point(t1);
        let _ = write!(
            out,
            r#"<path d="M{cx:.2} {cy:.2}L{x0:.2} {y0:.2}L{x1:.2} {y1:.2}Z" fill="{}""#,
            color_to_rgb(color),
        );
        if alpha < 1.0 {
            let _ = write!(out, r#" fill-opacity="{alpha:.3}""#);
        }
        out.push_str("/>");
    }
    out.push_str("</pattern>");
}

/// Colour of a stop list at `t` (0..=1): linear interpolation between
/// the neighbouring stops, clamped to the end stops outside them.
/// `sorted` must be ordered by offset.
pub(super) fn sample_stops(sorted: &[SceneGradientStop], t: f32) -> Color {
    let (Some(first), Some(last)) = (sorted.first(), sorted.last()) else {
        return Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
    };
    if t <= first.offset {
        return first.color;
    }
    if t >= last.offset {
        return last.color;
    }
    for pair in sorted.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if t <= b.offset {
            let span = (b.offset - a.offset).max(f32::EPSILON);
            let k = ((t - a.offset) / span).clamp(0.0, 1.0);
            let lerp = |x: f32, y: f32| x + (y - x) * k;
            return Color {
                r: lerp(a.color.r, b.color.r),
                g: lerp(a.color.g, b.color.g),
                b: lerp(a.color.b, b.color.b),
                a: lerp(a.color.a, b.color.a),
            };
        }
    }
    last.color
}
