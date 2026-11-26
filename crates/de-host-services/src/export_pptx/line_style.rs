//! DrawingML spelling of a stroke's cap, join, miter limit and line ends
//! (#89, #389): the `cap` attribute of `<a:ln>` and the join /
//! `<a:headEnd>` / `<a:tailEnd>` children that follow its dash.
//!
//! DrawingML has one cap for both ends and no reversed arrowhead, so a
//! line whose ends it cannot spell is rastered instead
//! ([`unsupported_line_ends`]).

use de_editor_ui::layout_scene::{StrokeCap, StrokeEnd, StrokeJoin, StrokeStyle};

/// The `cap` attribute value for `style`.
pub fn cap_attr(style: &StrokeStyle) -> &'static str {
    match plain_cap(style) {
        StrokeCap::Butt => "flat",
        StrokeCap::Round => "rnd",
        StrokeCap::Square => "sq",
    }
}

/// The join element, `<a:headEnd>` and `<a:tailEnd>` for `style`, in the
/// order `<a:ln>` takes them after `<a:prstDash>`.
pub fn ln_children(style: &StrokeStyle, width: f32) -> String {
    let mut out = String::new();
    match style.join {
        Some(StrokeJoin::Round) => out.push_str("<a:round/>"),
        Some(StrokeJoin::Bevel) => out.push_str("<a:bevel/>"),
        Some(StrokeJoin::Miter) | None if style.join.is_some() || style.miter_limit.is_some() => {
            // `lim` is in thousandths of a percent of the line width.
            let limit = style.miter_limit.unwrap_or(4.0);
            out.push_str(&format!(
                "<a:miter lim=\"{}\"/>",
                (limit * 100_000.0).round() as i64
            ));
        }
        _ => {}
    }
    let size = arrow_size(width);
    for (tag, end) in [("headEnd", style.start), ("tailEnd", style.end)] {
        if let Some(kind) = end.and_then(arrow_type) {
            out.push_str(&format!(
                "<a:{tag} type=\"{kind}\" w=\"{size}\" len=\"{size}\"/>"
            ));
        }
    }
    out
}

/// Why a stroke's ends cannot be written as DrawingML line ends, or
/// `None` when they can: a reversed triangle has no preset, and a round
/// or square cap on one end only cannot share the line's single `cap`.
pub fn unsupported_line_ends(style: &StrokeStyle) -> Option<&'static str> {
    if style.start.is_none() && style.end.is_none() {
        return None;
    }
    let ends = [end_or_cap(style, style.start), end_or_cap(style, style.end)];
    if ends.contains(&StrokeEnd::ReversedTriangle) {
        return Some("reversed triangle line end");
    }
    let caps: Vec<StrokeEnd> = ends.into_iter().filter(|e| !e.is_marker()).collect();
    if caps.len() == 2 && caps[0] != caps[1] {
        return Some("different caps on the two line ends");
    }
    None
}

/// The cap the line carries: the plain cap of an unmarked end when one
/// is set, else the authored `cap` (absent = flat).
fn plain_cap(style: &StrokeStyle) -> StrokeCap {
    let cap_of = |end: StrokeEnd| match end {
        StrokeEnd::Round => Some(StrokeCap::Round),
        StrokeEnd::Square => Some(StrokeCap::Square),
        StrokeEnd::None => Some(StrokeCap::Butt),
        _ => None,
    };
    [style.start, style.end]
        .into_iter()
        .flatten()
        .find_map(cap_of)
        .or(style.cap)
        .unwrap_or(StrokeCap::Butt)
}

fn end_or_cap(style: &StrokeStyle, end: Option<StrokeEnd>) -> StrokeEnd {
    end.unwrap_or(match style.cap {
        Some(StrokeCap::Round) => StrokeEnd::Round,
        Some(StrokeCap::Square) => StrokeEnd::Square,
        _ => StrokeEnd::None,
    })
}

fn arrow_type(end: StrokeEnd) -> Option<&'static str> {
    match end {
        StrokeEnd::LineArrow => Some("arrow"),
        StrokeEnd::TriangleArrow => Some("triangle"),
        StrokeEnd::Circle => Some("oval"),
        StrokeEnd::Diamond => Some("diamond"),
        StrokeEnd::None | StrokeEnd::Round | StrokeEnd::Square | StrokeEnd::ReversedTriangle => {
            None
        }
    }
}

/// PowerPoint sizes line ends in multiples of the line width (small ≈ 2,
/// medium ≈ 3, large ≈ 5); pick the one nearest the canvas head.
fn arrow_size(width: f32) -> &'static str {
    let ratio = de_editor_core::stroke_geometry::head_length(f64::from(width.max(0.1)))
        / f64::from(width.max(0.1));
    if ratio >= 4.0 {
        "lg"
    } else if ratio >= 2.5 {
        "med"
    } else {
        "sm"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(start: Option<StrokeEnd>, end: Option<StrokeEnd>) -> StrokeStyle {
        StrokeStyle {
            start,
            end,
            ..StrokeStyle::default()
        }
    }

    #[test]
    fn arrowheads_become_head_and_tail_ends() {
        let s = style(Some(StrokeEnd::Circle), Some(StrokeEnd::TriangleArrow));
        let xml = ln_children(&s, 2.0);
        assert!(xml.contains("<a:headEnd type=\"oval\""), "{xml}");
        assert!(xml.contains("<a:tailEnd type=\"triangle\""), "{xml}");
        assert!(xml.find("headEnd") < xml.find("tailEnd"));
        assert_eq!(unsupported_line_ends(&s), None);
        assert_eq!(cap_attr(&s), "flat");
    }

    #[test]
    fn joins_and_miter_limit_are_written() {
        let s = StrokeStyle {
            join: Some(StrokeJoin::Miter),
            miter_limit: Some(8.0),
            ..StrokeStyle::default()
        };
        assert_eq!(ln_children(&s, 1.0), "<a:miter lim=\"800000\"/>");
        let round = StrokeStyle {
            join: Some(StrokeJoin::Round),
            cap: Some(StrokeCap::Round),
            ..StrokeStyle::default()
        };
        assert_eq!(ln_children(&round, 1.0), "<a:round/>");
        assert_eq!(cap_attr(&round), "rnd");
        assert_eq!(ln_children(&StrokeStyle::default(), 1.0), "");
    }

    #[test]
    fn ends_drawingml_cannot_spell_are_reported() {
        assert!(unsupported_line_ends(&style(None, Some(StrokeEnd::ReversedTriangle))).is_some());
        assert!(
            unsupported_line_ends(&style(Some(StrokeEnd::Round), Some(StrokeEnd::Square)))
                .is_some()
        );
        // An unset end falls back to `cap`: round on both ends is fine.
        let round = StrokeStyle {
            cap: Some(StrokeCap::Round),
            ..style(None, Some(StrokeEnd::Round))
        };
        assert_eq!(unsupported_line_ends(&round), None);
        assert_eq!(cap_attr(&round), "rnd");
    }
}
