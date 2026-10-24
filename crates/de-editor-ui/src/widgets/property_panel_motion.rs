//! Motion section (#154): the node's `animations`, painted right below
//! the Interactions section in the same slot.
//!
//! Each animation the panel can express (one property, two stops) is
//! three rows: the property dropdown + On load | In view + remove, then
//! From | To, then Duration | Delay (ms). Any other shape is one row —
//! "Custom animation" with its trigger segments and remove — so a
//! hand-written or MCP-written declaration is never rewritten by
//! accident. The section shows once the node has an animation (the
//! first one comes from the Interactions "+"); its header "+" adds
//! another fade-in. Writes live in
//! `de_editor_core::motion_edit`.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    paint_dropdown, paint_input_with_prefix_focused_state, paint_section_divider,
    paint_section_label_with_add, section_add_target, INPUT_HEIGHT, PAD_X, SECTION_GAP,
    SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_interactions::{
    paint_plain_row, translate, InteractionMenuTarget,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::motion_edit::{simple_motion, MotionField, MotionProperty, SimpleMotion};
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
const TRIGGER_SEGMENTS_W: f32 = 116.0;
const REMOVE_SIZE: f32 = 22.0;
const TRIGGERS: [MotionTrigger; 2] = [MotionTrigger::Mount, MotionTrigger::InView];

/// Total vertical space the Motion section consumes.
/// Zero while the node has no animation: the section then does not
/// paint at all and is added from the Interactions "+".
pub fn motion_section_height(motion: &[MotionSummary]) -> f32 {
    if motion.is_empty() {
        return 0.0;
    }
    let rows: usize = motion.iter().map(MotionSummary::row_count).sum();
    SECTION_HEADER_HEIGHT + rows as f32 * ROW_STEP + 6.0 + SECTION_GAP
}

/// The first row's parts: property dropdown, trigger segments, remove.
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
        remove.origin.x - 4.0 - TRIGGER_SEGMENTS_W,
        y,
        TRIGGER_SEGMENTS_W,
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

/// Walk the rows: `(index, summary, top of its first row)`.
fn row_tops(
    motion: &[MotionSummary],
    y: f32,
) -> impl Iterator<Item = (usize, &MotionSummary, f32)> {
    let mut row_y = y + SECTION_HEADER_HEIGHT;
    motion.iter().enumerate().map(move |(index, summary)| {
        let at = row_y;
        row_y += summary.row_count() as f32 * ROW_STEP;
        (index, summary, at)
    })
}

pub(crate) fn push_motion_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    motion: &[MotionSummary],
    x: f32,
    y: f32,
    width: f32,
) {
    if motion.is_empty() {
        return;
    }
    out.push((
        PropertyPanelAction::AddAnimation,
        section_add_target(x, y, width),
    ));
    for (index, summary, row_y) in row_tops(motion, y) {
        let head = head_row(x, row_y, width);
        if matches!(summary, MotionSummary::Simple(_)) {
            out.push((
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::MotionProperty(
                    index,
                )),
                head.dropdown,
            ));
        }
        let cells = crate::widgets::property_panel_segments::segment_rects(head.triggers, 2);
        for (trigger, cell) in TRIGGERS.into_iter().zip(cells) {
            out.push((
                PropertyPanelAction::SetAnimationTrigger { index, trigger },
                cell,
            ));
        }
        out.push((PropertyPanelAction::RemoveAnimation(index), head.remove));
    }
}

/// The From / To / Duration / Delay inputs of every simple row.
pub fn motion_input_rects(
    motion: &[MotionSummary],
    x: f32,
    y: f32,
    width: f32,
) -> Vec<(PropertyFocus, Rect)> {
    let mut out = Vec::new();
    for (index, summary, row_y) in row_tops(motion, y) {
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

/// [`motion_input_rects`] placed in the panel: the Motion section
/// starts below the Interactions rows at the shared Interactions top.
pub fn motion_input_rects_in_panel(
    panel_rect: Rect,
    visible: crate::widgets::property_panel_visibility::VisibleSections,
    effects: &[crate::widgets::property_panel::EffectSummary],
    fills: &[crate::widgets::property_panel::FillSummary],
    interactions: &crate::widgets::property_panel_interactions::InteractionSummary,
) -> Vec<(PropertyFocus, Rect)> {
    if !visible.interactions || interactions.motion.is_empty() {
        return Vec::new();
    }
    let tops = crate::widgets::property_panel_section_tops::section_tops(
        panel_rect.origin.y,
        visible,
        effects,
        fills,
        interactions,
    );
    let top = tops.interactions
        + crate::widgets::property_panel_interactions::interactions_section_height(interactions);
    motion_input_rects(
        &interactions.motion,
        panel_rect.origin.x,
        top,
        panel_rect.size.x,
    )
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
    }
}

/// The value a field shows when it is not being edited.
pub fn field_value(simple: &SimpleMotion, field: MotionField) -> String {
    match field {
        MotionField::From => crate::util::format_panel_number(simple.from as f32),
        MotionField::To => crate::util::format_panel_number(simple.to as f32),
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
    motion: &[MotionSummary],
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    if motion.is_empty() {
        return y;
    }
    let title = translate(locale, "motion.title", "Motion");
    paint_section_label_with_add(cx, theme, title, x, y, width);
    let triggers = [
        translate(locale, "motion.onLoad", "On load"),
        translate(locale, "motion.inView", "In view"),
    ];
    for (_, summary, row_y) in row_tops(motion, y) {
        let head = head_row(x, row_y, width);
        match summary {
            MotionSummary::Simple(simple) => {
                paint_dropdown(
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
        let active = TRIGGERS.iter().position(|t| *t == summary.trigger());
        crate::widgets::property_panel_segments::paint_segment_row(
            cx,
            theme,
            head.triggers,
            &triggers,
            active,
            11.0,
        );
        draw_icon(
            cx.backend,
            Icon::Close,
            Point2D::new(
                head.remove.origin.x + (REMOVE_SIZE - 14.0) / 2.0,
                head.remove.origin.y + (REMOVE_SIZE - 14.0) / 2.0,
            ),
            14.0,
            theme.muted_foreground,
            1.3,
        );
    }
    let values: Vec<(PropertyFocus, Rect)> = motion_input_rects(motion, x, y, width);
    for (focus, rect) in values {
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
    let y = y + motion_section_height(motion) - SECTION_GAP;
    paint_section_divider(cx, theme, x, y, width);
    y + SECTION_GAP
}

#[cfg(test)]
#[path = "property_panel_motion_tests.rs"]
mod tests;
