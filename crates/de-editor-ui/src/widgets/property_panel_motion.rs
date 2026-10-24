//! Motion section (#154): the node's `transition` and `animations`,
//! painted right below the Interactions section in the same slot.
//!
//! A node-level `transition` (#176) is three rows at the top: "Transition"
//! with remove, its easing dropdown, then its duration.
//!
//! Each animation the panel can express (one property, two stops) is
//! five rows: the property dropdown + remove, the trigger dropdown (On
//! load / In view / On hover / On tap, #155), From | To, Duration, Delay
//! (ms). Any other shape is two rows — "Custom animation" with remove,
//! then its trigger dropdown — so a hand-written or MCP-written
//! declaration is never rewritten by accident. A localized label never
//! shares its row with another one (#190): the longest translation fits.
//!
//! The section shows once the node has an animation or a transition
//! (both are added from the Interactions "+"); its header "+" adds
//! another fade-in. Writes live in `de_editor_core::motion_edit` and
//! `transition_edit`.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, paint_section_divider, paint_section_label_with_add,
    section_add_target, INPUT_HEIGHT, PAD_X, SECTION_GAP, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_interactions::{
    paint_fitted_dropdown, paint_plain_row, translate, InteractionMenuTarget, InteractionSummary,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::motion_edit::{simple_motion, MotionField, MotionProperty, SimpleMotion};
use de_editor_core::transition_edit::MotionEasing;
use de_editor_core::PropertyFocus;
use jian_ops_schema::motion::MotionTrigger;
use jian_ops_schema::node::PenNode;

/// One Motion row, summarized for display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionSummary {
    Simple(SimpleMotion),
    /// A declaration the panel shows read-only (trigger and remove only).
    Custom {
        trigger: MotionTrigger,
    },
}

impl MotionSummary {
    pub fn trigger(&self) -> MotionTrigger {
        match self {
            MotionSummary::Simple(simple) => simple.trigger,
            MotionSummary::Custom { trigger } => *trigger,
        }
    }
}

pub(crate) fn motion_of(node: &PenNode) -> Vec<MotionSummary> {
    node.motion_declarations()
        .1
        .map(|list| {
            list.iter()
                .map(|animation| match simple_motion(animation) {
                    Some(simple) => MotionSummary::Simple(simple),
                    None => MotionSummary::Custom {
                        trigger: animation.trigger,
                    },
                })
                .collect()
        })
        .unwrap_or_default()
}

const ROW_STEP: f32 = INPUT_HEIGHT + 6.0;
const REMOVE_SIZE: f32 = 22.0;
/// The triggers the trigger dropdown lists, in order.
pub const MOTION_TRIGGERS: [MotionTrigger; 4] = [
    MotionTrigger::Mount,
    MotionTrigger::InView,
    MotionTrigger::Hover,
    MotionTrigger::Tap,
];

/// One painted line of the section (#190). Every control that carries a
/// localized label gets a full-width line of its own (the remove "×"
/// excepted), so no label shares a row with another label and the longest
/// translation in each of the 15 locales fits without an ellipsis.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Line {
    /// "Transition" + remove.
    TransitionHead,
    /// The transition's easing dropdown.
    TransitionEasing,
    /// The transition's duration input.
    TransitionDuration,
    /// Animation `index`: the property dropdown (or the read-only
    /// "Custom animation" label) + remove.
    Head(usize),
    /// Animation `index`: its trigger dropdown.
    Trigger(usize),
    /// Animation `index`: two half-width number inputs.
    Pair(usize, [MotionField; 2]),
    /// Animation `index`: one full-width input.
    Field(usize, MotionField),
}

/// The section's lines, top to bottom — the one walk height, hit rects,
/// inputs and paint share.
fn lines(summary: &InteractionSummary) -> Vec<Line> {
    let mut out = Vec::new();
    if summary.transition.is_some() {
        out.extend([
            Line::TransitionHead,
            Line::TransitionEasing,
            Line::TransitionDuration,
        ]);
    }
    for (index, motion) in summary.motion.iter().enumerate() {
        out.extend([Line::Head(index), Line::Trigger(index)]);
        if let MotionSummary::Simple(_) = motion {
            out.extend([
                Line::Pair(index, [MotionField::From, MotionField::To]),
                Line::Field(index, MotionField::DurationMs),
                Line::Field(index, MotionField::DelayMs),
            ]);
        }
    }
    out
}

/// Each line with the top of its row.
fn line_tops(summary: &InteractionSummary, y: f32) -> impl Iterator<Item = (Line, f32)> {
    let top = y + SECTION_HEADER_HEIGHT;
    lines(summary)
        .into_iter()
        .enumerate()
        .map(move |(i, line)| (line, top + i as f32 * ROW_STEP))
}

/// Whether the section shows at all: an animation or a transition.
fn shown(summary: &InteractionSummary) -> bool {
    !summary.motion.is_empty() || summary.transition.is_some()
}

/// Total vertical space the Motion section consumes.
/// Zero while the node has neither an animation nor a transition: the
/// section then does not paint at all and is added from the
/// Interactions "+".
pub fn motion_section_height(summary: &InteractionSummary) -> f32 {
    if !shown(summary) {
        return 0.0;
    }
    SECTION_HEADER_HEIGHT + lines(summary).len() as f32 * ROW_STEP + 6.0 + SECTION_GAP
}

/// A full-width row of the section.
fn full_row(x: f32, y: f32, width: f32) -> Rect {
    Rect::xywh(x + PAD_X, y, width - PAD_X * 2.0, INPUT_HEIGHT)
}

/// A head row's parts: the body (dropdown or label) and the remove "×".
fn head_parts(x: f32, y: f32, width: f32) -> (Rect, Rect) {
    let row = full_row(x, y, width);
    let remove = Rect::xywh(
        row.origin.x + row.size.x - REMOVE_SIZE,
        y + (INPUT_HEIGHT - REMOVE_SIZE) / 2.0,
        REMOVE_SIZE,
        REMOVE_SIZE,
    );
    let body = Rect::xywh(
        row.origin.x,
        y,
        (remove.origin.x - 6.0 - row.origin.x).max(0.0),
        INPUT_HEIGHT,
    );
    (body, remove)
}

/// The two half-width inputs of a value row.
fn value_pair(x: f32, y: f32, width: f32) -> [Rect; 2] {
    let half = (width - PAD_X * 2.0 - 8.0) / 2.0;
    [
        Rect::xywh(x + PAD_X, y, half, INPUT_HEIGHT),
        Rect::xywh(x + PAD_X + half + 8.0, y, half, INPUT_HEIGHT),
    ]
}

pub(crate) fn push_motion_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    interactions: &InteractionSummary,
    x: f32,
    y: f32,
    width: f32,
) {
    if !shown(interactions) {
        return;
    }
    out.push((
        PropertyPanelAction::AddAnimation,
        section_add_target(x, y, width),
    ));
    for (line, row_y) in line_tops(interactions, y) {
        match line {
            Line::TransitionHead => {
                let (_, remove) = head_parts(x, row_y, width);
                out.push((PropertyPanelAction::RemoveTransition, remove));
            }
            Line::TransitionEasing => out.push((
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::TransitionEasing),
                full_row(x, row_y, width),
            )),
            Line::Head(index) => {
                let (body, remove) = head_parts(x, row_y, width);
                if matches!(
                    interactions.motion.get(index),
                    Some(MotionSummary::Simple(_))
                ) {
                    out.push((
                        PropertyPanelAction::ToggleInteractionMenu(
                            InteractionMenuTarget::MotionProperty(index),
                        ),
                        body,
                    ));
                }
                out.push((PropertyPanelAction::RemoveAnimation(index), remove));
            }
            Line::Trigger(index) => out.push((
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::MotionTrigger(
                    index,
                )),
                full_row(x, row_y, width),
            )),
            Line::TransitionDuration | Line::Pair(..) | Line::Field(..) => {}
        }
    }
}

/// The transition's duration input, then the From / To / Duration /
/// Delay inputs of every simple row.
pub fn motion_input_rects(
    interactions: &InteractionSummary,
    x: f32,
    y: f32,
    width: f32,
) -> Vec<(PropertyFocus, Rect)> {
    let mut out = Vec::new();
    for (line, row_y) in line_tops(interactions, y) {
        match line {
            Line::TransitionDuration => {
                out.push((PropertyFocus::TransitionDuration, full_row(x, row_y, width)));
            }
            Line::Pair(index, fields) => {
                for (field, rect) in fields.into_iter().zip(value_pair(x, row_y, width)) {
                    out.push((PropertyFocus::MotionValue { index, field }, rect));
                }
            }
            Line::Field(index, field) => out.push((
                PropertyFocus::MotionValue { index, field },
                full_row(x, row_y, width),
            )),
            Line::TransitionHead | Line::TransitionEasing | Line::Head(_) | Line::Trigger(_) => {}
        }
    }
    out
}

/// The Interactions URL inputs and [`motion_input_rects`] placed in the
/// panel: the Motion section starts below the Interactions rows at the
/// shared Interactions top.
pub fn motion_input_rects_in_panel(
    panel_rect: Rect,
    visible: crate::widgets::property_panel_visibility::VisibleSections,
    effects: &[crate::widgets::property_panel::EffectSummary],
    fills: &[crate::widgets::property_panel::FillSummary],
    interactions: &crate::widgets::property_panel_interactions::InteractionSummary,
) -> Vec<(PropertyFocus, Rect)> {
    if !visible.interactions {
        return Vec::new();
    }
    let tops = crate::widgets::property_panel_section_tops::section_tops(
        panel_rect.origin.y,
        visible,
        effects,
        fills,
        interactions,
    );
    let mut out = crate::widgets::property_panel_interactions::interaction_input_rects(
        interactions,
        panel_rect.origin.x,
        tops.interactions,
        panel_rect.size.x,
    );
    let top = tops.interactions
        + crate::widgets::property_panel_interactions::interactions_section_height(interactions);
    out.extend(motion_input_rects(
        interactions,
        panel_rect.origin.x,
        top,
        panel_rect.size.x,
    ));
    out
}

/// Display name of an easing; `None` is a cubic-bezier easing.
pub fn easing_label(locale: de_editor_core::Locale, easing: Option<MotionEasing>) -> &'static str {
    match easing {
        Some(MotionEasing::Standard) => translate(locale, "motion.easeStandard", "Standard"),
        Some(MotionEasing::Emphasized) => translate(locale, "motion.easeEmphasized", "Emphasized"),
        Some(MotionEasing::EmphasizedDecelerate) => {
            translate(locale, "motion.easeEmphasizedDecelerate", "Emphasized out")
        }
        Some(MotionEasing::EmphasizedAccelerate) => {
            translate(locale, "motion.easeEmphasizedAccelerate", "Emphasized in")
        }
        Some(MotionEasing::Linear) => translate(locale, "motion.easeLinear", "Linear"),
        Some(MotionEasing::Ease) => translate(locale, "motion.ease", "Ease"),
        Some(MotionEasing::EaseIn) => translate(locale, "motion.easeIn", "Ease in"),
        Some(MotionEasing::EaseOut) => translate(locale, "motion.easeOut", "Ease out"),
        Some(MotionEasing::EaseInOut) => translate(locale, "motion.easeInOut", "Ease in-out"),
        None => translate(locale, "motion.easeCustom", "Custom"),
    }
}

fn paint_remove(cx: &mut PaintCx<'_>, theme: &Theme, remove: Rect) {
    draw_icon(
        cx.backend,
        Icon::Close,
        Point2D::new(
            remove.origin.x + (REMOVE_SIZE - 14.0) / 2.0,
            remove.origin.y + (REMOVE_SIZE - 14.0) / 2.0,
        ),
        14.0,
        theme.muted_foreground,
        1.3,
    );
}

/// The Transition head row: its label and remove.
fn paint_transition_head(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) {
    let (label, remove) = head_parts(x, y, width);
    paint_plain_row(
        cx,
        theme,
        label,
        translate(locale, "interactions.transition", "Transition"),
    );
    paint_remove(cx, theme, remove);
}

/// Display name of an animatable property.
pub fn property_label(locale: de_editor_core::Locale, property: MotionProperty) -> &'static str {
    match property {
        MotionProperty::Opacity => translate(locale, "motion.opacity", "Opacity"),
        MotionProperty::TranslateX => translate(locale, "motion.translateX", "Move X"),
        MotionProperty::TranslateY => translate(locale, "motion.translateY", "Move Y"),
        MotionProperty::ScaleX => translate(locale, "motion.scaleX", "Scale X"),
        MotionProperty::ScaleY => translate(locale, "motion.scaleY", "Scale Y"),
        MotionProperty::Rotation => translate(locale, "motion.rotation", "Rotation"),
        MotionProperty::Width => translate(locale, "motion.width", "Width"),
        MotionProperty::Height => translate(locale, "motion.height", "Height"),
        MotionProperty::CornerRadius => translate(locale, "motion.cornerRadius", "Corner radius"),
    }
}

/// Display name of a motion trigger.
pub fn trigger_label(locale: de_editor_core::Locale, trigger: MotionTrigger) -> &'static str {
    match trigger {
        MotionTrigger::Mount => translate(locale, "motion.onLoad", "On load"),
        MotionTrigger::InView => translate(locale, "motion.inView", "In view"),
        MotionTrigger::Hover => translate(locale, "interactions.onHover", "On hover"),
        MotionTrigger::Tap => translate(locale, "interactions.onTap", "On tap"),
    }
}

/// The value a field shows when it is not being edited.
pub fn field_value(simple: &SimpleMotion, field: MotionField) -> String {
    match field {
        MotionField::From => de_editor_core::motion_edit::format_motion_number(simple.from),
        MotionField::To => de_editor_core::motion_edit::format_motion_number(simple.to),
        MotionField::DurationMs => simple.duration_ms.to_string(),
        MotionField::DelayMs => simple.delay_ms.to_string(),
    }
}

fn field_prefix(locale: de_editor_core::Locale, field: MotionField) -> &'static str {
    match field {
        MotionField::From => translate(locale, "motion.from", "From"),
        MotionField::To => translate(locale, "motion.to", "To"),
        MotionField::DurationMs => translate(locale, "motion.duration", "Duration"),
        MotionField::DelayMs => translate(locale, "motion.delay", "Delay"),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_motion_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    interactions: &InteractionSummary,
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    if !shown(interactions) {
        return y;
    }
    let motion = interactions.motion.as_slice();
    let title = translate(locale, "motion.title", "Motion");
    paint_section_label_with_add(cx, theme, title, x, y, width);
    for (line, row_y) in line_tops(interactions, y) {
        match line {
            Line::TransitionHead => paint_transition_head(cx, theme, locale, x, row_y, width),
            Line::TransitionEasing => {
                let easing = interactions.transition.and_then(|t| t.easing);
                paint_fitted_dropdown(
                    cx,
                    theme,
                    full_row(x, row_y, width),
                    easing_label(locale, easing),
                );
            }
            Line::Head(index) => {
                let (body, remove) = head_parts(x, row_y, width);
                match motion.get(index) {
                    Some(MotionSummary::Simple(simple)) => paint_fitted_dropdown(
                        cx,
                        theme,
                        body,
                        property_label(locale, simple.property),
                    ),
                    _ => paint_plain_row(
                        cx,
                        theme,
                        body,
                        translate(locale, "motion.custom", "Custom animation"),
                    ),
                }
                paint_remove(cx, theme, remove);
            }
            Line::Trigger(index) => {
                if let Some(summary) = motion.get(index) {
                    paint_fitted_dropdown(
                        cx,
                        theme,
                        full_row(x, row_y, width),
                        trigger_label(locale, summary.trigger()),
                    );
                }
            }
            Line::TransitionDuration | Line::Pair(..) | Line::Field(..) => {}
        }
    }
    for (focus, rect) in motion_input_rects(interactions, x, y, width) {
        let (prefix, value) = match focus {
            PropertyFocus::TransitionDuration => (
                field_prefix(locale, MotionField::DurationMs),
                interactions
                    .transition
                    .map(|transition| transition.duration_ms.to_string())
                    .unwrap_or_default(),
            ),
            PropertyFocus::MotionValue { index, field } => {
                let Some(MotionSummary::Simple(simple)) = motion.get(index) else {
                    continue;
                };
                (field_prefix(locale, field), field_value(simple, field))
            }
            _ => continue,
        };
        paint_input_with_prefix_focused_state(
            cx,
            theme,
            rect,
            prefix,
            edit.value_for(focus, &value),
            edit.focus == Some(focus),
            edit.caret_at(focus),
            edit.select_all_at(focus),
            edit.input_at(focus),
            edit.now_ms,
        );
    }
    let y = y + motion_section_height(interactions) - SECTION_GAP;
    paint_section_divider(cx, theme, x, y, width);
    y + SECTION_GAP
}

#[cfg(test)]
#[path = "property_panel_motion_tests.rs"]
mod tests;
