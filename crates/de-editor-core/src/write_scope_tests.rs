use super::*;

fn node(value: serde_json::Value) -> PenNode {
    serde_json::from_value(value).expect("node")
}

#[test]
fn a_frame_lifted_to_the_top_level_is_not_a_new_root() {
    let mut state = EditorState::new();
    *state.active_children_mut() = vec![node(serde_json::json!(
        {"type":"group","id":"g","children":[
            {"type":"frame","id":"phone","width":375,"height":"fit_content"}
        ]}
    ))];
    let baseline = WriteBaseline::capture(&state);

    let phone = state.active_children_mut()[0]
        .children_mut()
        .expect("group children")
        .remove(0);
    state.active_children_mut().push(phone);

    let scope = baseline.scope_after(&state);
    assert!(scope.new_roots.is_empty(), "{scope:?}");
    assert!(scope.extended_roots.is_empty(), "{scope:?}");
    assert!(baseline.new_roots_after(&state).is_empty());
}

#[test]
fn switching_the_active_page_scopes_nothing() {
    let mut state = EditorState::new();
    state.doc.pages = Some(
        serde_json::from_value(serde_json::json!([
            {"id":"p1","name":"One","children":[]},
            {"id":"p2","name":"Two","children":[
                {"type":"frame","id":"phone","width":375,"height":"fit_content"}
            ]}
        ]))
        .expect("pages"),
    );
    state.ui.active_page_index = 0;
    let baseline = WriteBaseline::capture(&state);

    state.ui.active_page_index = 1;

    let scope = baseline.scope_after(&state);
    assert!(
        scope.new_roots.is_empty() && scope.touched.is_empty(),
        "{scope:?}"
    );
    assert!(baseline.new_roots_after(&state).is_empty());
}

#[test]
fn a_repeated_id_does_not_count_as_changed() {
    let mut state = EditorState::new();
    *state.active_children_mut() = vec![
        node(serde_json::json!({"type":"rectangle","id":"dup","name":"A","width":10,"height":10})),
        node(serde_json::json!({"type":"rectangle","id":"dup","name":"B","width":20,"height":20})),
    ];
    let baseline = WriteBaseline::capture(&state);

    let scope = baseline.scope_after(&state);
    assert!(scope.touched.is_empty(), "{scope:?}");
}

#[test]
fn new_roots_after_matches_the_scope_new_roots() {
    let mut state = EditorState::new();
    *state.active_children_mut() = vec![node(serde_json::json!(
        {"type":"frame","id":"board","width":375,"height":812,"children":[]}
    ))];
    let baseline = WriteBaseline::capture(&state);

    state.active_children_mut()[0]
        .children_mut()
        .expect("board children")
        .push(node(serde_json::json!(
            {"type":"rectangle","id":"added","width":10,"height":10}
        )));
    state.active_children_mut().push(node(serde_json::json!(
        {"type":"frame","id":"screen","width":375,"height":812}
    )));

    let scope = baseline.scope_after(&state);
    assert_eq!(scope.new_roots, vec!["screen".to_string()]);
    assert_eq!(scope.extended_roots, vec!["board".to_string()]);
    assert!(scope.is_new_root("screen") && !scope.is_new_root("board"));
    assert!(scope.is_new_or_extended_root("board"));
    assert_eq!(
        baseline.new_roots_after(&state),
        BTreeSet::from(["screen".to_string()])
    );
}
