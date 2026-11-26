//! The Stroke section's style rows, under the colour / width rows: the
//! Inside / Center / Outside alignment row (#90), the Cap, Join and Dash
//! rows (#89), then the Start / End rows of an open stroke and the Miter
//! field (#389, `property_panel_stroke_ends`). Add a row here, grow
//! [`stroke_style_rows_height`], and emit its rects from
//! [`push_stroke_style_action_rects`] / [`stroke_style_input_rects`];
//! every y-walk already reads the section height from
//! `property_panel_stroke`.

use crate::layout_scene::SceneStrokeAlign;
use crate::theme::Theme;
use crate::widgets::property_panel::NodeSnapshot;
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{INPUT_HEIGHT, INPUT_TEXT_BASELINE, PAD_X};
use crate::widgets::property_panel_sections::{EditContext, PropertyLabels};
use crate::widgets::property_panel_segments::{paint_segment_row, segment_rects};
use crate::widgets::property_panel_stroke::StrokeStyleRows;
use crate::widgets::property_panel_stroke_ends as ends;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::fills::{StrokeDashPreset, StrokeStyleEdit};
use de_editor_core::PropertyFocus;
use jian_ops_schema::style::{StrokeAlign, StrokeCap, StrokeJoin};

/// Gap between the rows above and the first style row.
const STYLE_ROWS_TOP_GAP: f32 = 6.0;
/// Gap between two style rows.
const STYLE_ROW_GAP: f32 = 6.0;
/// Width of the Cap / Join / Dash row label column.
const ROW_LABEL_W: f32 = 44.0;
const LABEL_FONT_SIZE: f32 = 11.0;

/// Alignment cells, left to right.
pub(crate) const STROKE_ALIGNS: [StrokeAlign; 3] = [
    StrokeAlign::Inside,
    StrokeAlign::Center,
    StrokeAlign::Outside,
];
/// Cap cells, left to right.
pub(crate) const STROKE_CAPS: [StrokeCap; 3] =
    [StrokeCap::None, StrokeCap::Round, StrokeCap::Square];
/// Join cells, left to right.
pub(crate) const STROKE_JOINS: [StrokeJoin; 3] =
    [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel];

/// The labelled rows under the alignment row.
#[derive(Clone, Copy)]
enum StyleRow {
    Cap,
    Join,
    Dash,
}

const STYLE_ROWS: [StyleRow; 3] = [StyleRow::Cap, StyleRow::Join, StyleRow::Dash];

/// Height the style rows add to the Stroke body. The Cap / Join / Dash
/// rows (`styled`) only show once the node has a stroke: they style an
/// existing stroke and nothing else; Start / End and Miter follow
/// [`StrokeStyleRows`].
pub(crate) fn stroke_style_rows_height(rows: StrokeStyleRows) -> f32 {
    let labelled = labelled_rows(rows) as f32;
    STYLE_ROWS_TOP_GAP + INPUT_HEIGHT + labelled * (STYLE_ROW_GAP + INPUT_HEIGHT)
}

/// How many rows follow the alignment row.
fn labelled_rows(rows: StrokeStyleRows) -> usize {
    if !rows.styled {
        return 0;
    }
    STYLE_ROWS.len() + if rows.ends { 2 } else { 0 } + usize::from(rows.miter)
}

/// Row index of the Start row (End follows it), after Cap / Join / Dash.
const START_ROW: usize = 1 + STYLE_ROWS.len();

/// Row index of the Miter row, under the end rows when they show.
fn miter_row(rows: StrokeStyleRows) -> usize {
    START_ROW + if rows.ends { 2 } else { 0 }
}

/// Row `index` (0 = alignment), given the y where the style rows start.
fn row_rect(x: f32, top: f32, width: f32, index: usize) -> Rect {
    Rect::xywh(
        x + PAD_X,
        top + STYLE_ROWS_TOP_GAP + index as f32 * (INPUT_HEIGHT + STYLE_ROW_GAP),
        width - PAD_X * 2.0,
        INPUT_HEIGHT,
    )
}

/// The segmented part of a labelled row (right of its label).
fn labelled_segments_rect(row: Rect) -> Rect {
    Rect::xywh(
        row.origin.x + ROW_LABEL_W,
        row.origin.y,
        (row.size.x - ROW_LABEL_W).max(0.0),
        row.size.y,
    )
}

fn row_edits(row: StyleRow) -> [StrokeStyleEdit; 3] {
    match row {
        StyleRow::Cap => STROKE_CAPS.map(StrokeStyleEdit::Cap),
        StyleRow::Join => STROKE_JOINS.map(StrokeStyleEdit::Join),
        StyleRow::Dash => StrokeDashPreset::ALL.map(StrokeStyleEdit::Dash),
    }
}

pub(crate) fn push_stroke_style_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x: f32,
    top: f32,
    width: f32,
    rows: StrokeStyleRows,
) {
    let cells = segment_rects(row_rect(x, top, width, 0), STROKE_ALIGNS.len());
    for (align, cell) in STROKE_ALIGNS.iter().zip(cells) {
        out.push((PropertyPanelAction::SetStrokeAlign(align.clone()), cell));
    }
    if !rows.styled {
        return;
    }
    for (i, row) in STYLE_ROWS.into_iter().enumerate() {
        let edits = row_edits(row);
        let segments = labelled_segments_rect(row_rect(x, top, width, i + 1));
        let cells = segment_rects(segments, edits.len());
        for (edit, cell) in edits.into_iter().zip(cells) {
            out.push((PropertyPanelAction::SetStrokeStyle(edit), cell));
        }
    }
    if rows.ends {
        ends::push_end_action_rects(
            out,
            row_rect(x, top, width, START_ROW),
            row_rect(x, top, width, START_ROW + 1),
        );
    }
}

/// The Miter field's rect while it shows (#389).
pub(crate) fn stroke_style_input_rects(
    x: f32,
    top: f32,
    width: f32,
    rows: StrokeStyleRows,
) -> Vec<(PropertyFocus, Rect)> {
    if !(rows.styled && rows.miter) {
        return Vec::new();
    }
    vec![(
        PropertyFocus::StrokeMiterLimit,
        ends::miter_input_rect(row_rect(x, top, width, miter_row(rows))),
    )]
}

/// Paint the style rows. A node without a stroke shows no active cell:
/// the style only applies once a stroke exists.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_stroke_style_rows(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    x: f32,
    top: f32,
    width: f32,
) {
    let active = snapshot.stroke.as_ref().and_then(|stroke| {
        let align = match stroke.align {
            SceneStrokeAlign::Inside => StrokeAlign::Inside,
            SceneStrokeAlign::Center => StrokeAlign::Center,
            SceneStrokeAlign::Outside => StrokeAlign::Outside,
        };
        STROKE_ALIGNS.iter().position(|a| *a == align)
    });
    paint_segment_row(
        cx,
        theme,
        row_rect(x, top, width, 0),
        &[
            labels.stroke_inside,
            labels.stroke_center,
            labels.stroke_outside,
        ],
        active,
        LABEL_FONT_SIZE,
    );
    let Some(style) = snapshot.stroke.as_ref().and(snapshot.stroke_style.as_ref()) else {
        // No stroke yet: only the alignment row shows.
        return;
    };
    let style = Some(style);
    for (i, row) in STYLE_ROWS.into_iter().enumerate() {
        let (label, cells, active) = match row {
            StyleRow::Cap => (
                labels.stroke_cap,
                [
                    labels.stroke_cap_none,
                    labels.stroke_round,
                    labels.stroke_cap_square,
                ],
                style.and_then(|s| STROKE_CAPS.iter().position(|c| *c == s.cap)),
            ),
            StyleRow::Join => (
                labels.stroke_join,
                [
                    labels.stroke_join_miter,
                    labels.stroke_round,
                    labels.stroke_join_bevel,
                ],
                style.and_then(|s| STROKE_JOINS.iter().position(|j| *j == s.join)),
            ),
            StyleRow::Dash => (
                labels.stroke_dash,
                [
                    labels.stroke_dash_solid,
                    labels.stroke_dash_dashed,
                    labels.stroke_dash_dotted,
                ],
                style
                    .and_then(|s| s.dash)
                    .and_then(|d| StrokeDashPreset::ALL.iter().position(|p| *p == d)),
            ),
        };
        let row_rect = row_rect(x, top, width, i + 1);
        paint_row_label(cx, theme, label, row_rect, ROW_LABEL_W);
        paint_segment_row(
            cx,
            theme,
            labelled_segments_rect(row_rect),
            &cells,
            active,
            LABEL_FONT_SIZE,
        );
    }
    let rows = StrokeStyleRows::of(snapshot);
    if rows.ends {
        ends::paint_end_rows(
            cx,
            theme,
            labels,
            snapshot,
            row_rect(x, top, width, START_ROW),
            row_rect(x, top, width, START_ROW + 1),
        );
    }
    if rows.miter {
        let row = row_rect(x, top, width, miter_row(rows));
        ends::paint_miter_row(cx, theme, labels, snapshot, edit, row);
    }
}

/// A row's muted label, fitted into its `label_w` column.
pub(crate) fn paint_row_label(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    label: &str,
    row: Rect,
    label_w: f32,
) {
    let fitted = text_metrics::fit_chrome(cx.backend, label, label_w - 6.0, LABEL_FONT_SIZE);
    let layout = TextLayout::single_run(
        &fitted,
        text_metrics::CHROME_FONT_FAMILY,
        LABEL_FONT_SIZE,
        theme.muted_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(row.origin.x, row.origin.y + INPUT_TEXT_BASELINE),
    );
}
