//! The Stroke section's style rows, under the colour / width rows:
//! the Inside / Center / Outside alignment row (#90). Caps, joins and
//! dashes (#89) stack below it — add the row here, grow
//! [`stroke_style_rows_height`], and emit its rects from
//! [`push_stroke_style_action_rects`]; every y-walk already reads the
//! section height from `property_panel_stroke`.

use crate::layout_scene::SceneStrokeAlign;
use crate::theme::Theme;
use crate::widgets::property_panel::NodeSnapshot;
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{INPUT_HEIGHT, PAD_X};
use crate::widgets::property_panel_sections::PropertyLabels;
use crate::widgets::property_panel_segments::{paint_segment_row, segment_rects};
use crate::widgets::PaintCx;
use crate::Rect;
use jian_ops_schema::style::StrokeAlign;

/// Gap between the rows above and the first style row.
const STYLE_ROWS_TOP_GAP: f32 = 6.0;

/// Alignment cells, left to right.
pub(crate) const STROKE_ALIGNS: [StrokeAlign; 3] = [
    StrokeAlign::Inside,
    StrokeAlign::Center,
    StrokeAlign::Outside,
];

/// Height the style rows add to the Stroke body.
pub(crate) fn stroke_style_rows_height() -> f32 {
    STYLE_ROWS_TOP_GAP + INPUT_HEIGHT
}

/// The alignment row, given the y where the style rows start.
fn align_row_rect(x: f32, top: f32, width: f32) -> Rect {
    Rect::xywh(
        x + PAD_X,
        top + STYLE_ROWS_TOP_GAP,
        width - PAD_X * 2.0,
        INPUT_HEIGHT,
    )
}

pub(crate) fn push_stroke_style_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x: f32,
    top: f32,
    width: f32,
) {
    let cells = segment_rects(align_row_rect(x, top, width), STROKE_ALIGNS.len());
    for (align, cell) in STROKE_ALIGNS.iter().zip(cells) {
        out.push((PropertyPanelAction::SetStrokeAlign(align.clone()), cell));
    }
}

/// Paint the style rows. A node without a stroke shows no active cell:
/// alignment only applies once a stroke exists.
pub(crate) fn paint_stroke_style_rows(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    snapshot: &NodeSnapshot,
    x: f32,
    top: f32,
    width: f32,
) {
    let active = snapshot.stroke.as_ref().map(|stroke| match stroke.align {
        SceneStrokeAlign::Inside => 0,
        SceneStrokeAlign::Center => 1,
        SceneStrokeAlign::Outside => 2,
    });
    paint_segment_row(
        cx,
        theme,
        align_row_rect(x, top, width),
        &[
            labels.stroke_inside,
            labels.stroke_center,
            labels.stroke_outside,
        ],
        active,
        11.0,
    );
}
