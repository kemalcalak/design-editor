//! The Interactions section's overlay rows (#372), under a trigger whose
//! single action opens or swaps an overlay:
//!
//! - Frame: the overlay frame, a dropdown listing every page's frames;
//! - Position (open only): a 3×3 grid of the six edge anchors and the
//!   centre, then a Manual cell (beside the trigger);
//! - Close on outside click (open only): a checkbox;
//! - Background (open only): None | Dim, then the dim's hex under Dim;
//! - Animation: Instant | Dissolve | Move in, then the edge under Move in.
//!
//! Every row is one control and writes one option through
//! `de_editor_core::overlay_edit` (one undo step each).

use crate::theme::Theme;
use crate::widgets::property_panel::PropertyPanelAction;
use crate::widgets::property_panel_interactions::{
    paint_fitted_dropdown, translate, InteractionMenuTarget, InteractionSummary,
    InteractionTrigger, TapActionSummary,
};
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::overlay_action::{
    OverlayAction, OverlayAnimation, OverlayEdge, OverlayPosition, OverlaySpec,
};
use de_editor_core::overlay_edit::{OverlayAnimationKind, OverlayOptionEdit};

impl InteractionSummary {
    /// `trigger`'s single overlay action, if that is what it holds.
    pub fn overlay(
        &self,
        trigger: InteractionTrigger,
    ) -> Option<&de_editor_core::overlay_action::OverlayAction> {
        match self.actions(trigger) {
            [TapActionSummary::Overlay(action)] => Some(action),
            _ => None,
        }
    }

    /// The dim hex of `trigger`'s open overlay.
    pub fn overlay_background(&self, trigger: InteractionTrigger) -> Option<String> {
        match self.overlay(trigger)? {
            de_editor_core::overlay_action::OverlayAction::Open(spec) => spec.background.clone(),
            _ => None,
        }
    }

    /// An overlay frame's name (its id when it is not listed).
    pub fn overlay_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.overlay_targets
            .iter()
            .find(|target| target.id == id)
            .map_or(id, |target| target.name.as_str())
    }
}

/// One overlay row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayRow {
    Target { swap: bool },
    Position,
    Outside,
    Background,
    BackgroundHex,
    Animation,
    Direction,
}

/// The rows under a trigger's single overlay action (none for a close).
pub(crate) fn overlay_rows(action: &OverlayAction) -> Vec<OverlayRow> {
    let animation_rows = |animation: OverlayAnimation| {
        let mut rows = vec![OverlayRow::Animation];
        if matches!(animation, OverlayAnimation::MoveIn(_)) {
            rows.push(OverlayRow::Direction);
        }
        rows
    };
    match action {
        OverlayAction::Open(spec) => {
            let mut rows = vec![
                OverlayRow::Target { swap: false },
                OverlayRow::Position,
                OverlayRow::Outside,
                OverlayRow::Background,
            ];
            if spec.background.is_some() {
                rows.push(OverlayRow::BackgroundHex);
            }
            rows.extend(animation_rows(spec.animation));
            rows
        }
        OverlayAction::Swap { animation, .. } => {
            let mut rows = vec![OverlayRow::Target { swap: true }];
            rows.extend(animation_rows(*animation));
            rows
        }
        OverlayAction::Close => Vec::new(),
    }
}

/// The grid cells: column, row, position. The middle row's sides have
/// no anchor.
const GRID: [(usize, usize, OverlayPosition); 7] = [
    (0, 0, OverlayPosition::TopLeft),
    (1, 0, OverlayPosition::TopCenter),
    (2, 0, OverlayPosition::TopRight),
    (1, 1, OverlayPosition::Center),
    (0, 2, OverlayPosition::BottomLeft),
    (1, 2, OverlayPosition::BottomCenter),
    (2, 2, OverlayPosition::BottomRight),
];
const GRID_CELL_W: f32 = 22.0;
const GRID_GAP: f32 = 2.0;

/// The grid's cells and the Manual cell inside `row`.
fn position_cells(row: Rect) -> (Vec<(OverlayPosition, Rect)>, Rect) {
    let cell_h = (row.size.y - GRID_GAP * 2.0) / 3.0;
    let cells = GRID
        .iter()
        .map(|(col, line, position)| {
            (
                *position,
                Rect::xywh(
                    row.origin.x + *col as f32 * (GRID_CELL_W + GRID_GAP),
                    row.origin.y + *line as f32 * (cell_h + GRID_GAP),
                    GRID_CELL_W,
                    cell_h,
                ),
            )
        })
        .collect();
    let grid_w = GRID_CELL_W * 3.0 + GRID_GAP * 2.0 + 8.0;
    let manual = Rect::xywh(
        row.origin.x + grid_w,
        row.origin.y,
        (row.size.x - grid_w).max(0.0),
        row.size.y,
    );
    (cells, manual)
}

const ANIMATIONS: [OverlayAnimationKind; 3] = OverlayAnimationKind::ALL;
const EDGES: [OverlayEdge; 4] = OverlayEdge::ALL;

/// The clickable rects of one overlay row.
pub(crate) fn push_overlay_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    trigger: InteractionTrigger,
    row: OverlayRow,
    rect: Rect,
    spec: Option<&OverlaySpec>,
) {
    let option = |edit| PropertyPanelAction::SetOverlayOption { trigger, edit };
    match row {
        OverlayRow::Target { swap } => out.push((
            PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::OverlayTarget {
                trigger,
                swap,
            }),
            rect,
        )),
        OverlayRow::Position => {
            let (cells, manual) = position_cells(rect);
            for (position, cell) in cells {
                out.push((option(OverlayOptionEdit::Position(position)), cell));
            }
            out.push((
                option(OverlayOptionEdit::Position(OverlayPosition::Manual)),
                manual,
            ));
        }
        OverlayRow::Outside => {
            let on = spec.is_none_or(|spec| spec.close_on_outside_click);
            out.push((option(OverlayOptionEdit::CloseOnOutsideClick(!on)), rect));
        }
        OverlayRow::Background => {
            let cells = crate::widgets::property_panel_segments::segment_rects(rect, 2);
            out.push((option(OverlayOptionEdit::Background(false)), cells[0]));
            out.push((option(OverlayOptionEdit::Background(true)), cells[1]));
        }
        OverlayRow::BackgroundHex => {}
        OverlayRow::Animation => {
            let cells = crate::widgets::property_panel_segments::segment_rects(rect, 3);
            for (kind, cell) in ANIMATIONS.into_iter().zip(cells) {
                out.push((option(OverlayOptionEdit::Animation(kind)), cell));
            }
        }
        OverlayRow::Direction => {
            let cells = crate::widgets::property_panel_segments::segment_rects(rect, 4);
            for (edge, cell) in EDGES.into_iter().zip(cells) {
                out.push((option(OverlayOptionEdit::Direction(edge)), cell));
            }
        }
    }
}

/// The section's text input for this row, if it is one.
pub(crate) fn overlay_input_focus(
    trigger: InteractionTrigger,
    row: OverlayRow,
) -> Option<de_editor_core::PropertyFocus> {
    (row == OverlayRow::BackgroundHex)
        .then_some(de_editor_core::PropertyFocus::OverlayBackground(trigger))
}

/// The animation and its kind of an overlay action.
fn animation_of(action: &OverlayAction) -> OverlayAnimation {
    match action {
        OverlayAction::Open(spec) => spec.animation,
        OverlayAction::Swap { animation, .. } => *animation,
        OverlayAction::Close => OverlayAnimation::Instant,
    }
}

/// Paint one overlay row.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_overlay_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    summary: &InteractionSummary,
    edit: &crate::widgets::property_panel_sections::EditContext<'_>,
    trigger: InteractionTrigger,
    action: &OverlayAction,
    row: OverlayRow,
    rect: Rect,
) {
    let spec = match action {
        OverlayAction::Open(spec) => Some(spec),
        _ => None,
    };
    match row {
        OverlayRow::Target { .. } => {
            let target = action.target().unwrap_or_default();
            let label = format!(
                "{}: {}",
                translate(locale, "interactions.overlayFrame", "Frame"),
                summary.overlay_name(target)
            );
            paint_fitted_dropdown(cx, theme, rect, &label);
        }
        OverlayRow::Position => {
            let current = spec.map_or(OverlayPosition::Center, |spec| spec.position);
            let (cells, manual) = position_cells(rect);
            for (position, cell) in cells {
                let color = if position == current {
                    theme.primary
                } else {
                    theme.muted
                };
                cx.backend.fill_round_rect(cell, 2.0, color);
            }
            let labels = [translate(locale, "interactions.overlayManual", "Manual")];
            let active = (current == OverlayPosition::Manual).then_some(0);
            crate::widgets::property_panel_segments::paint_segment_row(
                cx, theme, manual, &labels, active, 11.0,
            );
        }
        OverlayRow::Outside => {
            crate::widgets::property_panel_sections::paint_check_row(
                cx,
                theme,
                rect.origin.x,
                rect.origin.y,
                rect.size.x,
                translate(
                    locale,
                    "interactions.overlayCloseOutside",
                    "Close on outside click",
                ),
                spec.is_none_or(|spec| spec.close_on_outside_click),
                rect.size.y,
            );
        }
        OverlayRow::Background => {
            let labels = [
                translate(locale, "interactions.overlayNoBackground", "No background"),
                translate(locale, "interactions.overlayDim", "Dim"),
            ];
            let active = Some(usize::from(spec.is_some_and(|s| s.background.is_some())));
            crate::widgets::property_panel_segments::paint_segment_row(
                cx, theme, rect, &labels, active, 11.0,
            );
        }
        OverlayRow::BackgroundHex => {
            let focus = de_editor_core::PropertyFocus::OverlayBackground(trigger);
            let value = spec
                .and_then(|spec| spec.background.clone())
                .unwrap_or_default();
            // A swatch of the dim at the row's right end.
            let swatch = Rect::xywh(
                rect.origin.x + rect.size.x - rect.size.y,
                rect.origin.y,
                rect.size.y,
                rect.size.y,
            );
            let input = Rect::xywh(
                rect.origin.x,
                rect.origin.y,
                rect.size.x - rect.size.y - 4.0,
                rect.size.y,
            );
            crate::widgets::property_panel_inputs::paint_input_with_prefix_focused_state(
                cx,
                theme,
                input,
                translate(locale, "interactions.overlayColor", "Color"),
                edit.value_for(focus, &value),
                edit.focus == Some(focus),
                edit.caret_at(focus),
                edit.select_all_at(focus),
                edit.input_at(focus),
                edit.now_ms,
            );
            if let Some([r, g, b, a]) = spec.and_then(OverlaySpec::background_rgba) {
                cx.backend.fill_round_rect(
                    swatch,
                    6.0,
                    crate::Color {
                        r,
                        g,
                        b,
                        a: a.max(0.15),
                    },
                );
            }
            cx.backend.stroke_round_rect(swatch, 6.0, theme.border, 1.0);
        }
        OverlayRow::Animation => {
            let labels = [
                translate(locale, "interactions.overlayInstant", "Instant"),
                translate(locale, "interactions.overlayDissolve", "Dissolve"),
                translate(locale, "interactions.overlayMoveIn", "Move in"),
            ];
            let kind = OverlayAnimationKind::of(animation_of(action));
            let active = ANIMATIONS.iter().position(|k| *k == kind);
            crate::widgets::property_panel_segments::paint_segment_row(
                cx, theme, rect, &labels, active, 11.0,
            );
        }
        OverlayRow::Direction => {
            let labels = [
                translate(locale, "interactions.overlayFromTop", "Top"),
                translate(locale, "interactions.overlayFromBottom", "Bottom"),
                translate(locale, "interactions.overlayFromLeft", "Left"),
                translate(locale, "interactions.overlayFromRight", "Right"),
            ];
            let active = match animation_of(action) {
                OverlayAnimation::MoveIn(edge) => EDGES.iter().position(|e| *e == edge),
                _ => None,
            };
            crate::widgets::property_panel_segments::paint_segment_row(
                cx, theme, rect, &labels, active, 11.0,
            );
        }
    }
}

/// The popover rows that pick a frame for `trigger`'s overlay.
pub(crate) fn overlay_target_menu_rows(
    summary: &InteractionSummary,
    trigger: InteractionTrigger,
    swap: bool,
) -> Vec<(PropertyPanelAction, String)> {
    summary
        .overlay_targets
        .iter()
        .map(|target| {
            let label = match &target.page {
                Some(page) => format!("{} · {page}", target.name),
                None => target.name.clone(),
            };
            (
                PropertyPanelAction::SetInteractionOverlay {
                    trigger,
                    target: target.id.clone(),
                    swap,
                },
                label,
            )
        })
        .collect()
}

/// The row label of an overlay action: `Open overlay Menu`.
pub(crate) fn overlay_action_label(
    locale: de_editor_core::Locale,
    summary: &InteractionSummary,
    action: &OverlayAction,
) -> String {
    match action {
        OverlayAction::Open(spec) => format!(
            "{} {}",
            translate(locale, "interactions.openOverlay", "Open overlay"),
            summary.overlay_name(&spec.target)
        ),
        OverlayAction::Swap { target, .. } => format!(
            "{} {}",
            translate(locale, "interactions.swapOverlay", "Swap overlay"),
            summary.overlay_name(target)
        ),
        OverlayAction::Close => {
            translate(locale, "interactions.closeOverlay", "Close overlay").to_owned()
        }
    }
}

#[cfg(test)]
#[path = "property_panel_interactions_overlay_tests.rs"]
mod tests;
