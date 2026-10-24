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

fn simple(state: &EditorState, index: usize) -> SimpleMotion {
    simple_motion(&state.selected_animations()[index]).expect("simple animation")
}

#[test]
fn add_writes_a_mount_fade_in() {
    let mut state = state_with_rect();
    assert!(state.add_selected_animation());
    let motion = simple(&state, 0);
    assert_eq!(motion.trigger, MotionTrigger::Mount);
    assert_eq!(motion.property, MotionProperty::Opacity);
    assert_eq!((motion.from, motion.to), (0.0, 1.0));
    assert_eq!(motion.duration_ms, DEFAULT_MOTION_DURATION_MS);
    assert_eq!(motion.delay_ms, 0);
}

#[test]
fn the_written_animation_passes_document_validation() {
    let mut state = state_with_rect();
    assert!(state.add_selected_animation());
    assert!(state.set_selected_animation_property(0, MotionProperty::TranslateY));
    let node = state.selected_node().expect("node").clone();
    let json = serde_json::to_value(&node).expect("serialize");
    let back: jian_ops_schema::node::PenNode = serde_json::from_value(json).expect("round trip");
    assert_eq!(back.motion_declarations().1, node.motion_declarations().1);
}

#[test]
fn trigger_property_and_values_edit_in_place() {
    let mut state = state_with_rect();
    assert!(state.add_selected_animation());
    assert!(state.set_selected_animation_trigger(0, MotionTrigger::InView));
    assert!(!state.set_selected_animation_trigger(0, MotionTrigger::InView));
    assert!(state.set_selected_animation_property(0, MotionProperty::Rotation));
    assert_eq!(
        (simple(&state, 0).from, simple(&state, 0).to),
        MotionProperty::Rotation.default_range()
    );
    assert!(state.set_selected_animation_value(0, MotionField::From, -90.0));
    assert!(state.set_selected_animation_value(0, MotionField::To, 45.5));
    assert!(state.set_selected_animation_value(0, MotionField::DurationMs, 450.4));
    assert!(state.set_selected_animation_value(0, MotionField::DelayMs, 120.0));
    let motion = simple(&state, 0);
    assert_eq!(motion.trigger, MotionTrigger::InView);
    assert_eq!(motion.property, MotionProperty::Rotation);
    assert_eq!((motion.from, motion.to), (-90.0, 45.5));
    assert_eq!((motion.duration_ms, motion.delay_ms), (450, 120));
}

#[test]
fn durations_clamp_and_non_finite_values_are_rejected() {
    let mut state = state_with_rect();
    assert!(state.add_selected_animation());
    assert!(state.set_selected_animation_value(0, MotionField::DurationMs, 0.0));
    assert_eq!(simple(&state, 0).duration_ms, 1);
    assert!(state.set_selected_animation_value(0, MotionField::DelayMs, 50.0));
    assert!(state.set_selected_animation_value(0, MotionField::DelayMs, -5.0));
    assert_eq!(simple(&state, 0).delay_ms, 0);
    // A value that clamps to the current one is not a write.
    assert!(!state.set_selected_animation_value(0, MotionField::DelayMs, -1.0));
    assert!(state.set_selected_animation_value(0, MotionField::DelayMs, 1e9));
    assert_eq!(simple(&state, 0).delay_ms, MAX_MOTION_MS);
    assert!(!state.set_selected_animation_value(0, MotionField::From, f64::NAN));
}

#[test]
fn removing_the_last_animation_drops_the_field() {
    let mut state = state_with_rect();
    assert!(state.add_selected_animation());
    assert!(state.add_selected_animation());
    assert!(state.remove_selected_animation(0));
    assert_eq!(state.selected_animations().len(), 1);
    assert!(state.remove_selected_animation(0));
    assert!(state
        .selected_node()
        .and_then(|n| n.motion_declarations().1)
        .is_none());
    assert!(!state.remove_selected_animation(0));
}

#[test]
fn a_complex_animation_is_not_simple_and_its_values_are_not_edited() {
    let mut state = state_with_rect();
    let node_id = state.selection.anchor.clone();
    assert!(state.apply(EditorCommand::PatchNodeData {
        node_id,
        patch_json: r#"{"animations":[{"trigger":"mount","durationMs":200,
            "keyframes":{"from":{"opacity":0,"scaleX":0.5},"to":{"opacity":1,"scaleX":1}}}]}"#
            .into(),
        page_id: None,
    }));
    assert!(simple_motion(&state.selected_animations()[0]).is_none());
    assert!(!state.set_selected_animation_value(0, MotionField::To, 2.0));
    assert!(!state.set_selected_animation_property(0, MotionProperty::Opacity));
    // The trigger and removal still work on it.
    assert!(state.set_selected_animation_trigger(0, MotionTrigger::InView));
    assert!(state.remove_selected_animation(0));
}

#[test]
fn edits_keep_easing_and_unknown_keys() {
    let mut state = state_with_rect();
    let node_id = state.selection.anchor.clone();
    assert!(state.apply(EditorCommand::PatchNodeData {
        node_id,
        patch_json: r#"{"animations":[{"trigger":"inView","durationMs":200,"easing":"linear",
            "iterations":2,"futureKey":true,
            "keyframes":[{"offset":0,"values":{"translateX":-10}},
                         {"offset":1,"values":{"translateX":0}}]}]}"#
            .into(),
        page_id: None,
    }));
    assert!(state.set_selected_animation_value(0, MotionField::To, 8.0));
    let animation = &state.selected_animations()[0];
    assert_eq!(animation.easing, jian_ops_schema::motion::Easing::Linear);
    assert_eq!(animation.iterations, 2);
    assert!(animation.extra.contains_key("futureKey"));
}

#[test]
fn a_value_commit_through_the_property_focus_writes_the_field() {
    let mut state = state_with_rect();
    assert!(state.add_selected_animation());
    let focus = crate::PropertyFocus::MotionValue {
        index: 0,
        field: MotionField::DurationMs,
    };
    assert!(state.commit_property_edit(focus, 900.0));
    assert_eq!(simple(&state, 0).duration_ms, 900);
}

#[test]
fn motion_edits_undo_and_redo() {
    let mut state = state_with_rect();
    state.commit_history();
    assert!(state.add_selected_animation());
    assert_eq!(state.selected_animations().len(), 1);
    assert!(state.undo());
    assert!(state.selected_animations().is_empty());
    assert!(state.redo());
    assert_eq!(state.selected_animations().len(), 1);
}

#[test]
fn a_shown_value_commits_back_unchanged() {
    let mut state = state_with_rect();
    assert!(state.add_selected_animation());
    assert!(state.set_selected_animation_property(0, MotionProperty::ScaleX));
    let shown = format_motion_number(simple(&state, 0).from);
    assert_eq!(shown, "0.8");
    let parsed: f64 = shown.parse().expect("number");
    assert!(
        !state.set_selected_animation_value(0, MotionField::From, parsed),
        "focus and blur without typing must not write"
    );
    assert_eq!(format_motion_number(-24.0), "-24");
}

#[test]
fn a_value_edit_keeps_keyframe_level_keys() {
    let mut state = state_with_rect();
    let node_id = state.selection.anchor.clone();
    assert!(state.apply(EditorCommand::PatchNodeData {
        node_id,
        patch_json: r#"{"animations":[{"trigger":"mount","durationMs":200,
            "keyframes":[{"offset":0,"values":{"opacity":0},"easing":"linear"},
                         {"offset":1,"values":{"opacity":1}}]}]}"#
            .into(),
        page_id: None,
    }));
    assert!(state.set_selected_animation_value(0, MotionField::From, 0.5));
    let animation = &state.selected_animations()[0];
    assert!(animation.keyframes[0].extra.contains_key("easing"));
    assert_eq!(simple(&state, 0).from, 0.5);
}
