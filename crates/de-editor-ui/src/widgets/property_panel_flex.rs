//! Auto layout section paint and geometry helpers (#83): the direction
//! segments, then one `Gap | Pad` row. Alignment and the space modes
//! live in a popover behind the header's sliders button.
//!
//! Every helper takes `y` = the section body top (just below the
//! header), the same origin the action and input walkers pass, so the
//! header button sits at `y - SECTION_HEADER_HEIGHT`.

use crate::theme::Theme;
use crate::util::format_panel_number;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::{
    LayoutAlignValue, LayoutJustifyValue, NodeSnapshot, PropertyPanelAction,
};
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, paint_section_divider, paint_section_label,
    INPUT_HEIGHT, INPUT_RADIUS, PAD_X, SECTION_GAP, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_mode_popover as mode_popover;
use crate::widgets::property_panel_sections::{EditContext, PropertyLabels};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::{FlexLayout, PropertyFocus};

const DIR_BUTTON_H: f32 = 28.0;
/// Gap between the direction segments and the `Gap | Pad` row.
const ROW_GAP: f32 = 8.0;
/// Space below the last row before the divider.
const TRAILING_GAP: f32 = 12.0;
const COLUMN_GAP: f32 = 8.0;

const BUTTON_SIZE: f32 = 22.0;
const POPOVER_W: f32 = 232.0;
const POPOVER_PAD: f32 = 8.0;
const GRID_CELL_W: f32 = 24.0;
const GRID_CELL_H: f32 = 20.0;
const GRID_GAP: f32 = 2.0;
const GRID_PAD: f32 = 6.0;
const GRID_BOX_W: f32 = GRID_CELL_W * 3.0 + GRID_GAP * 2.0 + GRID_PAD * 2.0;
const GRID_BOX_H: f32 = GRID_CELL_H * 3.0 + GRID_GAP * 2.0 + GRID_PAD * 2.0;
const JUSTIFY_ROW_H: f32 = GRID_BOX_H / 3.0;
const RADIO_SIZE: f32 = 13.0;
const RADIO_GUTTER: f32 = 6.0 + RADIO_SIZE;

/// The direction control's segments (#30): full width, in the design's
/// order — vertical, horizontal, then free. Paint and the action walker
/// both read this, so a click lands on the segment drawn.
const DIRECTIONS: [(FlexLayout, Icon); 3] = [
    (FlexLayout::Vertical, Icon::ArrowDown),
    (FlexLayout::Horizontal, Icon::ArrowRight),
    (FlexLayout::Free, Icon::LayoutGrid),
];

/// Popover rows for the main-axis distribution, in paint order. The
/// host hover tier indexes `layout_align_popover_hover` into this.
pub const JUSTIFY_ROWS: [(LayoutJustifyValue, &str); 3] = [
    (LayoutJustifyValue::Start, "layout.packed"),
    (LayoutJustifyValue::SpaceBetween, "layout.spaceBetween"),
    (LayoutJustifyValue::SpaceAround, "layout.spaceAround"),
];

/// True for every action the open alignment popover owns (its toggle,
/// grid cells and distribution rows). Hosts apply these instead of
/// treating the press as an outside click that closes the popover.
pub fn is_layout_align_popover_action(action: &PropertyPanelAction) -> bool {
    matches!(
        action,
        PropertyPanelAction::ToggleLayoutAlignPopover
            | PropertyPanelAction::SetLayoutAlignment { .. }
            | PropertyPanelAction::SetLayoutAlign(_)
            | PropertyPanelAction::SetLayoutJustify(_)
    )
}

/// The [`JUSTIFY_ROWS`] index an action hovers, for the popover's
/// hover wash. `None` for anything that is not a distribution row.
pub fn layout_align_popover_hover_row(action: &PropertyPanelAction) -> Option<usize> {
    match action {
        PropertyPanelAction::SetLayoutJustify(value) => {
            JUSTIFY_ROWS.iter().position(|(row, _)| row == value)
        }
        _ => None,
    }
}

fn direction_control_rect(x0: f32, y: f32, width: f32) -> Rect {
    Rect::xywh(x0 + PAD_X, y, (width - PAD_X * 2.0).max(0.0), DIR_BUTTON_H)
}

/// `[gap, pad]` input rects of the row under the direction control.
fn gap_pad_rects(x0: f32, y: f32, width: f32) -> [Rect; 2] {
    let half_w = ((width - PAD_X * 2.0 - COLUMN_GAP) / 2.0).max(0.0);
    let row_y = y + DIR_BUTTON_H + ROW_GAP;
    [
        Rect::xywh(x0 + PAD_X, row_y, half_w, INPUT_HEIGHT),
        Rect::xywh(
            x0 + PAD_X + half_w + COLUMN_GAP,
            row_y,
            half_w,
            INPUT_HEIGHT,
        ),
    ]
}

/// The header's alignment button, right-aligned on the header row.
pub(crate) fn align_button_rect(x0: f32, y: f32, width: f32) -> Rect {
    let header_y = y - SECTION_HEADER_HEIGHT;
    Rect::xywh(
        x0 + width - PAD_X - BUTTON_SIZE + 4.0,
        header_y + (SECTION_HEADER_HEIGHT - BUTTON_SIZE) / 2.0,
        BUTTON_SIZE,
        BUTTON_SIZE,
    )
}

/// Popover box anchored under the alignment button. Paint, the action
/// walker and the host containment test all derive from this.
pub(crate) fn align_popover_rect_from_button(button: Rect, width: f32) -> Rect {
    let pop_w = POPOVER_W.min(width - PAD_X * 2.0);
    Rect::xywh(
        button.origin.x + button.size.x - pop_w,
        button.origin.y + button.size.y + 4.0,
        pop_w,
        GRID_BOX_H + POPOVER_PAD * 2.0,
    )
}

fn grid_box_rect(pop_box: Rect) -> Rect {
    Rect::xywh(
        pop_box.origin.x + POPOVER_PAD,
        pop_box.origin.y + POPOVER_PAD,
        GRID_BOX_W,
        GRID_BOX_H,
    )
}

fn grid_cell_rect(grid_box: Rect, row: usize, col: usize) -> Rect {
    Rect::xywh(
        grid_box.origin.x + GRID_PAD + col as f32 * (GRID_CELL_W + GRID_GAP),
        grid_box.origin.y + GRID_PAD + row as f32 * (GRID_CELL_H + GRID_GAP),
        GRID_CELL_W,
        GRID_CELL_H,
    )
}

fn justify_row_rect(pop_box: Rect, i: usize) -> Rect {
    let x = pop_box.origin.x + POPOVER_PAD + GRID_BOX_W + COLUMN_GAP;
    Rect::xywh(
        x,
        pop_box.origin.y + POPOVER_PAD + i as f32 * JUSTIFY_ROW_H,
        (pop_box.origin.x + pop_box.size.x - POPOVER_PAD - x).max(0.0),
        JUSTIFY_ROW_H,
    )
}

fn is_space(justify: LayoutJustifyValue) -> bool {
    matches!(
        justify,
        LayoutJustifyValue::SpaceBetween | LayoutJustifyValue::SpaceAround
    )
}

/// Total height of the section, header and trailing gap included.
pub fn flex_section_height(active: FlexLayout) -> f32 {
    let body = if active == FlexLayout::Free {
        DIR_BUTTON_H + TRAILING_GAP
    } else {
        DIR_BUTTON_H + ROW_GAP + INPUT_HEIGHT + TRAILING_GAP
    };
    SECTION_HEADER_HEIGHT + body + SECTION_GAP
}

#[allow(clippy::too_many_arguments)]
pub fn paint_flex_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    labels: &PropertyLabels,
    locale: de_editor_core::Locale,
    divider: bool,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let section_y = y;
    let body_y = paint_section_label(cx, theme, labels.auto_layout, x, y, width);
    paint_direction_buttons(cx, theme, snapshot.flex_layout, x, body_y, width);
    if snapshot.flex_layout != FlexLayout::Free {
        let button = align_button_rect(x, body_y, width);
        draw_icon(
            cx.backend,
            Icon::SlidersHorizontal,
            Point2D::new(button.origin.x + 4.0, button.origin.y + 4.0),
            14.0,
            theme.muted_foreground,
            1.4,
        );
        paint_gap_pad_row(cx, theme, snapshot, edit, locale, x, body_y, width);
    }
    let bottom = section_y + flex_section_height(snapshot.flex_layout) - SECTION_GAP;
    if divider {
        paint_section_divider(cx, theme, x, bottom, width);
    }
    bottom + SECTION_GAP
}

#[allow(clippy::too_many_arguments)]
fn paint_gap_pad_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) {
    let [gap_rect, pad_rect] = gap_pad_rects(x, y, width);
    let gap_prefix = de_i18n::translate(locale, "layout.gap");
    if is_space(snapshot.layout_justify) {
        // Space between / around distribute the gap themselves, so the
        // explicit value is ignored: paint "Auto" and emit no focus rect
        // (see `push_flex_input_rects`).
        paint_input_with_prefix_focused_state(
            cx,
            theme,
            gap_rect,
            gap_prefix,
            de_i18n::translate(locale, "layout.auto"),
            false,
            None,
            false,
            None,
            edit.now_ms,
        );
    } else {
        paint_focusable_input(
            cx,
            theme,
            edit,
            PropertyFocus::LayoutGap,
            gap_prefix,
            &format_panel_number(snapshot.layout_gap),
            gap_rect,
        );
    }
    paint_focusable_input(
        cx,
        theme,
        edit,
        PropertyFocus::Padding,
        de_i18n::translate(locale, "padding.short"),
        &snapshot.layout_padding.shorthand(),
        pad_rect,
    );
}

fn paint_focusable_input(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    edit: &EditContext<'_>,
    focus: PropertyFocus,
    prefix: &str,
    value: &str,
    rect: Rect,
) {
    paint_input_with_prefix_focused_state(
        cx,
        theme,
        rect,
        prefix,
        edit.value_for(focus, value),
        edit.focus == Some(focus),
        edit.caret_at(focus),
        edit.select_all_at(focus),
        edit.input_at(focus),
        edit.now_ms,
    );
}

pub fn push_flex_input_rects(
    rects: &mut Vec<(PropertyFocus, Rect)>,
    x0: f32,
    y: f32,
    width: f32,
    active: FlexLayout,
    justify: LayoutJustifyValue,
) {
    if active == FlexLayout::Free {
        return;
    }
    let [gap_rect, pad_rect] = gap_pad_rects(x0, y, width);
    if !is_space(justify) {
        rects.push((PropertyFocus::LayoutGap, gap_rect));
    }
    rects.push((PropertyFocus::Padding, pad_rect));
}

pub fn push_flex_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x0: f32,
    y: f32,
    width: f32,
    active: FlexLayout,
    justify: LayoutJustifyValue,
    align_popover_open: bool,
) {
    // Continuous even cells over the same footprint the ToggleGroup paints.
    let control = direction_control_rect(x0, y, width);
    let cell_w = control.size.x / DIRECTIONS.len() as f32;
    for (i, (mode, _)) in DIRECTIONS.iter().enumerate() {
        out.push((
            PropertyPanelAction::SetFlexLayout(*mode),
            Rect::xywh(
                control.origin.x + i as f32 * cell_w,
                y,
                cell_w,
                DIR_BUTTON_H,
            ),
        ));
    }
    if active == FlexLayout::Free {
        return;
    }
    let button = align_button_rect(x0, y, width);
    out.push((PropertyPanelAction::ToggleLayoutAlignPopover, button));
    if !align_popover_open {
        return;
    }
    // Popover targets go LAST so the open popover wins the hit-test
    // over the sections it overlaps.
    let pop_box = align_popover_rect_from_button(button, width);
    let grid_box = grid_box_rect(pop_box);
    for row in 0..3 {
        for col in 0..3 {
            let (justify_value, align_value) = cell_alignment(active, row, col);
            let action = if is_space(justify) {
                PropertyPanelAction::SetLayoutAlign(align_value)
            } else {
                PropertyPanelAction::SetLayoutAlignment {
                    justify: justify_value,
                    align: align_value,
                }
            };
            out.push((action, grid_cell_rect(grid_box, row, col)));
        }
    }
    for (i, (justify_value, _)) in JUSTIFY_ROWS.iter().enumerate() {
        out.push((
            PropertyPanelAction::SetLayoutJustify(*justify_value),
            justify_row_rect(pop_box, i),
        ));
    }
}

/// The (justify, align) pair a grid cell stands for: rows follow the
/// main axis in a vertical layout, columns in a horizontal one.
fn cell_alignment(
    active: FlexLayout,
    row: usize,
    col: usize,
) -> (LayoutJustifyValue, LayoutAlignValue) {
    if active == FlexLayout::Vertical {
        (position_to_justify(row), position_to_align(col))
    } else {
        (position_to_justify(col), position_to_align(row))
    }
}

fn paint_direction_buttons(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    active: FlexLayout,
    x: f32,
    y: f32,
    width: f32,
) {
    // Icon segmented control via jian ToggleGroup; font_size 14 yields an
    // 18px icon (font + 4).
    let labels: Vec<&str> = DIRECTIONS.iter().map(|_| "").collect();
    let icon_paths: Vec<&[&str]> = DIRECTIONS.iter().map(|(_, icon)| icon.paths()).collect();
    let active_idx = DIRECTIONS
        .iter()
        .position(|(m, _)| *m == active)
        .unwrap_or(0);
    // The design's active segment is a subtle raised fill, not the
    // solid primary ToggleGroup paints by default.
    let mut tokens = crate::widgets::button::tokens_from_theme(theme);
    tokens.primary = theme.muted;
    tokens.primary_foreground = theme.foreground;
    jian_widgets::components::toggle_group::ToggleGroup {
        options: &labels,
        icons: Some(&icon_paths),
        active: active_idx,
        hover: None,
        font_size: 14.0,
    }
    .paint(cx.backend, direction_control_rect(x, y, width), &tokens);
}

/// Paint the alignment popover (3×3 grid + Packed / Space between /
/// Space around). Painted as a top overlay so it sits above the
/// sections below the header. `hover` indexes [`JUSTIFY_ROWS`].
#[allow(clippy::too_many_arguments)]
pub fn paint_layout_align_popover(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    snapshot: &NodeSnapshot,
    hover: Option<usize>,
    x0: f32,
    y: f32,
    width: f32,
) {
    if snapshot.flex_layout == FlexLayout::Free {
        return;
    }
    let pop_box = align_popover_rect_from_button(align_button_rect(x0, y, width), width);
    cx.backend.fill_round_rect(pop_box, 8.0, theme.popover);
    cx.backend
        .stroke_round_rect(pop_box, 8.0, theme.border, 1.0);
    paint_alignment_grid(cx, theme, snapshot, grid_box_rect(pop_box));
    for (i, (justify_value, label_key)) in JUSTIFY_ROWS.iter().enumerate() {
        let rect = justify_row_rect(pop_box, i);
        if hover == Some(i) {
            cx.backend.fill_round_rect(rect, 6.0, theme.button_hover);
        }
        mode_popover::paint_radio_circle(
            cx,
            theme,
            rect.origin.x + 4.0,
            rect.origin.y + (rect.size.y - RADIO_SIZE) / 2.0,
            RADIO_SIZE,
            // Packed covers start / center / end; the grid shows which.
            if is_space(*justify_value) {
                snapshot.layout_justify == *justify_value
            } else {
                !is_space(snapshot.layout_justify)
            },
        );
        let label = TextLayout::single_run(
            de_i18n::translate(locale, label_key),
            crate::widgets::text_metrics::CHROME_FONT_FAMILY,
            11.0,
            (theme.foreground).to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &label,
            Point2D::new(
                rect.origin.x + 4.0 + RADIO_GUTTER,
                rect.origin.y + rect.size.y / 2.0 + 4.0,
            ),
        );
    }
}

fn paint_alignment_grid(cx: &mut PaintCx<'_>, theme: &Theme, snapshot: &NodeSnapshot, bg: Rect) {
    let space = is_space(snapshot.layout_justify);
    cx.backend
        .fill_round_rect(bg, INPUT_RADIUS - 2.0, theme.muted);
    for row in 0..3 {
        for col in 0..3 {
            let (justify, align) = cell_alignment(snapshot.flex_layout, row, col);
            let active = if space {
                snapshot.layout_align == align
            } else {
                snapshot.layout_justify == justify && snapshot.layout_align == align
            };
            let cell = grid_cell_rect(bg, row, col);
            let dot_size = if active { 10.0 } else { 4.0 };
            let dot = Rect::xywh(
                cell.origin.x + (cell.size.x - dot_size) / 2.0,
                cell.origin.y + (cell.size.y - dot_size) / 2.0,
                dot_size,
                dot_size,
            );
            cx.backend.fill_round_rect(
                dot,
                if active { 2.0 } else { 4.0 },
                if active {
                    theme.primary
                } else {
                    theme.muted_foreground
                },
            );
        }
    }
}

fn position_to_justify(index: usize) -> LayoutJustifyValue {
    match index {
        0 => LayoutJustifyValue::Start,
        1 => LayoutJustifyValue::Center,
        _ => LayoutJustifyValue::End,
    }
}

fn position_to_align(index: usize) -> LayoutAlignValue {
    match index {
        0 => LayoutAlignValue::Start,
        1 => LayoutAlignValue::Center,
        _ => LayoutAlignValue::End,
    }
}
