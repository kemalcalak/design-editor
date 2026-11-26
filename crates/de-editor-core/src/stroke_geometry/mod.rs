//! Stroke geometry the backends do not draw themselves (#389): per-end
//! caps and arrowheads on open strokes. One function turns a path's
//! subpaths into three SVG path strings in the path's own coordinates —
//! the centre line to stroke (trimmed under filled heads), the filled end
//! shapes, and the line-arrow arms to stroke — so the canvas, the SVG,
//! HTML and PNG exports all draw the same thing.
//!
//! Head sizes follow the stroke width ([`head_length`]); the tip of a
//! pointing head sits on the path's end point, as in Figma.

mod heads;
mod measure;

use crate::vector_edit::{segment_controls, segment_count, Pt, Subpath};
use jian_ops_schema::style::{PenStroke, StrokeCap, StrokeEndCap};

pub use heads::head_length;

/// What each end of an open stroke draws, after `cap` fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrokeEnds {
    pub start: StrokeEndCap,
    pub end: StrokeEndCap,
}

impl StrokeEnds {
    /// The ends a stroke asks for, or `None` when it sets neither
    /// `strokeStart` nor `strokeEnd` (backends draw its `cap` as before).
    /// An unset end falls back to `cap` (absent = butt).
    pub fn of(stroke: &PenStroke) -> Option<Self> {
        Self::from_parts(stroke.stroke_start, stroke.stroke_end, stroke.cap.as_ref())
    }

    /// [`Self::of`] from a stroke's `strokeStart`, `strokeEnd` and `cap`.
    pub fn from_parts(
        start: Option<StrokeEndCap>,
        end: Option<StrokeEndCap>,
        cap: Option<&StrokeCap>,
    ) -> Option<Self> {
        if start.is_none() && end.is_none() {
            return None;
        }
        let cap = match cap {
            Some(StrokeCap::Round) => StrokeEndCap::Round,
            Some(StrokeCap::Square) => StrokeEndCap::Square,
            Some(StrokeCap::None) | None => StrokeEndCap::None,
        };
        Some(Self {
            start: start.unwrap_or(cap),
            end: end.unwrap_or(cap),
        })
    }
}

/// The drawable parts of an open stroke with end shapes, as SVG path data
/// in the source path's coordinates. Any string may be empty.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StrokeGeometry {
    /// Centre line, stroked at the stroke width with butt caps (and the
    /// authored join / dash): trimmed where a filled head covers its end.
    pub stroke_d: String,
    /// End shapes filled with the stroke paint (arrowheads, dots,
    /// diamonds, per-end round / square caps).
    pub fill_d: String,
    /// Line-arrow arms, stroked at the stroke width with miter joins and
    /// butt caps, never dashed.
    pub arms_d: String,
}

impl StrokeGeometry {
    pub fn is_empty(&self) -> bool {
        self.stroke_d.is_empty() && self.fill_d.is_empty() && self.arms_d.is_empty()
    }
}

/// End geometry of `subs` stroked `width` wide, or `None` when there is
/// nothing to add: no open subpath, a degenerate width, or no end shape
/// at either end. The start of the first open subpath and the end of the
/// last one carry the shapes; other subpaths are stroked as they are.
pub fn stroke_end_geometry(
    subs: &[Subpath],
    width: f64,
    ends: StrokeEnds,
) -> Option<StrokeGeometry> {
    if !(width.is_finite() && width > 0.0)
        || (ends.start == StrokeEndCap::None && ends.end == StrokeEndCap::None)
    {
        return None;
    }
    let mut runs: Vec<(Vec<[Pt; 4]>, bool)> = subs
        .iter()
        .map(|sub| {
            let segs = (0..segment_count(sub))
                .filter_map(|seg| segment_controls(sub, seg))
                .collect::<Vec<_>>();
            (segs, sub.closed)
        })
        .filter(|(segs, _)| !segs.is_empty())
        .collect();
    let first = runs
        .iter()
        .position(|(segs, closed)| !closed && !segs.is_empty())?;
    let last = runs
        .iter()
        .rposition(|(segs, closed)| !closed && !segs.is_empty())?;

    let mut out = StrokeGeometry::default();
    // Start: the head points back, away from the first segment.
    if let Some((tip, dir)) = measure::start_frame(&runs[first].0) {
        let trim = heads::push_head(&mut out, tip, dir, width, ends.start);
        measure::trim_start(&mut runs[first].0, trim);
    }
    if let Some((tip, dir)) = measure::end_frame(&runs[last].0) {
        let trim = heads::push_head(&mut out, tip, dir, width, ends.end);
        measure::trim_end(&mut runs[last].0, trim);
    }
    if out.fill_d.is_empty() && out.arms_d.is_empty() {
        // Neither end had a direction to draw a shape along.
        return None;
    }
    for (segs, closed) in &runs {
        write_run(&mut out.stroke_d, segs, *closed);
    }
    Some(out)
}

/// Append one run of cubics to `d` as `M … C … [Z]`.
fn write_run(d: &mut String, segs: &[[Pt; 4]], closed: bool) {
    let Some(first) = segs.first() else {
        return;
    };
    push_cmd(d, 'M', &[first[0]]);
    for c in segs {
        if is_straight(c) {
            push_cmd(d, 'L', &[c[3]]);
        } else {
            push_cmd(d, 'C', &[c[1], c[2], c[3]]);
        }
    }
    if closed {
        d.push_str(" Z");
    }
}

fn is_straight(c: &[Pt; 4]) -> bool {
    c[1] == c[0] && c[2] == c[3]
}

/// Append ` X x,y …` with coordinates rounded to 1/1000 px.
pub(crate) fn push_cmd(d: &mut String, cmd: char, points: &[Pt]) {
    use std::fmt::Write;
    if !d.is_empty() {
        d.push(' ');
    }
    d.push(cmd);
    for (x, y) in points {
        let _ = write!(d, " {} {}", num(*x), num(*y));
    }
}

/// `v` rounded to 1/1000 with trailing zeros dropped (`-0` reads `0`).
pub(crate) fn num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 {
        "0".to_owned()
    } else {
        format!("{r}")
    }
}

#[cfg(test)]
mod tests;
