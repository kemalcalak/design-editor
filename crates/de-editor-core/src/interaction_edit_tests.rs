use super::*;

fn state_with_rect() -> EditorState {
    let mut state = EditorState::new();
    let mut next = 100u64;
    let id = state
        .create_node_for_tool(crate::Tool::Rect, &mut next, 0.0, 0.0, 120.0, 40.0)
        .expect("rect created");
    state.selection.anchor = id;
    state
}

fn events(state: &EditorState) -> Option<EventHandlers> {
    state.selected_node().and_then(|n| n.events().cloned())
}

fn key_and_body(action: &Action) -> (String, serde_json::Value) {
    let (key, body) = action.single().expect("one-key action");
    (key.to_string(), body.clone())
}

#[test]
fn navigate_writes_a_replace_with_the_quoted_path_literal() {
    let mut state = state_with_rect();
    let edit = InteractionActionEdit::Navigate {
        path: "/profile".into(),
    };
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &edit));
    let tap = events(&state).and_then(|e| e.on_tap).expect("onTap");
    assert_eq!(
        key_and_body(&tap[0]),
        ("replace".into(), serde_json::json!("\"/profile\""))
    );
}

#[test]
fn each_trigger_writes_its_own_handler_and_keeps_the_others() {
    let mut state = state_with_rect();
    let node_id = state.selection.anchor.clone();
    // A sibling handler the panel does not author must survive.
    assert!(state.apply(EditorCommand::PatchNodeData {
        node_id,
        patch_json: r#"{"events":{"onSwipe":[{"pop":null}]}}"#.into(),
        page_id: None,
    }));
    let back = InteractionActionEdit::Back;
    assert!(state.set_selected_interaction_action(InteractionTrigger::Hover, &back));
    assert!(state.set_selected_interaction_action(InteractionTrigger::LongPress, &back));
    let handlers = events(&state).expect("events");
    assert!(handlers.on_swipe.is_some());
    assert!(handlers.on_hover_enter.is_some());
    assert!(handlers.on_long_press.is_some());
    assert!(handlers.on_tap.is_none());

    assert!(state.remove_selected_interaction(InteractionTrigger::Hover));
    let handlers = events(&state).expect("events");
    assert!(handlers.on_hover_enter.is_none());
    assert!(handlers.on_long_press.is_some());
    assert!(handlers.on_swipe.is_some());
}

#[test]
fn removing_the_last_handler_drops_the_events_block() {
    let mut state = state_with_rect();
    assert!(state
        .set_selected_interaction_action(InteractionTrigger::Tap, &InteractionActionEdit::Back));
    assert!(state.remove_selected_interaction(InteractionTrigger::Tap));
    assert!(events(&state).is_none());
    // Nothing left to remove is not a write.
    assert!(!state.remove_selected_interaction(InteractionTrigger::Tap));
}

#[test]
fn the_verb_is_the_transition_and_a_new_target_keeps_it() {
    let mut state = state_with_rect();
    let to = |path: &str| InteractionActionEdit::Navigate { path: path.into() };
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &to("/a")));
    assert!(state.set_selected_interaction_verb(InteractionTrigger::Tap, NavigateVerb::Push));
    // Same verb again is a no-op.
    assert!(!state.set_selected_interaction_verb(InteractionTrigger::Tap, NavigateVerb::Push));
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &to("/b")));
    let tap = events(&state).and_then(|e| e.on_tap).expect("onTap");
    assert_eq!(
        key_and_body(&tap[0]),
        ("push".into(), serde_json::json!("\"/b\""))
    );
}

#[test]
fn the_verb_cannot_be_set_on_a_back_action() {
    let mut state = state_with_rect();
    assert!(state
        .set_selected_interaction_action(InteractionTrigger::Tap, &InteractionActionEdit::Back));
    assert!(!state.set_selected_interaction_verb(InteractionTrigger::Tap, NavigateVerb::Push));
}

#[test]
fn edits_are_one_undo_step_each() {
    let mut state = state_with_rect();
    state.commit_history();
    assert!(state
        .set_selected_interaction_action(InteractionTrigger::Tap, &InteractionActionEdit::Back));
    assert!(events(&state).is_some());
    state.undo();
    assert!(events(&state).is_none());
}

#[test]
fn nothing_is_written_without_a_selection() {
    let mut state = state_with_rect();
    state.selection.anchor = crate::NodeId::NONE;
    assert!(!state
        .set_selected_interaction_action(InteractionTrigger::Tap, &InteractionActionEdit::Back));
}

#[test]
fn scroll_to_writes_the_section_link_shape() {
    let mut state = state_with_rect();
    let edit = InteractionActionEdit::ScrollTo {
        target: "pricing".into(),
    };
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &edit));
    let tap = events(&state).and_then(|e| e.on_tap).expect("onTap");
    assert_eq!(
        key_and_body(&tap[0]),
        (
            "scroll_to".into(),
            serde_json::json!({"target":"pricing","alignment":"start"})
        )
    );
}

#[test]
fn open_url_stores_a_literal_and_edits_its_address_in_place() {
    let mut state = state_with_rect();
    let edit = InteractionActionEdit::OpenUrl { url: String::new() };
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &edit));
    let url = "https://a.dev/?q='x'";
    assert!(state.set_selected_interaction_url(InteractionTrigger::Tap, url));
    assert!(!state.set_selected_interaction_url(InteractionTrigger::Tap, url));
    let tap = events(&state).and_then(|e| e.on_tap).expect("onTap");
    let (key, body) = key_and_body(&tap[0]);
    assert_eq!(key, "open_url");
    assert_eq!(body["url"], serde_json::json!("\"https://a.dev/?q='x'\""));
    assert_eq!(open_url_of(&body).as_deref(), Some(url));
    // A hand-authored single-quoted literal reads back without its quotes.
    assert_eq!(
        open_url_of(&serde_json::json!({"url":"'https://b.dev'"})).as_deref(),
        Some("https://b.dev")
    );
    // Not an open_url any more: the URL edit is refused.
    let back = InteractionActionEdit::Back;
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &back));
    assert!(!state.set_selected_interaction_url(InteractionTrigger::Tap, "https://c.dev"));
}

#[test]
fn moving_a_trigger_keeps_its_actions_and_never_overwrites() {
    let mut state = state_with_rect();
    let nav = InteractionActionEdit::Navigate {
        path: "/profile".into(),
    };
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &nav));
    assert!(state.move_selected_interaction(InteractionTrigger::Tap, InteractionTrigger::Hover));
    let handlers = events(&state).expect("events");
    assert!(handlers.on_tap.is_none());
    let hover = handlers.on_hover_enter.expect("moved to hover");
    assert_eq!(key_and_body(&hover[0]).0, "replace");

    let back = InteractionActionEdit::Back;
    assert!(state.set_selected_interaction_action(InteractionTrigger::LongPress, &back));
    assert!(
        !state.move_selected_interaction(InteractionTrigger::Hover, InteractionTrigger::LongPress),
        "the target trigger already has an action"
    );
    assert!(!state.move_selected_interaction(InteractionTrigger::Tap, InteractionTrigger::Hover));
}
