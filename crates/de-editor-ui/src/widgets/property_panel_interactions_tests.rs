//! Tests for the Interactions section's data parsing + geometry +
//! menu helpers.

use super::*;
use crate::widgets::property_panel_visibility::SectionCapabilities;
use jian_ops_schema::node::PenNode;

fn node_from(json: &str) -> PenNode {
    serde_json::from_str(json).expect("test fixture parses")
}

const TAP: InteractionTrigger = InteractionTrigger::Tap;

// ── Section capability filtering ────────────────────────────────────

#[test]
fn interactions_capability_is_on_for_every_single_select_kind() {
    use crate::layout_scene::NodeKind as K;
    for kind in [
        K::Frame,
        K::Group,
        K::Rect,
        K::Ellipse,
        K::Polygon,
        K::Line,
        K::Path,
        K::Text,
        K::Other("image".to_string()),
        K::Other("icon_font".to_string()),
        K::Other("ref".to_string()),
        K::Other("widget".to_string()),
    ] {
        assert!(
            SectionCapabilities::for_kind(&kind).interactions,
            "{kind:?} should show the Interactions section"
        );
    }
}

#[test]
fn interactions_capability_is_off_for_multi_select() {
    assert!(!SectionCapabilities::for_multi().interactions);
}

// ── events parsing ───────────────────────────────────────────────────

#[test]
fn interactions_of_parses_replace_navigate() {
    let node =
        node_from(r#"{"type":"frame","id":"f1","events":{"onTap":[{"replace":"\"/profile\""}]}}"#);
    let summary = interactions_of(&node, false);
    assert_eq!(summary.actions(TAP).len(), 1);
    assert_eq!(
        summary.actions(TAP)[0],
        TapActionSummary::Navigate {
            verb: NavigateVerb::Replace,
            path: "/profile".to_string(),
        }
    );
}

#[test]
fn interactions_of_parses_push_navigate() {
    let node =
        node_from(r#"{"type":"frame","id":"f1","events":{"onTap":[{"push":"\"/detail\""}]}}"#);
    let summary = interactions_of(&node, false);
    assert_eq!(
        summary.actions(TAP)[0],
        TapActionSummary::Navigate {
            verb: NavigateVerb::Push,
            path: "/detail".to_string(),
        }
    );
}

#[test]
fn interactions_of_parses_pop() {
    let node = node_from(r#"{"type":"frame","id":"f1","events":{"onTap":[{"pop":null}]}}"#);
    let summary = interactions_of(&node, false);
    assert_eq!(summary.actions(TAP)[0], TapActionSummary::Pop);
}

#[test]
fn interactions_of_parses_unknown_key_as_other() {
    let node =
        node_from(r#"{"type":"frame","id":"f1","events":{"onTap":[{"set":{"$state.x":"1"}}]}}"#);
    let summary = interactions_of(&node, false);
    assert_eq!(
        summary.actions(TAP)[0],
        TapActionSummary::Other("set".to_string())
    );
}

#[test]
fn interactions_of_carries_multiple_actions_in_order() {
    let node = node_from(
        r#"{"type":"frame","id":"f1","events":{"onTap":[{"pop":null},{"replace":"\"/a\""}]}}"#,
    );
    let summary = interactions_of(&node, false);
    assert_eq!(summary.actions(TAP).len(), 2);
    assert_eq!(summary.actions(TAP)[0], TapActionSummary::Pop);
    assert_eq!(
        summary.actions(TAP)[1],
        TapActionSummary::Navigate {
            verb: NavigateVerb::Replace,
            path: "/a".to_string(),
        }
    );
}

#[test]
fn interactions_of_reports_no_actions_for_bare_node() {
    let node = node_from(r#"{"type":"frame","id":"f1"}"#);
    let summary = interactions_of(&node, false);
    assert!(summary.triggers.is_empty());
    assert!(summary.motion.is_empty());
    assert_eq!(summary.screen, None);
}

#[test]
fn interactions_of_lists_each_authored_trigger_in_order() {
    let node = node_from(
        r#"{"type":"frame","id":"f1","events":{
            "onLongPress":[{"pop":null}],
            "onHoverEnter":[{"push":"\"/a\""}],
            "onSwipe":[{"pop":null}]}}"#,
    );
    let summary = interactions_of(&node, false);
    let order: Vec<_> = summary.triggers.iter().map(|t| t.trigger).collect();
    assert_eq!(
        order,
        vec![InteractionTrigger::Hover, InteractionTrigger::LongPress]
    );
    assert_eq!(summary.unauthored_triggers(), vec![TAP]);
}

// ── screen marker, gated by `is_top_level` ──────────────────────────

#[test]
fn interactions_of_shows_screen_only_when_top_level() {
    let node = node_from(r#"{"type":"frame","id":"f1","screen":"/home"}"#);
    assert_eq!(
        interactions_of(&node, true).screen,
        Some("/home".to_string())
    );
    assert_eq!(interactions_of(&node, false).screen, None);
}

#[test]
fn interactions_of_never_shows_screen_for_non_frame() {
    // `screen` isn't a field on Rectangle at all — even `is_top_level`
    // true must not fabricate one.
    let node = node_from(r#"{"type":"rectangle","id":"r1"}"#);
    assert_eq!(interactions_of(&node, true).screen, None);
}

#[test]
fn the_core_navigate_writer_round_trips_through_interactions_of() {
    let action = de_editor_core::interaction_edit::navigate_action(
        NavigateVerb::Replace,
        "/settings/account",
    );
    let json = serde_json::json!({
        "type": "frame", "id": "f1", "events": { "onTap": [action] }
    });
    let node: PenNode = serde_json::from_value(json).expect("node parses");
    assert_eq!(
        interactions_of(&node, false).actions(TAP)[0],
        TapActionSummary::Navigate {
            verb: NavigateVerb::Replace,
            path: "/settings/account".to_string(),
        }
    );
}

// ── Section height + action-rect row math stay aligned ──────────────

fn summary_with(trigger: InteractionTrigger, actions: Vec<TapActionSummary>) -> InteractionSummary {
    InteractionSummary {
        triggers: vec![TriggerSummary { trigger, actions }],
        ..InteractionSummary::default()
    }
}

/// The Interactions rects alone (the Motion section's are dropped).
fn interaction_rects(summary: &InteractionSummary) -> Vec<(PropertyPanelAction, Rect)> {
    let mut out = Vec::new();
    push_interaction_action_rects(&mut out, summary, 0.0, 0.0, 280.0);
    out.retain(|(a, _)| {
        !matches!(
            a,
            PropertyPanelAction::AddAnimation
                | PropertyPanelAction::ToggleInteractionMenu(
                    InteractionMenuTarget::MotionProperty(_)
                )
        )
    });
    out
}

#[test]
fn empty_state_has_only_the_header_add() {
    let out = interaction_rects(&InteractionSummary::default());
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].0,
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::AddTrigger)
    );
}

#[test]
fn a_back_action_is_one_toggle_row() {
    let out = interaction_rects(&summary_with(
        InteractionTrigger::Hover,
        vec![TapActionSummary::Pop],
    ));
    assert_eq!(out.len(), 2);
    assert_eq!(
        out[1].0,
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::Trigger(
            InteractionTrigger::Hover
        ))
    );
}

#[test]
fn a_navigate_adds_a_slide_fade_transition_row() {
    let summary = summary_with(
        TAP,
        vec![TapActionSummary::Navigate {
            verb: NavigateVerb::Push,
            path: "/a".to_string(),
        }],
    );
    let verbs: Vec<_> = interaction_rects(&summary)
        .iter()
        .filter_map(|(a, _)| match a {
            PropertyPanelAction::SetInteractionTransition { verb, .. } => Some(*verb),
            _ => None,
        })
        .collect();
    assert_eq!(verbs, vec![NavigateVerb::Push, NavigateVerb::Replace]);
    let back = summary_with(TAP, vec![TapActionSummary::Pop]);
    assert!(interactions_section_height(&summary) > interactions_section_height(&back));
}

#[test]
fn multi_action_has_only_a_remove_all_row() {
    let summary = summary_with(
        TAP,
        vec![
            TapActionSummary::Pop,
            TapActionSummary::Navigate {
                verb: NavigateVerb::Replace,
                path: "/a".to_string(),
            },
        ],
    );
    let out = interaction_rects(&summary);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1].0, PropertyPanelAction::RemoveInteraction(TAP));
}

#[test]
fn every_trigger_authored_keeps_the_header_add_for_animations() {
    let summary = InteractionSummary {
        triggers: InteractionTrigger::ALL
            .into_iter()
            .map(|trigger| TriggerSummary {
                trigger,
                actions: vec![TapActionSummary::Pop],
            })
            .collect(),
        ..InteractionSummary::default()
    };
    assert!(interaction_rects(&summary).iter().any(|(a, _)| matches!(
        a,
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::AddTrigger)
    )));
    let rows = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::AddTrigger,
        &[],
        &summary,
    );
    assert_eq!(
        rows.into_iter().map(|(a, _)| a).collect::<Vec<_>>(),
        vec![PropertyPanelAction::AddAnimation]
    );
}

#[test]
fn screen_row_adds_height_but_no_click_rect() {
    let with_screen = InteractionSummary {
        screen: Some("/home".to_string()),
        ..InteractionSummary::default()
    };
    let without_screen = InteractionSummary::default();
    assert!(
        interactions_section_height(&with_screen) > interactions_section_height(&without_screen)
    );
    assert_eq!(interaction_rects(&with_screen).len(), 1);
}

#[test]
fn an_empty_node_has_no_motion_section() {
    let summary = InteractionSummary::default();
    let mut out = Vec::new();
    push_interaction_action_rects(&mut out, &summary, 0.0, 0.0, 280.0);
    assert!(out
        .iter()
        .all(|(a, _)| !matches!(a, PropertyPanelAction::AddAnimation)));
    assert_eq!(
        crate::widgets::property_panel_motion::motion_section_height(&summary.motion),
        0.0
    );
}

#[test]
fn the_motion_section_starts_where_interactions_end() {
    let node = node_from(
        r#"{"type":"rectangle","id":"r1","animations":[{"trigger":"mount","durationMs":300,
            "keyframes":{"from":{"opacity":0},"to":{"opacity":1}}}]}"#,
    );
    let summary = interactions_of(&node, false);
    let mut out = Vec::new();
    push_interaction_action_rects(&mut out, &summary, 0.0, 0.0, 280.0);
    let add = out
        .iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::AddAnimation))
        .map(|(_, r)| *r)
        .expect("motion add");
    let expected = section_add_target(0.0, interactions_section_height(&summary), 280.0);
    assert_eq!(add, expected);
}

// ── Popover rows ─────────────────────────────────────────────────────

#[test]
fn trigger_menu_lists_every_screen_path_then_back_then_remove() {
    let summary = summary_with(TAP, vec![TapActionSummary::Pop]);
    let rows = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::Trigger(TAP),
        &["/".to_string(), "/profile".to_string()],
        &summary,
    );
    let actions: Vec<_> = rows.into_iter().map(|(a, _)| a).collect();
    assert_eq!(
        actions,
        vec![
            PropertyPanelAction::SetInteractionNavigate {
                trigger: TAP,
                path: "/".to_string()
            },
            PropertyPanelAction::SetInteractionNavigate {
                trigger: TAP,
                path: "/profile".to_string()
            },
            PropertyPanelAction::SetInteractionPop(TAP),
            PropertyPanelAction::RemoveInteraction(TAP),
        ]
    );
}

#[test]
fn an_unauthored_trigger_menu_has_no_remove() {
    let rows = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::Trigger(InteractionTrigger::LongPress),
        &[],
        &InteractionSummary::default(),
    );
    assert_eq!(
        rows.into_iter().map(|(a, _)| a).collect::<Vec<_>>(),
        vec![PropertyPanelAction::SetInteractionPop(
            InteractionTrigger::LongPress
        )]
    );
}

#[test]
fn add_menu_lists_only_unauthored_triggers() {
    let summary = summary_with(InteractionTrigger::Hover, vec![TapActionSummary::Pop]);
    let rows = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::AddTrigger,
        &[],
        &summary,
    );
    assert_eq!(
        rows.into_iter().map(|(a, _)| a).collect::<Vec<_>>(),
        vec![
            PropertyPanelAction::PickInteractionTrigger(TAP),
            PropertyPanelAction::PickInteractionTrigger(InteractionTrigger::LongPress),
            PropertyPanelAction::AddAnimation,
        ]
    );
}

#[test]
fn motion_property_menu_lists_every_property() {
    let rows = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::MotionProperty(2),
        &[],
        &InteractionSummary::default(),
    );
    assert_eq!(
        rows.len(),
        de_editor_core::motion_edit::MotionProperty::ALL.len()
    );
    assert!(matches!(
        rows[0].0,
        PropertyPanelAction::SetAnimationProperty { index: 2, .. }
    ));
}

#[test]
fn menu_row_rects_are_stacked() {
    let rows = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::Trigger(TAP),
        &["/a".to_string()],
        &InteractionSummary::default(),
    );
    let anchor = Rect {
        origin: Point2D::new(20.0, 100.0),
        size: Point2D::new(240.0, 30.0),
    };
    let menu = interaction_menu_rect(anchor, rows.len());
    let rects = interaction_menu_row_rects(menu, &rows);
    assert_eq!(rects.len(), rows.len());
    for pair in rects.windows(2) {
        assert!(pair[1].1.origin.y >= pair[0].1.origin.y + pair[0].1.size.y);
    }
}

#[test]
fn a_narrow_dropdown_anchor_opens_the_popover_rightward() {
    // The Motion property dropdown sits at the panel's left edge.
    let dropdown = Rect::xywh(16.0, 300.0, 92.0, 28.0);
    let menu = interaction_menu_rect(dropdown, 6);
    assert_eq!(menu.origin.x, dropdown.origin.x);
    // The header "+" sits at the right edge: the popover hangs left.
    let add = section_add_target(0.0, 0.0, 272.0);
    let menu = interaction_menu_rect(add, 3);
    assert_eq!(menu.origin.x + menu.size.x, add.origin.x + add.size.x);
    assert!(menu.origin.x >= 0.0);
}
