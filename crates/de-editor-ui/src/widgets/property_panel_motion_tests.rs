//! Tests for the Motion section's parsing and row geometry.

use super::*;

fn node_from(json: &str) -> PenNode {
    serde_json::from_str(json).expect("test fixture parses")
}

const FADE: &str = r#"{"type":"rectangle","id":"r1","animations":[
    {"trigger":"mount","durationMs":300,
     "keyframes":[{"offset":0,"values":{"opacity":0}},{"offset":1,"values":{"opacity":1}}]},
    {"trigger":"inView","durationMs":200,
     "keyframes":{"from":{"opacity":0,"scaleX":0.5},"to":{"opacity":1,"scaleX":1}}}]}"#;

#[test]
fn motion_of_reads_simple_and_custom_rows() {
    let motion = motion_of(&node_from(FADE));
    assert_eq!(motion.len(), 2);
    let MotionSummary::Simple(simple) = motion[0] else {
        panic!("first row is simple");
    };
    assert_eq!(simple.property, MotionProperty::Opacity);
    assert_eq!(simple.trigger, MotionTrigger::Mount);
    assert_eq!(
        motion[1],
        MotionSummary::Custom {
            trigger: MotionTrigger::InView
        }
    );
    assert!(motion_of(&node_from(r#"{"type":"rectangle","id":"r1"}"#)).is_empty());
}

fn summary_of(json: &str) -> InteractionSummary {
    crate::widgets::property_panel_interactions::interactions_of(&node_from(json), false)
}

#[test]
fn a_simple_row_is_five_lines_and_a_custom_row_two() {
    let summary = summary_of(FADE);
    let header_only = SECTION_HEADER_HEIGHT + 6.0 + SECTION_GAP;
    assert_eq!(
        motion_section_height(&summary) - header_only,
        // FADE: one simple animation (5 rows) and one custom (2).
        (5.0 + 2.0) * (INPUT_HEIGHT + 6.0)
    );
    assert_eq!(motion_section_height(&InteractionSummary::default()), 0.0);
}

#[test]
fn a_transition_adds_three_rows_above_the_animations() {
    let summary = summary_of(r#"{"type":"rectangle","id":"r1","transition":{"durationMs":250}}"#);
    let header_only = SECTION_HEADER_HEIGHT + 6.0 + SECTION_GAP;
    assert_eq!(
        motion_section_height(&summary) - header_only,
        3.0 * (INPUT_HEIGHT + 6.0)
    );
    let mut actions = Vec::new();
    push_motion_action_rects(&mut actions, &summary, 0.0, 0.0, 272.0);
    assert!(actions.iter().any(|(a, _)| matches!(
        a,
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::TransitionEasing)
    )));
    assert!(actions
        .iter()
        .any(|(a, _)| matches!(a, PropertyPanelAction::RemoveTransition)));
    let inputs = motion_input_rects(&summary, 0.0, 0.0, 272.0);
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].0, PropertyFocus::TransitionDuration);
    // Animations start below the three transition rows.
    let with_both = summary_of(
        r#"{"type":"rectangle","id":"r1","transition":{"durationMs":250},"animations":[
        {"trigger":"mount","durationMs":300,
         "keyframes":[{"offset":0,"values":{"opacity":0}},{"offset":1,"values":{"opacity":1}}]}]}"#,
    );
    let inputs = motion_input_rects(&with_both, 0.0, 0.0, 272.0);
    assert_eq!(inputs.len(), 5);
    assert!(inputs[1].1.origin.y > inputs[0].1.origin.y + INPUT_HEIGHT);
}

#[test]
fn rects_cover_add_dropdown_triggers_remove_and_four_inputs() {
    let summary = summary_of(FADE);
    let mut actions = Vec::new();
    push_motion_action_rects(&mut actions, &summary, 0.0, 0.0, 272.0);
    assert!(matches!(actions[0].0, PropertyPanelAction::AddAnimation));
    let dropdowns = actions
        .iter()
        .filter(|(a, _)| {
            matches!(
                a,
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::MotionProperty(
                    _
                ))
            )
        })
        .count();
    // The custom row has no property dropdown.
    assert_eq!(dropdowns, 1);
    // Every row has a trigger dropdown (#155: four triggers).
    let triggers = actions
        .iter()
        .filter(|(a, _)| {
            matches!(
                a,
                PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::MotionTrigger(_))
            )
        })
        .count();
    assert_eq!(triggers, 2);
    let rows = crate::widgets::property_panel_interactions::interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::MotionTrigger(1),
        &[],
        &summary,
    );
    assert_eq!(
        rows.into_iter().map(|(a, _)| a).collect::<Vec<_>>(),
        MOTION_TRIGGERS
            .into_iter()
            .map(|trigger| PropertyPanelAction::SetAnimationTrigger { index: 1, trigger })
            .collect::<Vec<_>>()
    );
    let inputs = motion_input_rects(&summary, 0.0, 0.0, 272.0);
    assert_eq!(inputs.len(), 4);
    // No input overlaps an action rect.
    for (_, input) in &inputs {
        for (_, action) in &actions {
            let overlap_x = input.origin.x < action.origin.x + action.size.x
                && action.origin.x < input.origin.x + input.size.x;
            let overlap_y = input.origin.y < action.origin.y + action.size.y
                && action.origin.y < input.origin.y + input.size.y;
            assert!(!(overlap_x && overlap_y), "{input:?} overlaps {action:?}");
        }
    }
}

#[test]
fn field_values_format_like_the_panel_numbers() {
    let MotionSummary::Simple(simple) = motion_of(&node_from(FADE))[0] else {
        panic!("simple");
    };
    assert_eq!(field_value(&simple, MotionField::From), "0");
    assert_eq!(field_value(&simple, MotionField::To), "1");
    assert_eq!(field_value(&simple, MotionField::DurationMs), "300");
    assert_eq!(field_value(&simple, MotionField::DelayMs), "0");
}

/// #190: a Fill tween reads as a simple row whose From and To are hex
/// inputs on a row each.
#[test]
fn a_fill_tween_has_hex_inputs() {
    let summary = summary_of(
        r##"{"type":"rectangle","id":"r1","animations":[
        {"trigger":"hover","durationMs":150,
         "keyframes":[{"offset":0,"values":{"fill":"#3B5BFD"}},{"offset":1,"values":{"fill":"#2F49CA"}}]}]}"##,
    );
    let MotionSummary::Simple(simple) = summary.motion[0] else {
        panic!("a fill tween is simple");
    };
    assert_eq!(simple.property, MotionProperty::Fill);
    assert_eq!(field_value(&simple, MotionField::From), "#3B5BFD");
    let header_only = SECTION_HEADER_HEIGHT + 6.0 + SECTION_GAP;
    assert_eq!(
        motion_section_height(&summary) - header_only,
        6.0 * (INPUT_HEIGHT + 6.0)
    );
    let inputs = motion_input_rects(&summary, 0.0, 0.0, 272.0);
    let focuses: Vec<_> = inputs.iter().map(|(focus, _)| *focus).collect();
    assert_eq!(
        focuses,
        [
            PropertyFocus::MotionColor {
                index: 0,
                field: MotionField::From
            },
            PropertyFocus::MotionColor {
                index: 0,
                field: MotionField::To
            },
            PropertyFocus::MotionValue {
                index: 0,
                field: MotionField::DurationMs
            },
            PropertyFocus::MotionValue {
                index: 0,
                field: MotionField::DelayMs
            },
        ]
    );
    assert!(PropertyFocus::MotionColor {
        index: 0,
        field: MotionField::From
    }
    .is_hex());
}

/// Width of `text` at 12 px as the chrome face paints it, by script:
/// Latin ~0.55 em, Cyrillic / Greek / Thai / Devanagari ~0.62 em (combining
/// marks take no advance), CJK and Hangul a full em. Close to the real
/// Manrope / system fallback advances, a little generous.
fn chrome_width(text: &str) -> f32 {
    text.chars()
        .map(|c| {
            let code = c as u32;
            let combining = matches!(
                code,
                0x0300..=0x036F
                    | 0x0E31
                    | 0x0E34..=0x0E3A
                    | 0x0E47..=0x0E4E
                    | 0x0900..=0x0903
                    | 0x093A..=0x094F
                    | 0x0951..=0x0957
                    | 0x0962..=0x0963
            );
            if combining {
                0.0
            } else if code < 0x0250 {
                0.55
            } else if code < 0x2000 {
                0.62
            } else {
                1.0
            }
        })
        .sum::<f32>()
        * 12.0
}

/// The widest label each localized control has room for, in a panel of
/// `width` — `(property / custom label, trigger, transition label,
/// easing)`: the rects paint uses, minus what `paint_fitted_dropdown` /
/// `paint_plain_row` keep for padding and the chevron.
fn label_budgets(width: f32) -> (f32, f32, f32, f32) {
    use crate::widgets::property_panel_interactions::{DROPDOWN_CHROME_W, PLAIN_ROW_INSET_W};
    let (body, _) = head_parts(0.0, 0.0, width);
    let full = full_row(0.0, 0.0, width);
    (
        body.size.x - DROPDOWN_CHROME_W,
        full.size.x - DROPDOWN_CHROME_W,
        body.size.x - PLAIN_ROW_INSET_W,
        full.size.x - DROPDOWN_CHROME_W,
    )
}

/// #190: every label the Motion section paints fits its control in every
/// locale at the panel's width — no "Yüklen…", "Deckkr…" or "Im Sic…".
#[test]
fn every_motion_label_fits_in_every_locale() {
    let width = crate::widgets::property_panel::PROPERTY_PANEL_WIDTH;
    let (property_room, trigger_room, label_room, easing_room) = label_budgets(width);
    // Prefixed inputs: 10 px before the prefix, 8 px gap, 8 px after the
    // value, and room for a four-digit value.
    let value_w = chrome_width("1000") + 26.0;
    let full_room = full_row(0.0, 0.0, width).size.x - value_w;
    let half_room = value_pair(0.0, 0.0, width)[0].size.x - value_w;
    for locale in de_editor_core::Locale::ALL {
        let mut checks: Vec<(&str, f32)> = Vec::new();
        for property in MotionProperty::ALL {
            checks.push((property_label(locale, property), property_room));
        }
        checks.push((
            translate(locale, "motion.custom", "Custom animation"),
            label_room,
        ));
        checks.push((
            translate(locale, "interactions.transition", "Transition"),
            label_room,
        ));
        for trigger in MOTION_TRIGGERS {
            checks.push((trigger_label(locale, trigger), trigger_room));
        }
        for easing in MotionEasing::ALL.into_iter().map(Some).chain([None]) {
            checks.push((easing_label(locale, easing), easing_room));
        }
        for field in [MotionField::From, MotionField::To] {
            checks.push((field_prefix(locale, field), half_room));
        }
        for field in [MotionField::DurationMs, MotionField::DelayMs] {
            checks.push((field_prefix(locale, field), full_room));
        }
        // The Interactions Slide | Fade cells share one full row; each
        // keeps 6 px either side (11 px text, measured here at 12 px).
        let cell_room = full_row(0.0, 0.0, width).size.x / 2.0 - 12.0;
        checks.push((translate(locale, "interactions.slide", "Slide"), cell_room));
        checks.push((translate(locale, "interactions.fade", "Fade"), cell_room));
        for (label, room) in checks {
            assert!(
                chrome_width(label) <= room,
                "{locale:?}: {label:?} is {:.0} px, room {room:.0} px",
                chrome_width(label)
            );
        }
    }
}
