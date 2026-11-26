//! The Interactions panel authors overlays (#372): pick the action, the
//! frame, then the options.

use super::*;
use crate::widgets::property_panel_dispatch_interactions::apply_interaction_action;
use crate::widgets::property_panel_interactions::{
    interaction_menu_rows, interactions_of, interactions_section_height,
    push_interaction_action_rects,
};
use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::{EditorState, NodeId};

const TAP: InteractionTrigger = InteractionTrigger::Tap;

fn state() -> EditorState {
    let doc = serde_json::from_str(
        r#"{"version":"1.0","pages":[
          {"id":"p1","name":"Site","children":[
            {"type":"frame","id":"home","name":"Home","width":400,"height":800,"children":[
              {"type":"frame","id":"go","width":80,"height":40}]},
            {"type":"frame","id":"menu","name":"Menu","x":600,"width":200,"height":300}]},
          {"id":"p2","name":"Overlays","children":[
            {"type":"frame","id":"sheet","name":"Sheet","width":400,"height":300}]}]}"#,
    )
    .expect("doc");
    let mut state = EditorState::from_document(doc);
    state.set_single_selection(NodeId::new("go"));
    state
}

fn summary(state: &EditorState) -> InteractionSummary {
    let mut summary = interactions_of(state.selected_node().expect("selection"), false);
    summary.overlay_targets =
        de_editor_core::overlay_edit::overlay_targets(&state.doc, 0, "go");
    summary
}

#[test]
fn the_trigger_menu_offers_overlays_and_a_frame_list() {
    let state = state();
    let summary = summary(&state);
    let rows = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::Trigger(TAP),
        &[],
        &summary,
    );
    let labels: Vec<&str> = rows.iter().map(|(_, l)| l.as_str()).collect();
    assert!(labels.contains(&"Open overlay…"), "{labels:?}");
    assert!(labels.contains(&"Swap overlay…"), "{labels:?}");
    assert!(labels.contains(&"Close overlay"), "{labels:?}");
    let frames = interaction_menu_rows(
        de_editor_core::Locale::EnUs,
        InteractionMenuTarget::OverlayTarget {
            trigger: TAP,
            swap: false,
        },
        &[],
        &summary,
    );
    let labels: Vec<&str> = frames.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(labels, ["Menu", "Sheet · Overlays"]);
}

#[test]
fn picking_a_frame_writes_the_overlay_and_shows_its_option_rows() {
    let mut state = state();
    apply_interaction_action(
        &mut state,
        &PropertyPanelAction::PickOverlayTarget {
            trigger: TAP,
            swap: false,
        },
    );
    assert_eq!(
        state.editor_ui.interaction_menu_target,
        InteractionMenuTarget::OverlayTarget {
            trigger: TAP,
            swap: false
        }
    );
    apply_interaction_action(
        &mut state,
        &PropertyPanelAction::SetInteractionOverlay {
            trigger: TAP,
            target: "menu".into(),
            swap: false,
        },
    );
    let before = summary(&state);
    assert!(matches!(before.overlay(TAP), Some(OverlayAction::Open(_))));
    let height = interactions_section_height(&before);

    // The position grid and the Dim cell write their options.
    let mut out = Vec::new();
    push_interaction_action_rects(&mut out, &before, 0.0, 0.0, 280.0);
    let has = |edit: OverlayOptionEdit| {
        out.iter().any(|(a, _)| {
            *a == PropertyPanelAction::SetOverlayOption { trigger: TAP, edit: edit.clone() }
        })
    };
    assert!(has(OverlayOptionEdit::Position(OverlayPosition::BottomCenter)));
    assert!(has(OverlayOptionEdit::Position(OverlayPosition::Manual)));
    assert!(has(OverlayOptionEdit::CloseOnOutsideClick(false)));
    assert!(has(OverlayOptionEdit::Background(true)));
    assert!(has(OverlayOptionEdit::Animation(OverlayAnimationKind::MoveIn)));

    apply_interaction_action(
        &mut state,
        &PropertyPanelAction::SetOverlayOption {
            trigger: TAP,
            edit: OverlayOptionEdit::Background(true),
        },
    );
    apply_interaction_action(
        &mut state,
        &PropertyPanelAction::SetOverlayOption {
            trigger: TAP,
            edit: OverlayOptionEdit::Animation(OverlayAnimationKind::MoveIn),
        },
    );
    let after = summary(&state);
    assert_eq!(after.overlay_background(TAP).as_deref(), Some("#00000066"));
    // The hex row and the edge row appear.
    assert!(interactions_section_height(&after) > height);
    let label = crate::widgets::property_panel_interactions::action_label(
        de_editor_core::Locale::EnUs,
        &after,
        TAP,
        &after.actions(TAP)[0],
    );
    assert_eq!(label, "Tap → Open overlay Menu");
    assert_eq!(after.target_of(TAP).as_deref(), Some("menu"));
    let events = state.selected_node().and_then(|n| n.events().cloned());
    assert!(events.and_then(|e| e.on_tap).is_some());
}
