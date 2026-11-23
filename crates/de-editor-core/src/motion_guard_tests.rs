use super::*;
use crate::state::EditorState;

fn state_with_rect() -> (EditorState, crate::NodeId) {
    let mut state = EditorState::new();
    let mut next = 100u64;
    let id = state
        .create_node_for_tool(crate::Tool::Rect, &mut next, 0.0, 0.0, 120.0, 40.0)
        .expect("rect created");
    (state, id)
}

fn patch(state: &mut EditorState, id: &crate::NodeId, keyframes: &str) -> bool {
    state.apply(EditorCommand::PatchNodeData {
        node_id: id.clone(),
        patch_json: format!(
            r#"{{"animations":[{{"trigger":"mount","durationMs":200,"keyframes":{keyframes}}}]}}"#
        ),
        page_id: None,
    })
}

#[test]
fn patching_scale_stores_both_axes() {
    let (mut state, id) = state_with_rect();
    assert!(patch(
        &mut state,
        &id,
        r#"[{"offset":0,"values":{"scale":0.5}},{"offset":1,"values":{"scale":1}}]"#
    ));
    let node = crate::walkers::find_node(state.active_children(), &id).expect("node");
    let animations = node.motion_declarations().1.expect("animations");
    let first = &animations[0].keyframes[0].values;
    assert_eq!(first.get("scaleX"), Some(&serde_json::json!(0.5)));
    assert_eq!(first.get("scaleY"), Some(&serde_json::json!(0.5)));
    assert!(!first.contains_key("scale"));
}

#[test]
fn patching_an_unknown_property_or_bad_offsets_is_rejected() {
    let (mut state, id) = state_with_rect();
    let before = serde_json::to_value(state.active_children()).unwrap();
    assert!(!patch(
        &mut state,
        &id,
        r#"[{"offset":0,"values":{"blur":0}},{"offset":1,"values":{"blur":4}}]"#
    ));
    assert!(!patch(
        &mut state,
        &id,
        r#"[{"offset":0.2,"values":{"opacity":0}},{"offset":1,"values":{"opacity":1}}]"#
    ));
    assert_eq!(
        serde_json::to_value(state.active_children()).unwrap(),
        before
    );
}

#[test]
fn inserted_subtrees_are_guarded_down_to_their_children() {
    let (mut state, _) = state_with_rect();
    let child: PenNode = serde_json::from_value(serde_json::json!({
        "type": "rectangle", "id": "c",
        "animations": [{"trigger": "mount", "durationMs": 200, "keyframes": [
            {"offset": 0, "values": {"notAProperty": 1}},
            {"offset": 1, "values": {"opacity": 1}}
        ]}]
    }))
    .unwrap();
    let parent: PenNode = serde_json::from_value(serde_json::json!({
        "type": "frame", "id": "p", "children": [child]
    }))
    .unwrap();
    assert!(!state.apply(EditorCommand::InsertSubtree {
        nodes: vec![parent],
        parent_id: crate::NodeId::NONE,
        page_id: None,
    }));
}

#[test]
fn miscased_scale_is_refused_with_the_axis_hint() {
    let mut value = serde_json::json!({
        "children": [{"animations": [{"trigger": "mount", "durationMs": 1, "keyframes": [
            {"offset": 0, "values": {"Scale": 0}},
            {"offset": 1, "values": {"opacity": 1}}
        ]}]}]
    });
    let error = prepare_motion_json(&mut value).unwrap_err();
    assert!(
        error.path.starts_with("$.children[0].animations[0]"),
        "{}",
        error.path
    );
    assert!(error.reason.contains("scaleX"), "{error}");
}
