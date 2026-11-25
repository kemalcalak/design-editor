use super::*;
use crate::interaction_edit::{navigate_action, pop_action, NavigateVerb};
use crate::interaction_fixture::{state, APP_PAGE, KIT_PAGE, SITE_PAGE};
use crate::pen_node_ext::PenNodeExt;
use crate::EditorCommand;

fn set(
    node: &str,
    trigger: InteractionTrigger,
    actions: Option<ActionList>,
    page: &str,
) -> EditorCommand {
    EditorCommand::Interaction(InteractionCommand::Set {
        node_id: NodeId::new(node),
        trigger,
        actions,
        page_id: Some(page.to_owned()),
    })
}

fn authored<'a>(state: &'a EditorState, id: &str) -> &'a PenNode {
    crate::instance_anchor::find_authored(&state.doc, id).expect("node")
}

fn descendants(state: &EditorState, instance: &str) -> serde_json::Value {
    serde_json::to_value(authored(state, instance)).expect("json")["descendants"].clone()
}

#[test]
fn setting_one_trigger_keeps_the_others() {
    let mut s = state();
    let back = vec![pop_action()];
    assert!(s.apply(set(
        "hero-link",
        InteractionTrigger::Tap,
        Some(back.clone()),
        SITE_PAGE
    )));
    let events = authored(&s, "hero-link").events().expect("events").clone();
    assert_eq!(events.on_tap, Some(back.clone()));
    assert!(
        events.on_hover_enter.is_some(),
        "the hover handler survives"
    );
    // The same write again changes nothing.
    assert!(!s.apply(set(
        "hero-link",
        InteractionTrigger::Tap,
        Some(back),
        SITE_PAGE
    )));
}

#[test]
fn removing_the_last_trigger_drops_the_events_block() {
    let mut s = state();
    assert!(s.apply(set("old-link", InteractionTrigger::Tap, None, SITE_PAGE)));
    assert!(authored(&s, "old-link").events().is_none());
}

#[test]
fn a_write_on_another_page_leaves_the_active_page_alone() {
    let mut s = state();
    let active = s.ui.active_page_index;
    let go = vec![navigate_action(NavigateVerb::Push, "/")];
    assert!(s.apply(set(
        "to-site",
        InteractionTrigger::LongPress,
        Some(go.clone()),
        APP_PAGE
    )));
    assert_eq!(s.ui.active_page_index, active);
    let events = authored(&s, "to-site").events().expect("events");
    assert_eq!(events.on_long_press, Some(go));
    assert!(events.on_tap.is_some());
}

#[test]
fn an_instance_child_write_lands_in_the_instance_descendants() {
    let mut s = state();
    let go = vec![navigate_action(NavigateVerb::Replace, "/about")];
    assert!(s.apply(set(
        "cta/btn-label",
        InteractionTrigger::Tap,
        Some(go.clone()),
        SITE_PAGE
    )));
    let entry = &descendants(&s, "cta")["btn-label"]["events"]["onTap"];
    assert_eq!(entry, &serde_json::to_value(&go).expect("json"));
    // The master is untouched.
    assert!(authored(&s, "btn-label").events().is_none());
    // The canvas anchor addresses the same node.
    let back = vec![pop_action()];
    assert!(s.apply(set(
        "cta__btn-label",
        InteractionTrigger::Hover,
        Some(back),
        SITE_PAGE
    )));
    let entry = &descendants(&s, "cta")["btn-label"]["events"];
    assert!(entry.get("onHoverEnter").is_some());
    assert!(entry.get("onTap").is_some(), "the tap override survives");
}

#[test]
fn an_instance_root_write_overrides_the_master_handlers_for_that_instance() {
    let mut s = state();
    let back = vec![pop_action()];
    assert!(s.apply(set("cta", InteractionTrigger::Hover, Some(back), SITE_PAGE)));
    let top = &descendants(&s, "cta")["btn"]["events"];
    assert!(top.get("onHoverEnter").is_some());
    // The master's own tap rides along in the instance's whole block.
    assert!(top.get("onTap").is_some());
    let master = authored(&s, "btn").events().expect("master events");
    assert!(master.on_hover_enter.is_none());
    assert!(matches!(authored(&s, "cta"), PenNode::Ref(_)));
}

#[test]
fn a_master_write_needs_its_own_page() {
    let mut s = state();
    let back = vec![pop_action()];
    assert!(!s.apply(set(
        "btn",
        InteractionTrigger::Hover,
        Some(back.clone()),
        SITE_PAGE
    )));
    assert!(s.apply(set("btn", InteractionTrigger::Hover, Some(back), KIT_PAGE)));
}

#[test]
fn a_panel_write_on_an_instance_child_lands_in_descendants() {
    // The host wraps every property-panel action in the instance-write
    // scope; the Interactions rows ride it like any other row.
    let mut s = state();
    s.set_single_selection(NodeId::new("cta__btn-label"));
    let scope = s.begin_instance_write_for_anchor().expect("instance scope");
    let edit = crate::interaction_edit::InteractionActionEdit::Navigate {
        path: "/about".into(),
    };
    assert!(s.set_selected_interaction_action(InteractionTrigger::Tap, &edit));
    assert!(s.finish_instance_write(scope));
    let entry = &descendants(&s, "cta")["btn-label"]["events"]["onTap"][0];
    assert_eq!(entry["replace"], serde_json::json!("\"/about\""));
    assert!(authored(&s, "btn-label").events().is_none());
}

#[test]
fn an_interaction_write_is_one_undo_step_through_a_batch() {
    let mut s = state();
    let before = s.snapshot_for_history();
    let command = EditorCommand::Batch {
        commands: vec![
            set("hero-link", InteractionTrigger::Tap, None, SITE_PAGE),
            set("old-link", InteractionTrigger::Tap, None, SITE_PAGE),
        ],
    };
    assert!(s.apply(command));
    s.history_push_past(before);
    assert!(s.undo());
    assert!(authored(&s, "hero-link")
        .events()
        .and_then(|e| e.on_tap.as_ref())
        .is_some());
    assert!(authored(&s, "old-link").events().is_some());
}
