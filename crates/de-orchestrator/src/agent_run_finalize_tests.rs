use super::*;
use de_editor_core::PenNodeExt;

fn empty_phone(id: &str, name: &str, x: f64) -> jian_ops_schema::node::PenNode {
    serde_json::from_value(serde_json::json!(
        {"type":"frame","id":id,"name":name,"x":x,"y":0,"width":375,"height":812,"children":[]}
    ))
    .expect("node")
}

fn name_of<'a>(state: &'a EditorState, id: &str) -> Option<&'a str> {
    state
        .active_children()
        .iter()
        .find(|root| root.id_str() == id)
        .and_then(|root| root.base().name.as_deref())
}

#[test]
fn only_the_runs_empty_screens_are_reported_and_marked() {
    let mut state = EditorState::new();
    state
        .active_children_mut()
        .push(empty_phone("board", "Board", 0.0));
    let run = WriteBaseline::capture(&state);
    state
        .active_children_mut()
        .push(empty_phone("saved", "Saved", 500.0));

    assert_eq!(
        agent_run_screens(&state, &run),
        AgentRunScreens {
            committed: vec!["Saved".to_string()],
            unfilled: vec!["Saved".to_string()],
        }
    );

    let finalized = finalize_agent_run(&mut state, &run);

    assert_eq!(finalized.unfilled, vec!["Saved".to_string()]);
    assert_eq!(name_of(&state, "saved"), Some("Saved (unfilled)"));
    assert_eq!(
        name_of(&state, "board"),
        Some("Board"),
        "a board the run did not create is never marked"
    );
}

#[test]
fn a_run_that_created_nothing_finalizes_nothing() {
    let mut state = EditorState::new();
    state
        .active_children_mut()
        .push(empty_phone("board", "Board", 0.0));
    let run = WriteBaseline::capture(&state);
    let before = serde_json::to_value(&state.doc).expect("serialize");

    let finalized = finalize_agent_run(&mut state, &run);

    assert!(finalized.quality.is_empty());
    assert!(finalized.unfilled.is_empty());
    assert_eq!(serde_json::to_value(&state.doc).expect("serialize"), before);
}
