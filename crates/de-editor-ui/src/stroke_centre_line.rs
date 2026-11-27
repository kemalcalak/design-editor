//! The centre line of a stroke with end shapes (#389), styled one way for
//! every writer: the canvas painter and the SVG export both stroke it with
//! [`centre_line_style`], so a file and the screen draw the same caps.

use crate::layout_scene::{StrokeCap, StrokeStyle};

/// The style a precomputed centre line is stroked with (#389): the
/// stroke's own join / dash / miter limit, with butt caps on a solid line
/// (its ends are fill shapes) and the authored cap on a dashed one, whose
/// dashes need it (dots are zero-length dashes only a round cap draws, so
/// a dotted line never goes invisible). An unset cap on a dashed open
/// stroke is round, as everywhere else for open strokes. The geometry is
/// already shortened so no dash cap reaches past an end.
pub(crate) fn centre_line_style(style: StrokeStyle) -> StrokeStyle {
    let cap = if style.dash.is_some() {
        style.cap.unwrap_or(StrokeCap::Round)
    } else {
        StrokeCap::Butt
    };
    StrokeStyle {
        cap: Some(cap),
        start: None,
        end: None,
        ..style
    }
}
