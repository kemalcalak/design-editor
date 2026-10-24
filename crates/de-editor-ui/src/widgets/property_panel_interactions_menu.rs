//! The shared Interactions / Motion popover (#154). One popover serves
//! the header "+" (trigger list), each trigger row (Navigate to … /
//! Back / Remove) and each Motion row's property dropdown, so the hosts'
//! existing hit, hover and dismiss routing covers all of them.
//! `editor_ui.interaction_menu_target` says which list is showing.

use crate::theme::Theme;
use crate::widgets::property_panel::PropertyPanelAction;
use crate::widgets::property_panel_inputs::INPUT_RADIUS;
use crate::widgets::property_panel_interactions::{
    translate, InteractionMenuTarget, InteractionSummary, InteractionTrigger,
};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::motion_edit::MotionProperty;

pub const INTERACTION_MENU_ROW_H: f32 = 30.0;
pub const INTERACTION_MENU_W: f32 = 200.0;

/// Hit-result for a click on the open popover.
#[derive(Debug, Clone, PartialEq)]
pub enum InteractionMenuHit {
    Row(PropertyPanelAction),
    Inside,
    Outside,
}

/// The "+" menu label of a trigger.
fn trigger_label(locale: de_editor_core::Locale, trigger: InteractionTrigger) -> &'static str {
    match trigger {
        InteractionTrigger::Tap => translate(locale, "interactions.onTap", "On tap"),
        InteractionTrigger::Hover => translate(locale, "interactions.onHover", "On hover"),
        InteractionTrigger::LongPress => {
            translate(locale, "interactions.onLongPress", "On long press")
        }
    }
}

/// Popover rows for `target`, in paint order.
/// - `AddTrigger`: the triggers not authored yet, then "Animation".
/// - `Trigger(t)`: one "Navigate to <path>" row per screen on the page,
///   "Back (pop)", then "Remove" when `t` has an action.
/// - `MotionProperty(i)`: every animatable property.
pub fn interaction_menu_rows(
    locale: de_editor_core::Locale,
    target: InteractionMenuTarget,
    screen_paths: &[String],
    summary: &InteractionSummary,
) -> Vec<(PropertyPanelAction, String)> {
    match target {
        InteractionMenuTarget::AddTrigger => {
            let mut rows: Vec<(PropertyPanelAction, String)> = summary
                .unauthored_triggers()
                .into_iter()
                .map(|trigger| {
                    (
                        PropertyPanelAction::PickInteractionTrigger(trigger),
                        trigger_label(locale, trigger).to_string(),
                    )
                })
                .collect();
            rows.push((
                PropertyPanelAction::AddAnimation,
                translate(locale, "interactions.addAnimation", "Animation").to_string(),
            ));
            rows
        }
        InteractionMenuTarget::Trigger(trigger) => {
            let navigate_to = translate(locale, "interactions.navigateTo", "Navigate to");
            let mut rows: Vec<(PropertyPanelAction, String)> = screen_paths
                .iter()
                .map(|path| {
                    (
                        PropertyPanelAction::SetInteractionNavigate {
                            trigger,
                            path: path.clone(),
                        },
                        format!("{navigate_to} {path}"),
                    )
                })
                .collect();
            rows.push((
                PropertyPanelAction::SetInteractionPop(trigger),
                translate(locale, "interactions.back", "Back (pop)").to_string(),
            ));
            if !summary.actions(trigger).is_empty() {
                rows.push((
                    PropertyPanelAction::RemoveInteraction(trigger),
                    translate(locale, "interactions.remove", "Remove").to_string(),
                ));
            }
            rows
        }
        InteractionMenuTarget::MotionProperty(index) => MotionProperty::ALL
            .into_iter()
            .map(|property| {
                (
                    PropertyPanelAction::SetAnimationProperty { index, property },
                    crate::widgets::property_panel_motion::property_label(locale, property)
                        .to_string(),
                )
            })
            .collect(),
    }
}

/// The popover rect, dropping below `anchor`, right-aligned to it.
pub fn interaction_menu_rect(anchor: Rect, row_count: usize) -> Rect {
    let h = row_count as f32 * INTERACTION_MENU_ROW_H + 8.0;
    let right = anchor.origin.x + anchor.size.x;
    Rect {
        origin: Point2D::new(right - INTERACTION_MENU_W, anchor.origin.y + anchor.size.y),
        size: Point2D::new(INTERACTION_MENU_W, h),
    }
}

/// `(action, row_rect)` for each popover row, given the popover rect.
pub fn interaction_menu_row_rects(
    menu: Rect,
    rows: &[(PropertyPanelAction, String)],
) -> Vec<(PropertyPanelAction, Rect)> {
    rows.iter()
        .enumerate()
        .map(|(i, (action, _))| {
            let ry = menu.origin.y + 4.0 + i as f32 * INTERACTION_MENU_ROW_H;
            (
                action.clone(),
                Rect {
                    origin: Point2D::new(menu.origin.x, ry),
                    size: Point2D::new(menu.size.x, INTERACTION_MENU_ROW_H),
                },
            )
        })
        .collect()
}

/// Paint the popover anchored to `anchor`. `hover` highlights the row
/// under the cursor, like the Effects add-menu.
pub fn paint_interaction_menu(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    anchor: Rect,
    rows: &[(PropertyPanelAction, String)],
    hover: Option<usize>,
) {
    let menu = interaction_menu_rect(anchor, rows.len());
    cx.backend
        .fill_round_rect(menu, INPUT_RADIUS, theme.popover);
    cx.backend
        .stroke_round_rect(menu, INPUT_RADIUS, theme.border, 1.0);
    for (i, (_, label)) in rows.iter().enumerate() {
        let ry = menu.origin.y + 4.0 + i as f32 * INTERACTION_MENU_ROW_H;
        if hover == Some(i) {
            let row = Rect {
                origin: Point2D::new(menu.origin.x + 4.0, ry),
                size: Point2D::new(menu.size.x - 8.0, INTERACTION_MENU_ROW_H),
            };
            cx.backend.fill_round_rect(row, 6.0, theme.muted);
        }
        let label =
            crate::widgets::text_metrics::fit_chrome(cx.backend, label, menu.size.x - 24.0, 12.0);
        let text = TextLayout::single_run(
            &label,
            crate::widgets::text_metrics::CHROME_FONT_FAMILY,
            12.0,
            theme.foreground.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &text,
            Point2D::new(
                menu.origin.x + 12.0,
                ry + INTERACTION_MENU_ROW_H / 2.0 + 4.0,
            ),
        );
    }
}
