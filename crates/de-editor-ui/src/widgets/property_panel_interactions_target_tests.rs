//! Interaction rows name their target screen and jump to it (#371).

use super::*;
use de_editor_core::{EditorState, NodeId};

const TAP: InteractionTrigger = InteractionTrigger::Tap;

fn navigate_summary() -> InteractionSummary {
    InteractionSummary {
        triggers: vec![TriggerSummary {
            trigger: TAP,
            actions: vec![TapActionSummary::Navigate {
                verb: NavigateVerb::Replace,
                path: "/pricing".to_string(),
            }],
        }],
        screens: vec![ScreenRef {
            route: "/pricing".into(),
            name: "Pricing".into(),
            id: "pricing".into(),
        }],
        ..InteractionSummary::default()
    }
}

#[test]
fn a_navigate_row_names_its_screen_and_offers_a_jump_button() {
    let summary = navigate_summary();
    assert_eq!(summary.target_of(TAP).as_deref(), Some("pricing"));
    let label = action_label(
        de_editor_core::Locale::EnUs,
        &summary,
        TAP,
        &summary.actions(TAP)[0],
    );
    assert!(label.ends_with("Pricing"), "{label}");
    let mut out = Vec::new();
    push_interaction_action_rects(&mut out, &summary, 0.0, 0.0, 280.0);
    let menu = out
        .iter()
        .find(|(a, _)| {
            *a == PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::Trigger(TAP))
        })
        .expect("menu row")
        .1;
    let jump = out
        .iter()
        .find(|(a, _)| *a == PropertyPanelAction::GoToInteractionTarget(TAP))
        .expect("jump button")
        .1;
    assert!(
        menu.origin.x + menu.size.x < jump.origin.x,
        "side by side, no overlap"
    );
    assert_eq!(menu.origin.y, jump.origin.y);
}

#[test]
fn an_unknown_route_has_no_jump_button() {
    let mut summary = navigate_summary();
    summary.screens.clear();
    let mut out = Vec::new();
    push_interaction_action_rects(&mut out, &summary, 0.0, 0.0, 280.0);
    assert!(!out
        .iter()
        .any(|(a, _)| matches!(a, PropertyPanelAction::GoToInteractionTarget(_))));
}

#[test]
fn the_jump_button_selects_the_screen_and_asks_to_frame_it() {
    let doc = serde_json::from_str(
        r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","screen":"/","width":400,"height":800,"children":[
          {"type":"frame","id":"go","width":80,"height":40,
           "events":{"onTap":[{"push":"\"/pricing\""}]}}]},
        {"type":"frame","id":"pricing","name":"Pricing","screen":"/pricing","x":600,
         "width":400,"height":800}]}"#,
    )
    .expect("doc");
    let mut state = EditorState::from_document(doc);
    state.set_single_selection(NodeId::new("go"));
    let screens = document_screens(&state);
    assert_eq!(screens.len(), 2);
    crate::widgets::property_panel_dispatch_interactions::apply_interaction_action(
        &mut state,
        &PropertyPanelAction::GoToInteractionTarget(TAP),
    );
    assert_eq!(state.selection.anchor.as_str(), "pricing");
    assert_eq!(
        state.editor_ui.prototype.pending_frame,
        Some(NodeId::new("pricing"))
    );
}
