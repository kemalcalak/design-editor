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

#[test]
fn add_writes_the_default_transition_once() {
    let mut state = state_with_rect();
    assert!(state.add_selected_transition());
    assert!(!state.add_selected_transition(), "already has one");
    let transition = state.selected_transition().expect("transition");
    assert_eq!(transition.duration_ms, 200);
    assert_eq!(transition.easing, Easing::Standard);
}

#[test]
fn duration_and_easing_edit_in_place_and_keep_the_property_filter() {
    let mut state = state_with_rect();
    let node_id = state.selection.anchor.clone();
    assert!(state.apply(EditorCommand::PatchNodeData {
        node_id,
        patch_json: r#"{"transition":{"durationMs":120,"properties":["fill"]}}"#.into(),
        page_id: None,
    }));
    assert!(state.set_selected_transition_duration(450.4));
    assert!(!state.set_selected_transition_duration(450.0), "unchanged");
    assert!(state.set_selected_transition_easing(MotionEasing::EaseOut));
    let transition = state.selected_transition().expect("transition");
    assert_eq!(transition.duration_ms, 450);
    assert_eq!(transition.easing, Easing::EaseOut);
    assert_eq!(transition.properties, Some(vec!["fill".to_string()]));
    let summary = TransitionSummary::of(&transition);
    assert_eq!(summary.easing, Some(MotionEasing::EaseOut));
}

#[test]
fn duration_clamps_and_rejects_non_finite_values() {
    let mut state = state_with_rect();
    assert!(state.add_selected_transition());
    assert!(!state.set_selected_transition_duration(f64::NAN));
    assert!(state.set_selected_transition_duration(1e9));
    assert_eq!(
        state.selected_transition().map(|t| t.duration_ms),
        Some(crate::motion_edit::MAX_MOTION_MS)
    );
}

#[test]
fn remove_clears_the_field_and_a_cubic_bezier_reads_as_custom() {
    let mut state = state_with_rect();
    let node_id = state.selection.anchor.clone();
    assert!(
        state.apply(EditorCommand::PatchNodeData {
            node_id,
            patch_json:
                r#"{"transition":{"durationMs":120,"easing":{"cubicBezier":[0.1,0.2,0.3,0.4]}}}"#
                    .into(),
            page_id: None,
        })
    );
    let transition = state.selected_transition().expect("transition");
    assert_eq!(TransitionSummary::of(&transition).easing, None);
    assert!(state.remove_selected_transition());
    assert!(state.selected_transition().is_none());
    assert!(!state.remove_selected_transition());
}
