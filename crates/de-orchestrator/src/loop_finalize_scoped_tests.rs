use super::*;

/// Two roleless headers the finalizer's role inference would tag `navbar`:
/// `hand` was drawn by someone, `run` was generated.
fn two_headers() -> EditorState {
    EditorState::from_document(
        serde_json::from_str(
            r#"{"version":"1.0.0","children":[
              {"type":"frame","id":"hand","name":"Header","x":0,"y":0,"width":1200,"height":64,"children":[]},
              {"type":"frame","id":"run","name":"Header","x":0,"y":200,"width":1200,"height":64,"children":[]}
            ]}"#,
        )
        .expect("document"),
    )
}

fn root<'a>(state: &'a EditorState, id: &str) -> &'a jian_ops_schema::node::PenNode {
    state
        .active_children()
        .iter()
        .find(|root| root.id_str() == id)
        .expect("root")
}

fn roots(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

#[test]
fn only_the_given_roots_are_finalized() {
    let mut state = two_headers();
    let hand_before = serde_json::to_value(root(&state, "hand")).expect("serialize");

    let summary = apply_loop_finalize_for_roots(&mut state, &roots(&["run"]));

    assert!(!summary.is_empty(), "the finalizer ran on the run's root");
    assert_eq!(root(&state, "run").base().role.as_deref(), Some("navbar"));
    assert_eq!(
        serde_json::to_value(root(&state, "hand")).expect("serialize"),
        hand_before,
        "a root outside the scope stays as authored"
    );
}

#[test]
fn the_whole_page_in_scope_matches_the_plain_finalizer() {
    let mut scoped = two_headers();
    let mut plain = two_headers();

    apply_loop_finalize_for_roots(&mut scoped, &roots(&["hand", "run"]));
    crate::apply_loop_finalize_counted(&mut plain);

    assert_eq!(
        serde_json::to_value(&scoped.doc).expect("serialize"),
        serde_json::to_value(&plain.doc).expect("serialize")
    );
}

#[test]
fn no_root_in_scope_changes_nothing() {
    let mut state = two_headers();
    let before = serde_json::to_value(&state.doc).expect("serialize");

    let summary = apply_loop_finalize_for_roots(&mut state, &roots(&["gone"]));

    assert!(summary.is_empty());
    assert_eq!(serde_json::to_value(&state.doc).expect("serialize"), before);
    assert!(matches!(
        record_loop_finalize_for_roots(&state, &roots(&["gone"])),
        Ok(None)
    ));
}
