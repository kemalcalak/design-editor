//! The Stroke section's end and miter rows (#389), under Cap / Join /
//! Dash: on an open stroke (a line, or a path with an open subpath) a
//! Start row (dropdown + swap button) and an End row (dropdown), then a
//! Miter field while the join is Miter. Each localized control owns its
//! row so the longest of the 15 translations fits (swept by the tests).
//!
//! The dropdowns open the shared Interactions popover
//! (`InteractionMenuTarget::StrokeEnd`), so the hosts' popover hit,
//! hover and dismiss routing covers them unchanged.

use crate::theme::Theme;
use crate::util::format_panel_number;
use crate::widgets::icons::{draw_icon_data, IconPathData};
use crate::widgets::property_panel::NodeSnapshot;
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, INPUT_HEIGHT, INPUT_RADIUS,
};
use crate::widgets::property_panel_sections::{EditContext, PropertyLabels};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::fills::StrokeStyleEdit;
use de_editor_core::interaction_edit::InteractionMenuTarget;
use de_editor_core::PropertyFocus;
use jian_ops_schema::style::StrokeEndCap;

/// Width of the Start / End / Miter label column.
pub(crate) const LABEL_W: f32 = 56.0;
/// The swap button on the Start row, and its gap to the dropdown.
const SWAP_W: f32 = 24.0;
const SWAP_GAP: f32 = 4.0;
const SWAP_ICON: f32 = 14.0;
/// Lucide `arrow-left-right`.
const SWAP_PATHS: [&str; 4] = ["M8 3 4 7l4 4", "M4 7h16", "m16 21 4-4-4-4", "M20 17H4"];

/// The end shapes, in popover order.
pub(crate) const END_CAPS: [StrokeEndCap; 8] = [
    StrokeEndCap::None,
    StrokeEndCap::Round,
    StrokeEndCap::Square,
    StrokeEndCap::LineArrow,
    StrokeEndCap::TriangleArrow,
    StrokeEndCap::ReversedTriangle,
    StrokeEndCap::Circle,
    StrokeEndCap::Diamond,
];

/// Locale key and English fallback of each [`END_CAPS`] entry.
pub(crate) const END_CAP_KEYS: [(&str, &str); 8] = [
    ("stroke.endNone", "None"),
    ("stroke.endRound", "Round"),
    ("stroke.endSquare", "Square"),
    ("stroke.endLineArrow", "Line arrow"),
    ("stroke.endTriangleArrow", "Triangle arrow"),
    ("stroke.endReversedTriangle", "Reversed triangle"),
    ("stroke.endCircle", "Circle"),
    ("stroke.endDiamond", "Diamond"),
];

/// The localized name of `cap`.
pub(crate) fn end_cap_name(labels: &PropertyLabels, cap: StrokeEndCap) -> &'static str {
    let index = END_CAPS.iter().position(|c| *c == cap).unwrap_or(0);
    labels.stroke_end_names[index]
}

/// The control part of a labelled row (right of its label).
fn control(row: Rect) -> Rect {
    Rect::xywh(
        row.origin.x + LABEL_W,
        row.origin.y,
        (row.size.x - LABEL_W).max(0.0),
        row.size.y,
    )
}

/// The dropdown / input part of a row: the control minus the swap
/// button's column, so the Start, End and Miter fields line up.
pub(crate) fn field_rect(row: Rect) -> Rect {
    let control = control(row);
    Rect::xywh(
        control.origin.x,
        control.origin.y,
        (control.size.x - SWAP_W - SWAP_GAP).max(0.0),
        control.size.y,
    )
}

/// The swap button at the right end of the Start row.
pub(crate) fn swap_rect(row: Rect) -> Rect {
    Rect::xywh(
        row.origin.x + row.size.x - SWAP_W,
        row.origin.y,
        SWAP_W,
        INPUT_HEIGHT,
    )
}

/// The popover rows of the Start (`end: false`) or End dropdown.
pub(crate) fn end_menu_rows(
    labels: &PropertyLabels,
    end: bool,
) -> Vec<(PropertyPanelAction, String)> {
    END_CAPS
        .iter()
        .zip(labels.stroke_end_names)
        .map(|(cap, name)| {
            let edit = if end {
                StrokeStyleEdit::End(*cap)
            } else {
                StrokeStyleEdit::Start(*cap)
            };
            (PropertyPanelAction::SetStrokeStyle(edit), name.to_string())
        })
        .collect()
}

/// Click targets of the Start and End rows.
pub(crate) fn push_end_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    start_row: Rect,
    end_row: Rect,
) {
    out.push((
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::StrokeEnd { end: false }),
        field_rect(start_row),
    ));
    out.push((
        PropertyPanelAction::SetStrokeStyle(StrokeStyleEdit::SwapEnds),
        swap_rect(start_row),
    ));
    out.push((
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::StrokeEnd { end: true }),
        field_rect(end_row),
    ));
}

/// Paint the Start and End rows.
pub(crate) fn paint_end_rows(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    snapshot: &NodeSnapshot,
    start_row: Rect,
    end_row: Rect,
) {
    let Some(style) = snapshot.stroke_style.as_ref() else {
        return;
    };
    for (row, label, cap) in [
        (start_row, labels.stroke_start, style.start),
        (end_row, labels.stroke_end, style.end),
    ] {
        super::property_panel_stroke_style::paint_row_label(cx, theme, label, row, LABEL_W);
        crate::widgets::property_panel_compositing::paint_trigger(
            cx,
            theme,
            field_rect(row),
            end_cap_name(labels, cap),
        );
    }
    let swap = swap_rect(start_row);
    cx.backend.fill_round_rect(swap, INPUT_RADIUS, theme.muted);
    for d in SWAP_PATHS {
        draw_icon_data(
            cx.backend,
            IconPathData {
                d,
                style: crate::widgets::icon_catalog::IconRenderStyle::Stroke,
                viewbox: 24.0,
            },
            Point2D::new(
                swap.origin.x + (swap.size.x - SWAP_ICON) / 2.0,
                swap.origin.y + (swap.size.y - SWAP_ICON) / 2.0,
            ),
            SWAP_ICON,
            theme.muted_foreground,
            1.5,
        );
    }
}

/// The Start / End / Miter labels and end names of `labels` that the rows
/// would paint cut (ellipsized) in the property panel, measured through
/// `backend` in the face and size each one is painted with and fitted by
/// the same fitters paint uses. Empty when every one fits. A host test
/// runs it on the native backend's real font metrics for all 15 locales.
pub fn cut_end_labels(
    backend: &mut dyn crate::RenderBackend,
    labels: &PropertyLabels,
) -> Vec<&'static str> {
    use crate::widgets::text_metrics::{fit_chrome, fit_select_trigger_label};
    let pad = crate::widgets::property_panel_inputs::PAD_X;
    let row = Rect::xywh(
        pad,
        0.0,
        crate::widgets::property_panel::PROPERTY_PANEL_WIDTH - pad * 2.0,
        INPUT_HEIGHT,
    );
    let label_size = crate::widgets::property_panel_stroke_style::LABEL_FONT_SIZE;
    let name_size = crate::widgets::property_panel_compositing::TRIGGER_FONT_SIZE;
    let mut cut = Vec::new();
    // `paint_row_label` fits a label into `LABEL_W - 6`.
    for label in [labels.stroke_start, labels.stroke_end, labels.stroke_miter] {
        if fit_chrome(backend, label, LABEL_W - 6.0, label_size) != label {
            cut.push(label);
        }
    }
    // `paint_trigger` fits an end name into the dropdown.
    for name in labels.stroke_end_names {
        if fit_select_trigger_label(backend, name, field_rect(row), name_size) != name {
            cut.push(name);
        }
    }
    cut
}

/// The Miter field's rect in its row.
pub(crate) fn miter_input_rect(row: Rect) -> Rect {
    field_rect(row)
}

/// Paint the Miter row: label + the numeric field.
pub(crate) fn paint_miter_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    row: Rect,
) {
    let Some(style) = snapshot.stroke_style.as_ref() else {
        return;
    };
    super::property_panel_stroke_style::paint_row_label(
        cx,
        theme,
        labels.stroke_miter,
        row,
        LABEL_W,
    );
    let focus = PropertyFocus::StrokeMiterLimit;
    let value = format_panel_number(style.miter_limit);
    paint_input_with_prefix_focused_state(
        cx,
        theme,
        miter_input_rect(row),
        "",
        edit.value_for(focus, &value),
        edit.focus == Some(focus),
        edit.caret_at(focus),
        edit.select_all_at(focus),
        edit.input_at(focus),
        edit.now_ms,
    );
}
