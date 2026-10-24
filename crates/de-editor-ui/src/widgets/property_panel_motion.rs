//! Motion section (#154): the node's `transition` and `animations`,
//! painted right below the Interactions section in the same slot.
//!
//! A node-level `transition` (#176) is two rows at the top: "Transition"
//! with its easing dropdown and remove, then its duration.
//!
//! Each animation the panel can express (one property, two stops) is
//! three rows: the property dropdown + the trigger dropdown (On load /
//! In view / On hover / On tap, #155) + remove, then From | To, then
//! Duration | Delay (ms). Any other shape is one row — "Custom animation"
//! with its trigger dropdown and remove — so a hand-written or
//! MCP-written declaration is never rewritten by accident.
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
use de_editor_core::transition_edit::{MotionEasing, TransitionSummary};
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

    fn row_count(&self) -> usize {
        match self {
            MotionSummary::Simple(_) => 3,
            MotionSummary::Custom { .. } => 1,
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
const TRIGGER_DROPDOWN_W: f32 = 116.0;
const REMOVE_SIZE: f32 = 22.0;
/// The triggers the trigger dropdown lists, in order.
pub const MOTION_TRIGGERS: [MotionTrigger; 4] = [
    MotionTrigger::Mount,
    MotionTrigger::InView,
    MotionTrigger::Hover,
    MotionTrigger::Tap,
];

/// Rows the node-level transition takes (label + easing, then duration).
const TRANSITION_ROWS: usize = 2;
/// Width of the Transition row's label column.
const TRANSITION_LABEL_W: f32 = 76.0;

fn transition_rows(summary: &InteractionSummary) -> usize {
    if summary.transition.is_some() {
        TRANSITION_ROWS
    } else {
        0
    }
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
    let rows: usize = summary
        .motion
        .iter()
        .map(MotionSummary::row_count)
        .sum::<usize>()
        + transition_rows(summary);
    SECTION_HEADER_HEIGHT + rows as f32 * ROW_STEP + 6.0 + SECTION_GAP
}

/// The Transition head row's parts: easing dropdown and remove.
fn transition_head(x: f32, y: f32, width: f32) -> (Rect, Rect) {
    let head = head_row(x, y, width);
    let dropdown = Rect::xywh(
        head.dropdown.origin.x + TRANSITION_LABEL_W,
        y,
        (head.remove.origin.x - 6.0 - head.dropdown.origin.x - TRANSITION_LABEL_W).max(0.0),
        INPUT_HEIGHT,
    );
    (dropdown, head.remove)
}

/// The first row's parts: property dropdown, trigger dropdown, remove.
struct HeadRow {
    dropdown: Rect,
    triggers: Rect,
    remove: Rect,
}

fn head_row(x: f32, y: f32, width: f32) -> HeadRow {
    let row = Rect::xywh(x + PAD_X, y, width - PAD_X * 2.0, INPUT_HEIGHT);
    let remove = Rect::xywh(
        row.origin.x + row.size.x - REMOVE_SIZE,
        y + (INPUT_HEIGHT - REMOVE_SIZE) / 2.0,
        REMOVE_SIZE,
        REMOVE_SIZE,
    );
    let triggers = Rect::xywh(
        remove.origin.x - 4.0 - TRIGGER_DROPDOWN_W,
        y,
        TRIGGER_DROPDOWN_W,
        INPUT_HEIGHT,
    );
    let dropdown = Rect::xywh(
        row.origin.x,
        y,
        (triggers.origin.x - 6.0 - row.origin.x).max(0.0),
        INPUT_HEIGHT,
    );
    HeadRow {
        dropdown,
        triggers,
        remove,
    }
}

/// The two half-width inputs of a value row.
fn value_pair(x: f32, y: f32, width: f32) -> [Rect; 2] {
    let half = (width - PAD_X * 2.0 - 8.0) / 2.0;
    [
        Rect::xywh(x + PAD_X, y, half, INPUT_HEIGHT),
        Rect::xywh(x + PAD_X + half + 8.0, y, half, INPUT_HEIGHT),
    ]
}

/// Walk the animation rows: `(index, summary, top of its first row)`.
/// They start below the transition rows.
fn row_tops(
    summary: &InteractionSummary,
    y: f32,
) -> impl Iterator<Item = (usize, &MotionSummary, f32)> {
    let mut row_y = y + SECTION_HEADER_HEIGHT + transition_rows(summary) as f32 * ROW_STEP;
    summary
        .motion
        .iter()
        .enumerate()
        .map(move |(index, summary)| {
            let at = row_y;
            row_y += summary.row_count() as f32 * ROW_STEP;
            (index, summary, at)
        })
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
    if interactions.transition.is_some() {
        let (dropdown, remove) = transition_head(x, y + SECTION_HEADER_HEIGHT, width);
        out.push((
            PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::TransitionEasing),
            dropdown,
        ));
        out.push((PropertyPanelAction::RemoveTransition, remove));
    }
    for (index, summary, row_y) in row_tops(interactions, y) {
        let head = head_row(x, row_y, width);
        if matches!(summary, MotionSummary::Simple(_)) {
            out.push((
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::MotionProperty(
                    index,
                )),
                head.dropdown,
            ));
        }
        out.push((
            PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::MotionTrigger(index)),
            head.triggers,
        ));
        out.push((PropertyPanelAction::RemoveAnimation(index), head.remove));
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
    if interactions.transition.is_some() {
        let [duration, _] = value_pair(x, y + SECTION_HEADER_HEIGHT + ROW_STEP, width);
        out.push((PropertyFocus::TransitionDuration, duration));
    }
    for (index, summary, row_y) in row_tops(interactions, y) {
        if !matches!(summary, MotionSummary::Simple(_)) {
            continue;
        }
        let fields = [
            [MotionField::From, MotionField::To],
            [MotionField::DurationMs, MotionField::DelayMs],
        ];
        for (line, pair) in fields.into_iter().enumerate() {
            let rects = value_pair(x, row_y + (line + 1) as f32 * ROW_STEP, width);
            for (field, rect) in pair.into_iter().zip(rects) {
                out.push((PropertyFocus::MotionValue { index, field }, rect));
            }
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

/// The Transition head row: label, easing dropdown, remove.
fn paint_transition_head(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    transition: TransitionSummary,
    x: f32,
    y: f32,
    width: f32,
) {
    let (dropdown, remove) = transition_head(x, y, width);
    let label = Rect::xywh(x + PAD_X, y, TRANSITION_LABEL_W - 4.0, INPUT_HEIGHT);
    paint_plain_row(
        cx,
        theme,
        label,
        translate(locale, "interactions.transition", "Transition"),
    );
    paint_fitted_dropdown(cx, theme, dropdown, easing_label(locale, transition.easing));
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
    if let Some(transition) = interactions.transition {
        paint_transition_head(
            cx,
            theme,
            locale,
            transition,
            x,
            y + SECTION_HEADER_HEIGHT,
            width,
        );
    }
    for (_, summary, row_y) in row_tops(interactions, y) {
        let head = head_row(x, row_y, width);
        match summary {
            MotionSummary::Simple(simple) => {
                paint_fitted_dropdown(
                    cx,
                    theme,
                    head.dropdown,
                    property_label(locale, simple.property),
                );
            }
            MotionSummary::Custom { .. } => paint_plain_row(
                cx,
                theme,
                head.dropdown,
                translate(locale, "motion.custom", "Custom animation"),
            ),
        }
        paint_fitted_dropdown(
            cx,
            theme,
            head.triggers,
            trigger_label(locale, summary.trigger()),
        );
        paint_remove(cx, theme, head.remove);
    }
    let values: Vec<(PropertyFocus, Rect)> = motion_input_rects(interactions, x, y, width);
    for (focus, rect) in values {
        if focus == PropertyFocus::TransitionDuration {
            let value = interactions
                .transition
                .map(|transition| transition.duration_ms.to_string())
                .unwrap_or_default();
            paint_input_with_prefix_focused_state(
                cx,
                theme,
                rect,
                field_prefix(locale, MotionField::DurationMs),
                edit.value_for(focus, &value),
                edit.focus == Some(focus),
                edit.caret_at(focus),
                edit.select_all_at(focus),
                edit.input_at(focus),
                edit.now_ms,
            );
            continue;
        }
        let PropertyFocus::MotionValue { index, field } = focus else {
            continue;
        };
        let Some(MotionSummary::Simple(simple)) = motion.get(index) else {
            continue;
        };
        let value = field_value(simple, field);
        paint_input_with_prefix_focused_state(
            cx,
            theme,
            rect,
            field_prefix(locale, field),
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
