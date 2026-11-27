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
use jian_ops_schema::style::{StrokeCap, StrokeEndCap};

pub use heads::head_length;

/// What each end of an open stroke draws, after `cap` fallback, and the
/// cap itself (which the inner ends of a multi-subpath stroke and the
/// dashes keep).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrokeEnds {
    pub start: StrokeEndCap,
    pub end: StrokeEndCap,
    /// The authored `cap` as an end (absent = round).
    pub cap: StrokeEndCap,
}

impl StrokeEnds {
    /// [`Self::from_parts`] on a schema stroke (tests).
    #[cfg(test)]
    pub(crate) fn of(stroke: &jian_ops_schema::style::PenStroke) -> Option<Self> {
        Self::from_parts(stroke.stroke_start, stroke.stroke_end, stroke.cap.as_ref())
    }

    /// The ends a stroke's `strokeStart`, `strokeEnd` and `cap` ask for,
    /// or `None` when there is nothing the backends' own `cap` does not
    /// already draw: neither end is set, or both equal the authored
    /// `cap`. An unset end falls back to `cap`; an absent `cap` is round,
    /// which is what every renderer draws on an open stroke.
    pub fn from_parts(
        start: Option<StrokeEndCap>,
        end: Option<StrokeEndCap>,
        cap: Option<&StrokeCap>,
    ) -> Option<Self> {
        if start.is_none() && end.is_none() {
            return None;
        }
        let authored = cap.map(cap_end);
        let cap = authored.unwrap_or(StrokeEndCap::Round);
        let ends = Self {
            start: start.unwrap_or(cap),
            end: end.unwrap_or(cap),
            cap,
        };
        if authored.is_some_and(|cap| ends.start == cap && ends.end == cap) {
            return None;
        }
        Some(ends)
    }
}

/// A schema cap as the end it draws.
pub fn cap_end(cap: &StrokeCap) -> StrokeEndCap {
    match cap {
        StrokeCap::Round => StrokeEndCap::Round,
        StrokeCap::Square => StrokeEndCap::Square,
        StrokeCap::None => StrokeEndCap::None,
    }
}

/// How the centre line is stroked, which decides how far it may reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CentreLine {
    /// Solid: stroked with butt caps, every end drawn as a fill shape.
    #[default]
    Solid,
    /// Dashed: every dash keeps the authored cap (dots are zero-length
    /// dashes only a round cap draws), so the line is shortened by half a
    /// width at both of its ends and no dash cap pokes past them.
    Dashed,
}

/// The drawable parts of an open stroke with end shapes, as SVG path data
/// in the source path's coordinates. Any string may be empty.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StrokeGeometry {
    /// Centre line, trimmed where a filled head covers its end. Solid:
    /// stroked with butt caps (and the authored join / dash). Dashed:
    /// stroked with the authored cap (see [`CentreLine::Dashed`]).
    pub stroke_d: String,
    /// End shapes filled with the stroke paint (arrowheads, dots,
    /// diamonds, per-end round / square caps, and on a solid line the
    /// authored cap on the inner ends of a multi-subpath stroke). Filled
    /// with the nonzero rule.
    pub fill_d: String,
    /// Line-arrow arms, stroked at the stroke width with miter joins and
    /// butt caps, never dashed.
    pub arms_d: String,
    /// One disc per marker end, covering it, for hit-testing.
    pub heads: Vec<HeadCover>,
}

/// A disc covering one end shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadCover {
    pub center: Pt,
    pub radius: f64,
}

impl StrokeGeometry {
    pub fn is_empty(&self) -> bool {
        self.stroke_d.is_empty() && self.fill_d.is_empty() && self.arms_d.is_empty()
    }
}

/// End geometry of `subs` stroked `width` wide, or `None` when there is
/// nothing to add: no open subpath, a degenerate width, or no direction
/// at either end to draw along. The start of the first open subpath and
/// the end of the last one carry the shapes; the other open ends keep the
/// authored cap (a fill shape on a solid line, the dash cap on a dashed
/// one), and closed subpaths are stroked as they are.
pub fn stroke_end_geometry(
    subs: &[Subpath],
    width: f64,
    ends: StrokeEnds,
    centre: CentreLine,
) -> Option<StrokeGeometry> {
    if !(width.is_finite() && width > 0.0) {
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
    let first = runs.iter().position(|(_, closed)| !closed)?;
    let last = runs.iter().rposition(|(_, closed)| !closed)?;
    let end = EndPass {
        width,
        cap: ends.cap,
        centre,
    };
    let mut out = StrokeGeometry::default();
    let mut framed = false;
    for (i, (segs, closed)) in runs.iter_mut().enumerate().take(last + 1).skip(first) {
        if *closed {
            continue;
        }
        // The path's own two ends carry `ends.start` / `ends.end`; every
        // other open end keeps the cap, drawn as a fill shape on a solid
        // line and by the dashes themselves on a dashed one.
        let at_start = (i == first).then_some(ends.start);
        let at_end = (i == last).then_some(ends.end);
        if let Some((tip, dir)) = measure::start_frame(segs) {
            framed = true;
            let trim = end.draw(&mut out, tip, dir, at_start);
            measure::trim_start(segs, trim);
        }
        if let Some((tip, dir)) = measure::end_frame(segs) {
            framed = true;
            let trim = end.draw(&mut out, tip, dir, at_end);
            measure::trim_end(segs, trim);
        }
    }
    if !framed {
        // No end had a direction to draw a shape along.
        return None;
    }
    for (segs, closed) in &runs {
        write_run(&mut out.stroke_d, segs, *closed);
    }
    Some(out)
}

/// What every open end of one stroke shares.
struct EndPass {
    width: f64,
    cap: StrokeEndCap,
    centre: CentreLine,
}

impl EndPass {
    /// Draw one open end and return how far the centre line must be
    /// shortened there. `head` is the shape the path's own end asks for
    /// (`None` for an inner end, which keeps `cap`).
    fn draw(&self, out: &mut StrokeGeometry, tip: Pt, dir: Pt, head: Option<StrokeEndCap>) -> f64 {
        match (head, self.centre) {
            (Some(head), _) => heads::push_head(out, tip, dir, self.width, head) + self.overhang(),
            (None, CentreLine::Solid) => heads::push_head(out, tip, dir, self.width, self.cap),
            (None, CentreLine::Dashed) => 0.0,
        }
    }

    /// How far a dash cap reaches past where its dash ends: half a width
    /// for a round or square cap on a dashed line, so a head end is
    /// shortened by it and no dash cap pokes past the end or into a head.
    fn overhang(&self) -> f64 {
        match (self.centre, self.cap) {
            (CentreLine::Dashed, StrokeEndCap::Round | StrokeEndCap::Square) => self.width / 2.0,
            _ => 0.0,
        }
    }
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
