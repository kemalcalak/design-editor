//! Wrap and grid rows of the Auto layout section (#379), under the
//! `Gap | Pad` row:
//!
//! - horizontal: `[x] Wrap | Row gap`;
//! - grid: `Columns` (full width), then `Rows | Row gap`.
//!
//! The track fields take the CSS-like list the schema stores (`1fr 1fr
//! 120`, `3`, `repeat(2, 1fr) auto`, `minmax(120, 1fr)`); an empty Rows
//! field means auto rows. Every helper takes `y` = the top of these rows,
//! the same origin paint, the input walker and the action walker use.

use crate::theme::Theme;
use crate::util::format_panel_number;
use crate::widgets::property_panel::{NodeSnapshot, PropertyPanelAction};
use crate::widgets::property_panel_flex::{COLUMN_GAP, ROW_GAP};
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, INPUT_HEIGHT, PAD_X,
};
use crate::widgets::property_panel_sections::{paint_check_row, EditContext};
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::{FlexLayout, PropertyFocus};

/// Rows these controls add below `Gap | Pad` for `active`.
fn extra_rows(active: FlexLayout) -> usize {
    match active {
        FlexLayout::Horizontal => 1,
        FlexLayout::Grid => 2,
        FlexLayout::Vertical | FlexLayout::Free => 0,
    }
}

/// Height the extra rows add, their leading gaps included.
pub(crate) fn extra_height(active: FlexLayout) -> f32 {
    extra_rows(active) as f32 * (ROW_GAP + INPUT_HEIGHT)
}

fn row_y(y: f32, row: usize) -> f32 {
    y + ROW_GAP + row as f32 * (ROW_GAP + INPUT_HEIGHT)
}

fn halves(x0: f32, y: f32, width: f32) -> [Rect; 2] {
    let half_w = ((width - PAD_X * 2.0 - COLUMN_GAP) / 2.0).max(0.0);
    [
        Rect::xywh(x0 + PAD_X, y, half_w, INPUT_HEIGHT),
        Rect::xywh(x0 + PAD_X + half_w + COLUMN_GAP, y, half_w, INPUT_HEIGHT),
    ]
}

fn full(x0: f32, y: f32, width: f32) -> Rect {
    Rect::xywh(x0 + PAD_X, y, (width - PAD_X * 2.0).max(0.0), INPUT_HEIGHT)
}

/// `(focus, rect)` of every input in the extra rows.
fn inputs(x0: f32, y: f32, width: f32, active: FlexLayout) -> Vec<(PropertyFocus, Rect)> {
    match active {
        FlexLayout::Horizontal => {
            let [_, gap] = halves(x0, row_y(y, 0), width);
            vec![(PropertyFocus::LayoutRowGap, gap)]
        }
        FlexLayout::Grid => {
            let [rows, gap] = halves(x0, row_y(y, 1), width);
            vec![
                (PropertyFocus::GridColumns, full(x0, row_y(y, 0), width)),
                (PropertyFocus::GridRows, rows),
                (PropertyFocus::LayoutRowGap, gap),
            ]
        }
        FlexLayout::Vertical | FlexLayout::Free => Vec::new(),
    }
}

/// The Wrap check row's rect (horizontal only).
fn wrap_rect(x0: f32, y: f32, width: f32) -> Rect {
    halves(x0, row_y(y, 0), width)[0]
}

pub(crate) fn push_input_rects(
    rects: &mut Vec<(PropertyFocus, Rect)>,
    x0: f32,
    y: f32,
    width: f32,
    active: FlexLayout,
) {
    rects.extend(inputs(x0, y, width, active));
}

pub(crate) fn push_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x0: f32,
    y: f32,
    width: f32,
    active: FlexLayout,
) {
    if active == FlexLayout::Horizontal {
        out.push((
            PropertyPanelAction::ToggleLayoutWrap,
            wrap_rect(x0, y, width),
        ));
    }
}

fn label(focus: PropertyFocus, locale: de_editor_core::Locale) -> &'static str {
    let key = match focus {
        PropertyFocus::GridColumns => "layout.columns",
        PropertyFocus::GridRows => "layout.rows",
        _ => "layout.rowGap",
    };
    de_i18n::translate(locale, key)
}

fn value(focus: PropertyFocus, snapshot: &NodeSnapshot, locale: de_editor_core::Locale) -> String {
    let grid = &snapshot.layout_grid;
    match focus {
        PropertyFocus::GridColumns => grid.columns.clone(),
        PropertyFocus::GridRows if grid.rows.is_empty() => {
            de_i18n::translate(locale, "layout.auto").to_string()
        }
        PropertyFocus::GridRows => grid.rows.clone(),
        _ => format_panel_number(grid.row_gap),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_extra_rows(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) {
    let active = snapshot.flex_layout;
    if active == FlexLayout::Horizontal {
        let rect = wrap_rect(x, y, width);
        paint_check_row(
            cx,
            theme,
            rect.origin.x,
            rect.origin.y,
            rect.size.x,
            de_i18n::translate(locale, "layout.wrap"),
            snapshot.layout_grid.wrap,
            INPUT_HEIGHT,
        );
    }
    for (focus, rect) in inputs(x, y, width, active) {
        let shown = value(focus, snapshot, locale);
        paint_input_with_prefix_focused_state(
            cx,
            theme,
            rect,
            label(focus, locale),
            edit.value_for(focus, &shown),
            edit.focus == Some(focus),
            edit.caret_at(focus),
            edit.select_all_at(focus),
            edit.input_at(focus),
            edit.now_ms,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_per_mode() {
        assert_eq!(extra_height(FlexLayout::Vertical), 0.0);
        assert_eq!(extra_height(FlexLayout::Free), 0.0);
        assert_eq!(extra_height(FlexLayout::Horizontal), ROW_GAP + INPUT_HEIGHT);
        assert_eq!(
            extra_height(FlexLayout::Grid),
            2.0 * (ROW_GAP + INPUT_HEIGHT)
        );
    }

    #[test]
    fn grid_inputs_sit_inside_the_extra_rows() {
        let y = 100.0;
        let rects = inputs(0.0, y, 272.0, FlexLayout::Grid);
        let focuses: Vec<_> = rects.iter().map(|(f, _)| *f).collect();
        assert_eq!(
            focuses,
            [
                PropertyFocus::GridColumns,
                PropertyFocus::GridRows,
                PropertyFocus::LayoutRowGap
            ]
        );
        for (_, r) in &rects {
            assert!(r.origin.y >= y && r.origin.y + r.size.y <= y + extra_height(FlexLayout::Grid));
        }
        let mut actions = Vec::new();
        push_action_rects(&mut actions, 0.0, y, 272.0, FlexLayout::Grid);
        assert!(actions.is_empty());
        push_action_rects(&mut actions, 0.0, y, 272.0, FlexLayout::Horizontal);
        assert_eq!(actions[0].0, PropertyPanelAction::ToggleLayoutWrap);
    }
}
