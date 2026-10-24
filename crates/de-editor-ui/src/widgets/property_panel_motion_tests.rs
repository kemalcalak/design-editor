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
fn a_simple_row_is_three_lines_and_a_custom_row_one() {
    let summary = summary_of(FADE);
    let header_only = SECTION_HEADER_HEIGHT + 6.0 + SECTION_GAP;
    assert_eq!(
        motion_section_height(&summary) - header_only,
        4.0 * (INPUT_HEIGHT + 6.0)
    );
    assert_eq!(motion_section_height(&InteractionSummary::default()), 0.0);
}

#[test]
fn a_transition_adds_two_rows_above_the_animations() {
    let summary = summary_of(r#"{"type":"rectangle","id":"r1","transition":{"durationMs":250}}"#);
    let header_only = SECTION_HEADER_HEIGHT + 6.0 + SECTION_GAP;
    assert_eq!(
        motion_section_height(&summary) - header_only,
        2.0 * (INPUT_HEIGHT + 6.0)
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
    // Animations start below the two transition rows.
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
        .filter(|(a, _)| matches!(a, PropertyPanelAction::ToggleInteractionMenu(_)))
        .count();
    // The custom row has no property dropdown.
    assert_eq!(dropdowns, 1);
    let triggers = actions
        .iter()
        .filter(|(a, _)| matches!(a, PropertyPanelAction::SetAnimationTrigger { .. }))
        .count();
    assert_eq!(triggers, 4);
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
