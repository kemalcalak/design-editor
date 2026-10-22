//! Shared chrome for the Agents tab's detail pane (#26): the title block
//! with its switch, labelled inputs, segmented controls, the model
//! checklist and the footer buttons. The API, CLI and ACP panes compose
//! these so the three read as one form.

use crate::theme::Theme;
use crate::widgets::agent_settings_rows::{fit_text, measure_settings_text, SETTINGS_FONT_FAMILY};
use crate::widgets::agent_settings_switch::{SETTINGS_SWITCH_H, SETTINGS_SWITCH_W};
use crate::widgets::button::tokens_from_theme;
use crate::widgets::home_surface::MONO;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

pub(crate) const TITLE_FONT: f32 = 17.0;
pub(crate) const SUBTITLE_FONT: f32 = 12.5;
pub(crate) const LABEL_FONT: f32 = 12.5;
pub(crate) const VALUE_FONT: f32 = 13.0;
/// Title block: title over one muted line.
pub(crate) const HEADER_H: f32 = 60.0;
/// A field label's band above its input.
pub(crate) const LABEL_H: f32 = 24.0;
pub(crate) const INPUT_H: f32 = 36.0;
pub(crate) const INPUT_PAD_X: f32 = 12.0;
/// Vertical space between two fields.
pub(crate) const FIELD_GAP: f32 = 16.0;
pub(crate) const BUTTON_H: f32 = 34.0;
pub(crate) const CHECK_ROW_H: f32 = 30.0;
pub(crate) const CHECK_BOX_PAD: f32 = 4.0;
/// Rows a checklist shows before it scrolls.
pub(crate) const CHECK_MAX_ROWS: usize = 8;
const CHECK_SIZE: f32 = 16.0;
const RADIUS: f32 = 8.0;

/// One run of text at a baseline.
#[allow(clippy::too_many_arguments)]
pub(crate) fn text(
    cx: &mut PaintCx<'_>,
    value: &str,
    size: f32,
    color: Color,
    weight: u16,
    family: &str,
    x: f32,
    baseline: f32,
) {
    let layout =
        TextLayout::single_run(value, family, size, color.to_jian(), Point2D::new(0.0, 0.0))
            .with_font_weight(weight);
    cx.backend.draw_text(&layout, Point2D::new(x, baseline));
}

/// Baseline that centres one line of `size` text in `rect`.
pub(crate) fn centred_baseline(rect: Rect, size: f32) -> f32 {
    jian_widgets::centered_text_baseline_y(rect, size)
}

/// The enable switch at the right of the title block.
pub(crate) fn header_switch_rect(detail: Rect) -> Rect {
    Rect::xywh(
        detail.origin.x + detail.size.x - SETTINGS_SWITCH_W,
        detail.origin.y + 6.0,
        SETTINGS_SWITCH_W,
        SETTINGS_SWITCH_H,
    )
}

/// Title and subtitle; `switch` paints the enable switch at the right.
pub(crate) fn paint_header(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    detail: Rect,
    title: &str,
    subtitle: &str,
    switch: Option<bool>,
) {
    let reserved = if switch.is_some() {
        SETTINGS_SWITCH_W + 16.0
    } else {
        0.0
    };
    let budget = (detail.size.x - reserved).max(0.0);
    let title = fit_text(cx, title, budget, TITLE_FONT);
    text(
        cx,
        &title,
        TITLE_FONT,
        theme.foreground,
        600,
        SETTINGS_FONT_FAMILY,
        detail.origin.x,
        detail.origin.y + 20.0,
    );
    let subtitle = fit_text(cx, subtitle, budget, SUBTITLE_FONT);
    text(
        cx,
        &subtitle,
        SUBTITLE_FONT,
        theme.muted_foreground,
        400,
        SETTINGS_FONT_FAMILY,
        detail.origin.x,
        detail.origin.y + 42.0,
    );
    if let Some(on) = switch {
        crate::widgets::agent_settings_switch::paint_settings_switch(
            cx,
            theme,
            header_switch_rect(detail),
            on,
        );
    }
}

/// A field's label, painted in the band above `input`.
pub(crate) fn paint_label(cx: &mut PaintCx<'_>, theme: &Theme, label: &str, input: Rect) {
    text(
        cx,
        label,
        LABEL_FONT,
        theme.muted_foreground,
        500,
        SETTINGS_FONT_FAMILY,
        input.origin.x,
        input.origin.y - 8.0,
    );
}

/// An input box; focused boxes take the primary ring.
pub(crate) fn paint_input_box(cx: &mut PaintCx<'_>, theme: &Theme, input: Rect, focused: bool) {
    cx.backend.fill_round_rect(input, RADIUS, theme.background);
    cx.backend.stroke_round_rect(
        input,
        RADIUS,
        if focused { theme.primary } else { theme.border },
        1.0,
    );
}

/// The static value of an unfocused input, ellipsized to the box.
/// `trailing` reserves room for an icon at the right edge.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_input_value(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    input: Rect,
    value: &str,
    placeholder: &str,
    mono: bool,
    trailing: f32,
) {
    let (shown, color) = if value.is_empty() {
        (placeholder, theme.muted_subtle)
    } else {
        (value, theme.foreground)
    };
    let budget = (input.size.x - INPUT_PAD_X * 2.0 - trailing).max(0.0);
    let fitted = fit_text(cx, shown, budget, VALUE_FONT);
    text(
        cx,
        &fitted,
        VALUE_FONT,
        color,
        400,
        if mono { MONO } else { SETTINGS_FONT_FAMILY },
        input.origin.x + INPUT_PAD_X,
        centred_baseline(input, VALUE_FONT),
    );
}

/// Segmented control over `labels`, `active` highlighted.
pub(crate) fn paint_segmented(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    labels: &[&str],
    active: Option<usize>,
) {
    jian_widgets::components::toggle_group::ToggleGroup {
        options: labels,
        icons: None,
        active: active.unwrap_or(usize::MAX),
        hover: None,
        font_size: 12.5,
    }
    .paint(cx.backend, rect, &segment_tokens(theme));
}

/// The active segment reads as a raised chip on the track, not as a
/// primary-filled button: a choice between formats, not an action.
fn segment_tokens(theme: &Theme) -> jian_widgets::Tokens {
    let mut tokens = tokens_from_theme(theme);
    tokens.primary = theme.muted;
    tokens.primary_foreground = theme.foreground;
    tokens
}

/// The segment of a `count`-segment control under `point`.
pub(crate) fn segment_at(rect: Rect, count: usize, point: Point2D) -> Option<usize> {
    jian_widgets::components::toggle_group::ToggleGroup::segment_at(rect, count, point)
}

/// Width a segmented control needs for `labels`.
pub(crate) fn segmented_width(labels: &[&str]) -> f32 {
    labels
        .iter()
        .map(|label| label.chars().count() as f32 * 7.4 + 28.0)
        .sum::<f32>()
        + 8.0
}

/// Height of a checklist box with `rows` rows (capped, then it scrolls).
pub(crate) fn checklist_height(rows: usize) -> f32 {
    rows.clamp(1, CHECK_MAX_ROWS) as f32 * CHECK_ROW_H + CHECK_BOX_PAD * 2.0
}

/// How far a checklist with `rows` rows can scroll.
pub(crate) fn checklist_scroll_max(rows: usize) -> f32 {
    rows.saturating_sub(CHECK_MAX_ROWS) as f32 * CHECK_ROW_H
}

/// Row `index` of a checklist, before clipping to the box.
pub(crate) fn checklist_row_rect(boxed: Rect, index: usize, scroll: f32) -> Rect {
    Rect::xywh(
        boxed.origin.x + CHECK_BOX_PAD,
        boxed.origin.y + CHECK_BOX_PAD + index as f32 * CHECK_ROW_H - scroll,
        boxed.size.x - CHECK_BOX_PAD * 2.0,
        CHECK_ROW_H,
    )
}

/// The checklist row under `point`, scroll included.
pub(crate) fn checklist_row_at(
    boxed: Rect,
    rows: usize,
    scroll: f32,
    point: Point2D,
) -> Option<usize> {
    let inner = Rect::xywh(
        boxed.origin.x,
        boxed.origin.y + CHECK_BOX_PAD,
        boxed.size.x,
        boxed.size.y - CHECK_BOX_PAD * 2.0,
    );
    if !inner.contains(point) {
        return None;
    }
    (0..rows).find(|row| checklist_row_rect(boxed, *row, scroll).contains(point))
}

/// One checklist row's state.
pub(crate) struct CheckRow<'a> {
    pub(crate) label: &'a str,
    pub(crate) checked: bool,
}

/// A bordered list of mono model ids with checkboxes.
pub(crate) fn paint_checklist(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    boxed: Rect,
    rows: &[CheckRow<'_>],
    scroll: f32,
    hover: Option<usize>,
) {
    cx.backend.fill_round_rect(boxed, RADIUS, theme.background);
    cx.backend
        .stroke_round_rect(boxed, RADIUS, theme.border, 1.0);
    cx.backend.save();
    cx.backend.clip_rect(Rect::xywh(
        boxed.origin.x,
        boxed.origin.y + CHECK_BOX_PAD,
        boxed.size.x,
        boxed.size.y - CHECK_BOX_PAD * 2.0,
    ));
    for (index, row) in rows.iter().enumerate() {
        let rect = checklist_row_rect(boxed, index, scroll);
        if rect.origin.y + rect.size.y < boxed.origin.y
            || rect.origin.y > boxed.origin.y + boxed.size.y
        {
            continue;
        }
        if hover == Some(index) {
            cx.backend.fill_round_rect(rect, 6.0, theme.muted);
        }
        let check = Rect::xywh(
            rect.origin.x + 8.0,
            rect.origin.y + (rect.size.y - CHECK_SIZE) / 2.0,
            CHECK_SIZE,
            CHECK_SIZE,
        );
        if row.checked {
            cx.backend.fill_round_rect(check, 4.0, theme.primary);
            draw_icon(
                cx.backend,
                Icon::Check,
                Point2D::new(check.origin.x + 2.0, check.origin.y + 2.0),
                CHECK_SIZE - 4.0,
                theme.primary_foreground,
                2.2,
            );
        } else {
            cx.backend.stroke_round_rect(check, 4.0, theme.border, 1.2);
        }
        let x = check.origin.x + CHECK_SIZE + 12.0;
        let budget = (rect.origin.x + rect.size.x - 8.0 - x).max(0.0);
        let label = fit_text(cx, row.label, budget, VALUE_FONT);
        text(
            cx,
            &label,
            VALUE_FONT,
            if row.checked {
                theme.foreground
            } else {
                theme.muted_foreground
            },
            400,
            MONO,
            x,
            centred_baseline(rect, VALUE_FONT),
        );
    }
    cx.backend.restore();
}

/// How a footer button reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonKind {
    Primary,
    Outline,
}

/// Estimated button width for hit geometry, which has no painter.
pub(crate) fn button_width_estimate(label: &str, icon: bool) -> f32 {
    label.chars().count() as f32 * 7.6 + 32.0 + if icon { 22.0 } else { 0.0 }
}

/// A rounded footer button.
pub(crate) fn paint_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    label: &str,
    icon: Option<Icon>,
    kind: ButtonKind,
) {
    let ink = match kind {
        ButtonKind::Primary => {
            cx.backend.fill_round_rect(rect, RADIUS, theme.primary);
            theme.primary_foreground
        }
        ButtonKind::Outline => {
            cx.backend.fill_round_rect(rect, RADIUS, theme.card);
            cx.backend
                .stroke_round_rect(rect, RADIUS, theme.border, 1.0);
            theme.foreground
        }
    };
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    // Widths come from an estimate (hit-test has no painter), so a long
    // translation is fitted rather than left to spill past the border.
    let label = fit_text(
        cx,
        label,
        (rect.size.x - 24.0 - icon_w).max(0.0),
        VALUE_FONT,
    );
    let label = label.as_str();
    let label_w = measure_settings_text(cx, label, VALUE_FONT);
    let mut x = rect.origin.x + (rect.size.x - label_w - icon_w) / 2.0;
    if let Some(icon) = icon {
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(x, rect.origin.y + (rect.size.y - 15.0) / 2.0),
            15.0,
            ink,
            1.7,
        );
        x += icon_w;
    }
    text(
        cx,
        label,
        VALUE_FONT,
        ink,
        500,
        SETTINGS_FONT_FAMILY,
        x,
        centred_baseline(rect, VALUE_FONT),
    );
}

/// A square icon-only button (remove).
pub(crate) fn paint_icon_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    icon: Icon,
    color: Color,
) {
    cx.backend.fill_round_rect(rect, RADIUS, theme.card);
    cx.backend
        .stroke_round_rect(rect, RADIUS, theme.border, 1.0);
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(
            rect.origin.x + (rect.size.x - 15.0) / 2.0,
            rect.origin.y + (rect.size.y - 15.0) / 2.0,
        ),
        15.0,
        color,
        1.7,
    );
}

/// A status line: a leading glyph and one fitted line of text.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_status(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    x: f32,
    row: Rect,
    icon: Icon,
    color: Color,
    label: &str,
    max_w: f32,
) {
    let glyph = 15.0;
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(x, row.origin.y + (row.size.y - glyph) / 2.0),
        glyph,
        color,
        1.7,
    );
    let label = fit_text(cx, label, (max_w - glyph - 8.0).max(0.0), LABEL_FONT);
    text(
        cx,
        &label,
        LABEL_FONT,
        theme.muted_foreground,
        400,
        SETTINGS_FONT_FAMILY,
        x + glyph + 8.0,
        centred_baseline(row, LABEL_FONT),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checklist_caps_its_height_and_scrolls_the_rest() {
        assert_eq!(checklist_height(3), 3.0 * CHECK_ROW_H + CHECK_BOX_PAD * 2.0);
        assert_eq!(checklist_height(20), checklist_height(CHECK_MAX_ROWS));
        assert_eq!(checklist_scroll_max(3), 0.0);
        assert_eq!(checklist_scroll_max(10), 2.0 * CHECK_ROW_H);
    }

    #[test]
    fn checklist_rows_map_back_through_the_scroll() {
        let boxed = Rect::xywh(0.0, 0.0, 300.0, checklist_height(10));
        let row = checklist_row_rect(boxed, 0, 0.0);
        let mid = Point2D::new(40.0, row.origin.y + 10.0);
        assert_eq!(checklist_row_at(boxed, 10, 0.0, mid), Some(0));
        assert_eq!(checklist_row_at(boxed, 10, CHECK_ROW_H, mid), Some(1));
        assert_eq!(
            checklist_row_at(boxed, 10, 0.0, Point2D::new(40.0, -5.0)),
            None
        );
    }
}
