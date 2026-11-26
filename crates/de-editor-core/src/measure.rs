//! Hover measurements (#399): the distances Figma shows while Alt / Option
//! is held with a selection and the pointer is over another layer.
//!
//! Pure geometry over two document-space boxes — the selection's and the
//! hovered target's — so the rules are unit-testable without a canvas:
//!
//! - **Target contains the selection** (hovering the parent frame): four
//!   lines from each selection edge out to the target's edge, through the
//!   selection's centre — the "padding".
//! - **Selection contains the target**: the same, inverted.
//! - **Apart on an axis**: one gap line between the facing edges. When the
//!   boxes overlap on the other axis it runs through the middle of that
//!   overlap; otherwise through the selection's centre, with a dashed
//!   guide from its end to the target so the line has something to point at.
//! - **Overlapping** (neither inside the other): the distance between each
//!   pair of matching edges that differ.
//!
//! Zero-length distances are left out, as Figma does.

use crate::DocRect;

/// A labelled distance line, document coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasureLine {
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// The distance the label shows (doc px, never negative).
    pub value: f64,
}

impl MeasureLine {
    /// True for a vertical line (it measures a vertical distance).
    pub fn vertical(&self) -> bool {
        (self.from.0 - self.to.0).abs() < 1e-9
    }
}

/// An unlabelled dashed extension from a gap line to the target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasureGuide {
    pub from: (f64, f64),
    pub to: (f64, f64),
}

/// Every line and guide between the selection and the target.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Measurement {
    pub lines: Vec<MeasureLine>,
    pub guides: Vec<MeasureGuide>,
}

#[derive(Clone, Copy)]
struct Edges {
    l: f64,
    t: f64,
    r: f64,
    b: f64,
}

impl Edges {
    fn of(r: DocRect) -> Self {
        Self {
            l: r.x,
            t: r.y,
            r: r.x + r.w,
            b: r.y + r.h,
        }
    }
    fn cx(&self) -> f64 {
        (self.l + self.r) / 2.0
    }
    fn cy(&self) -> f64 {
        (self.t + self.b) / 2.0
    }
    fn contains(&self, o: &Edges) -> bool {
        self.l <= o.l && self.t <= o.t && self.r >= o.r && self.b >= o.b
    }
}

const EPS: f64 = 1e-6;

/// The measurement between the selection box `sel` and the hovered
/// `target` box. Empty when the boxes coincide or either is degenerate.
pub fn measure(sel: DocRect, target: DocRect) -> Measurement {
    let mut out = Measurement::default();
    let finite = |r: DocRect| [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite());
    if !finite(sel) || !finite(target) || sel.w < 0.0 || sel.h < 0.0 {
        return out;
    }
    let s = Edges::of(sel);
    let g = Edges::of(target);
    if g.contains(&s) {
        inside_lines(&mut out, &s, &g);
    } else if s.contains(&g) {
        inside_lines(&mut out, &g, &s);
    } else {
        apart_or_overlap(&mut out, &s, &g);
    }
    out
}

/// Lines from each edge of `inner` out to the matching edge of `outer`,
/// through `inner`'s centre.
fn inside_lines(out: &mut Measurement, inner: &Edges, outer: &Edges) {
    let (cx, cy) = (inner.cx(), inner.cy());
    push(out, (outer.l, cy), (inner.l, cy));
    push(out, (inner.r, cy), (outer.r, cy));
    push(out, (cx, outer.t), (cx, inner.t));
    push(out, (cx, inner.b), (cx, outer.b));
}

fn apart_or_overlap(out: &mut Measurement, s: &Edges, g: &Edges) {
    let x_gap = gap(s.l, s.r, g.l, g.r);
    let y_gap = gap(s.t, s.b, g.t, g.b);
    if x_gap.is_none() && y_gap.is_none() {
        // Overlapping without containment: matching edges.
        let mid_y = (s.t.max(g.t) + s.b.min(g.b)) / 2.0;
        let mid_x = (s.l.max(g.l) + s.r.min(g.r)) / 2.0;
        push(out, (s.l.min(g.l), mid_y), (s.l.max(g.l), mid_y));
        push(out, (s.r.min(g.r), mid_y), (s.r.max(g.r), mid_y));
        push(out, (mid_x, s.t.min(g.t)), (mid_x, s.t.max(g.t)));
        push(out, (mid_x, s.b.min(g.b)), (mid_x, s.b.max(g.b)));
        return;
    }
    if let Some((a, b)) = x_gap {
        let y = match overlap_mid(s.t, s.b, g.t, g.b) {
            Some(mid) => mid,
            None => {
                let y = s.cy();
                // The target sits off this line: guide from the gap
                // line's target end up / down to the target.
                let target_x = if (a - s.r).abs() < EPS { b } else { a };
                let edge = if y < g.t { g.t } else { g.b };
                out.guides.push(MeasureGuide {
                    from: (target_x, y),
                    to: (target_x, edge),
                });
                y
            }
        };
        push(out, (a, y), (b, y));
    }
    if let Some((a, b)) = y_gap {
        let x = match overlap_mid(s.l, s.r, g.l, g.r) {
            Some(mid) => mid,
            None => {
                let x = s.cx();
                let target_y = if (a - s.b).abs() < EPS { b } else { a };
                let edge = if x < g.l { g.l } else { g.r };
                out.guides.push(MeasureGuide {
                    from: (x, target_y),
                    to: (edge, target_y),
                });
                x
            }
        };
        push(out, (x, a), (x, b));
    }
}

/// The open interval between two ranges that do not overlap, low end
/// first; `None` when they overlap or touch.
fn gap(a0: f64, a1: f64, b0: f64, b1: f64) -> Option<(f64, f64)> {
    if a1 <= b0 - EPS {
        Some((a1, b0))
    } else if b1 <= a0 - EPS {
        Some((b1, a0))
    } else {
        None
    }
}

/// Middle of the overlap of two ranges, when they overlap.
fn overlap_mid(a0: f64, a1: f64, b0: f64, b1: f64) -> Option<f64> {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    (hi >= lo).then_some((lo + hi) / 2.0)
}

fn push(out: &mut Measurement, from: (f64, f64), to: (f64, f64)) {
    let value = (to.0 - from.0).abs() + (to.1 - from.1).abs();
    if value > EPS {
        out.lines.push(MeasureLine { from, to, value });
    }
}

/// A distance as the label shows it: at most two decimals, no trailing
/// zeros (`12`, `12.5`, `0.33`).
pub fn format_measure_value(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
