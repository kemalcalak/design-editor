//! The Scale section (#394): shown at the top of the properties card while
//! the Scale tool (K) is active. A factor field, the target width / height
//! of the selection's bounds and the 3 × 3 anchor grid. A committed field
//! scales the selection and everything in it about the anchor, one undo
//! step, through the same request the handle drag and MCP `scale_nodes`
//! build ([`crate::scale_plan`]).

use de_editor_core::selection_scale::{factor_for_size, scale_with_history};
use de_editor_core::{EditorState, PropertyFocus, ScaleAnchor, Tool, VariableMode};

use crate::layout_scene::LayoutScene;
use crate::scale_plan::ScalePlan;
use crate::theme::Theme;
use crate::util::format_panel_number;
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, paint_section_divider, paint_section_label,
    INPUT_HEIGHT, PAD_X, SECTION_GAP, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

/// What the section shows: the selection's bounds and the anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleSummary {
    pub width: f64,
    pub height: f64,
    pub anchor: ScaleAnchor,
}

impl ScaleSummary {
    /// The summary while the Scale tool is active and something is
    /// selected; the authored union until the scene refines it.
    pub fn for_state(state: &EditorState) -> Option<Self> {
        if state.tool != Tool::Scale {
            return None;
        }
        let ids = de_editor_core::selection_scale::scalable_selection(state);
        let bounds =
            de_editor_core::geometry::union_aggregate_bounds(state.active_children(), &ids)?;
        Some(Self {
            width: bounds.w,
            height: bounds.h,
            anchor: state.editor_ui.scale_anchor,
        })
    }

    /// Replace the size with the layout-resolved one.
    pub fn refine(&mut self, state: &EditorState, scene: &LayoutScene) {
        if let Some(plan) = ScalePlan::for_selection(state, scene) {
            self.width = plan.bounds.w;
            self.height = plan.bounds.h;
        }
    }
}

/// One anchor cell of the grid.
const CELL: f32 = 12.0;
const CELL_GAP: f32 = 4.0;
const GRID: f32 = CELL * 3.0 + CELL_GAP * 2.0;
const ROW_GAP: f32 = 6.0;

/// Section height: title, W / H row, factor row (as tall as the grid).
pub fn scale_section_height() -> f32 {
    SECTION_HEADER_HEIGHT + INPUT_HEIGHT + ROW_GAP + GRID.max(INPUT_HEIGHT) + 12.0 + SECTION_GAP
}

fn half_w(width: f32) -> f32 {
    (width - PAD_X * 2.0 - 8.0) / 2.0
}

/// The three inputs at section top `y`.
fn input_rects(x: f32, y: f32, width: f32) -> [(PropertyFocus, Rect); 3] {
    let half = half_w(width);
    let row1 = y + SECTION_HEADER_HEIGHT;
    let row2 = row1 + INPUT_HEIGHT + ROW_GAP;
    [
        (
            PropertyFocus::ScaleWidth,
            Rect::xywh(x + PAD_X, row1, half, INPUT_HEIGHT),
        ),
        (
            PropertyFocus::ScaleHeight,
            Rect::xywh(x + PAD_X + half + 8.0, row1, half, INPUT_HEIGHT),
        ),
        (
            PropertyFocus::ScaleFactor,
            Rect::xywh(x + PAD_X, row2, half, INPUT_HEIGHT),
        ),
    ]
}

/// The nine anchor cells at section top `y`, right column of row two.
fn anchor_rects(x: f32, y: f32, width: f32) -> [(ScaleAnchor, Rect); 9] {
    let half = half_w(width);
    let left = x + PAD_X + half + 8.0;
    let top = y + SECTION_HEADER_HEIGHT + INPUT_HEIGHT + ROW_GAP;
    ScaleAnchor::ALL.map(|anchor| {
        let (fx, fy) = anchor.fractions();
        let col = (fx * 2.0) as f32;
        let row = (fy * 2.0) as f32;
        (
            anchor,
            Rect::xywh(
                left + col * (CELL + CELL_GAP),
                top + row * (CELL + CELL_GAP),
                CELL,
                CELL,
            ),
        )
    })
}

pub(crate) fn push_scale_input_rects(
    out: &mut Vec<(PropertyFocus, Rect)>,
    x: f32,
    y: f32,
    width: f32,
) {
    out.extend(input_rects(x, y, width));
}

pub(crate) fn push_scale_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x: f32,
    y: f32,
    width: f32,
) {
    for (anchor, rect) in anchor_rects(x, y, width) {
        out.push((PropertyPanelAction::SetScaleAnchor(anchor), rect));
    }
}

/// Paint the section from its top `y`; returns where it ends.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_scale_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    edit: &EditContext<'_>,
    summary: &ScaleSummary,
    title: &str,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    paint_section_label(cx, theme, title, x, y, width);
    let values = [
        format_panel_number(summary.width.round() as f32),
        format_panel_number(summary.height.round() as f32),
        "1".to_owned(),
    ];
    for ((focus, rect), (prefix, value)) in input_rects(x, y, width)
        .into_iter()
        .zip(["W", "H", "×"].into_iter().zip(values.iter()))
    {
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
    for (anchor, rect) in anchor_rects(x, y, width) {
        let on = anchor == summary.anchor;
        let colour = if on { theme.primary } else { theme.muted };
        let dot = if on { CELL } else { CELL - 6.0 };
        let inset = (CELL - dot) / 2.0;
        cx.backend.fill_round_rect(
            Rect {
                origin: Point2D::new(rect.origin.x + inset, rect.origin.y + inset),
                size: Point2D::new(dot, dot),
            },
            dot / 2.0,
            colour,
        );
    }
    let end = y + scale_section_height() - SECTION_GAP;
    paint_section_divider(cx, theme, x, end, width);
    end + SECTION_GAP
}

/// The draft a Scale field starts from.
pub fn scale_focus_initial(focus: PropertyFocus, summary: Option<&ScaleSummary>) -> Option<String> {
    let summary = summary?;
    match focus {
        PropertyFocus::ScaleFactor => Some("1".to_owned()),
        PropertyFocus::ScaleWidth => Some(format_panel_number(summary.width.round() as f32)),
        PropertyFocus::ScaleHeight => Some(format_panel_number(summary.height.round() as f32)),
        _ => None,
    }
}

/// Commit a focused Scale field: `None` when the focus is not one of
/// them (the caller runs the ordinary commit), else whether the
/// selection was scaled. An unparsable, out-of-range or unchanged value
/// is dropped like any other bad numeric draft.
pub fn commit_scale_focus(state: &mut EditorState, scene: &LayoutScene) -> Option<bool> {
    let focus = state.ui.property_focus.filter(|f| f.is_scale())?;
    state.ui.property_focus = None;
    state.ui.property_draft_select_all = false;
    let draft = state.ui.property_input.text().trim().to_owned();
    state.ui.property_input.set_text("");
    state.ui.property_input_draft.clear();
    state.ui.property_caret_pos = 0;
    let Ok(value) = draft.trim_end_matches(['x', '×', '%']).parse::<f64>() else {
        return Some(false);
    };
    let Some(plan) = ScalePlan::for_selection(state, scene) else {
        return Some(false);
    };
    let factor = match focus {
        PropertyFocus::ScaleWidth => factor_for_size(plan.bounds.w, value),
        PropertyFocus::ScaleHeight => factor_for_size(plan.bounds.h, value),
        _ if draft.ends_with('%') => Ok(value / 100.0),
        _ => Ok(value),
    };
    let Ok(factor) = factor else {
        return Some(false);
    };
    let request = plan.request(
        factor,
        state.editor_ui.scale_anchor,
        VariableMode::KeepBound,
    );
    Some(scale_with_history(state, &request).is_ok())
}

#[cfg(test)]
#[path = "property_panel_scale_tests.rs"]
mod tests;
