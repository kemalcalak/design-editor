use super::*;
use crate::{EditorCommand, NodeId};

/// One page: a large board (every edit inside it makes a snapshot keep its
/// own copy of the board) and a small one.
fn state(rows: usize) -> EditorState {
    let rows: Vec<String> = (0..rows)
        .map(|i| {
            format!(r#"{{"type":"rectangle","id":"r{i}","name":"Row {i}","width":10,"height":10}}"#)
        })
        .collect();
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(&format!(
        r#"{{"version":"1.0","children":[],"pages":[{{"id":"p","name":"P","children":[
            {{"type":"frame","id":"big","width":100,"height":100,"children":[{}]}},
            {{"type":"frame","id":"small","width":10,"height":10}}]}}]}}"#,
        rows.join(",")
    ))
    .expect("document");
    EditorState::from_document(doc)
}

fn rename(state: &mut EditorState, id: &str, name: &str) {
    state.commit_history();
    assert!(state.apply(EditorCommand::SetNodeName {
        node_id: NodeId::new(id),
        name: name.into(),
    }));
}

fn name_of(state: &EditorState, id: &str) -> Option<String> {
    let pages = state.doc.pages.as_ref()?;
    crate::walkers::find_node(&pages[0].children, &NodeId::new(id))
        .and_then(|node| crate::PenNodeExt::base(node).name.clone())
}

#[test]
fn a_history_over_budget_drops_its_oldest_steps_down_to_the_minimum() {
    let mut state = state(200);
    for step in 0..30 {
        rename(&mut state, "r0", &format!("Step {step}"));
    }
    assert_eq!(state.history.past.len(), 30);
    // Every step copied the 200-row board; a 1-byte budget keeps the floor.
    let dropped = state.trim_history_to_budget(1, 5);
    assert_eq!(dropped.len(), 25, "the dropped steps come back to be freed");
    assert_eq!(state.history.past.len(), 5);
    // The kept steps still undo to the names they recorded.
    for step in (24..29).rev() {
        assert!(state.undo());
        assert_eq!(
            name_of(&state, "r0").as_deref(),
            Some(format!("Step {step}").as_str())
        );
    }
    assert!(!state.undo(), "older steps are gone");
}

#[test]
fn a_history_within_budget_is_left_alone() {
    let mut state = state(200);
    for step in 0..30 {
        rename(&mut state, "small", &format!("Step {step}"));
    }
    assert!(state
        .trim_history_to_budget(HISTORY_BYTE_BUDGET, 5)
        .is_empty());
    assert_eq!(state.history.past.len(), 30);
    // At or under the floor nothing is walked or dropped.
    assert!(state.trim_history_to_budget(0, 30).is_empty());
}

/// The steps share every board they did not change, and the budget counts
/// a shared board once: 30 renames of the small board hold one copy of the
/// 200-row board (~0.6 MB) plus 30 small ones, well under 1 MB beyond the
/// document, where counting the big board per step would be ~18 MB.
#[test]
fn a_board_every_step_shares_is_counted_once() {
    let mut state = state(200);
    let big = crate::memory_report::approx_node_bytes_fast(
        &state.doc.pages.as_ref().expect("pages")[0].children[0],
    );
    assert!(
        30 * big > 8 << 20,
        "a naive per-step count would be over budget"
    );
    for step in 0..30 {
        rename(&mut state, "small", &format!("Step {step}"));
    }
    assert!(
        state.trim_history_to_budget(1 << 20, 5).is_empty(),
        "shared, the history is under 1 MB beyond the document"
    );
    assert_eq!(state.history.past.len(), 30);
    // The same budget does bite when every step copies the big board.
    for step in 0..30 {
        rename(&mut state, "r0", &format!("Copy {step}"));
    }
    assert!(!state.trim_history_to_budget(1 << 20, 5).is_empty());
}

#[test]
fn a_write_host_trims_every_tenth_write() {
    let mut state = state(200);
    for step in 0..30 {
        rename(&mut state, "r0", &format!("Step {step}"));
    }
    assert!(
        state.trim_history_after_write(7).is_empty(),
        "not a tenth write"
    );
    assert_eq!(state.history.past.len(), 30);
    // The default budget is far above this small document.
    assert!(state.trim_history_after_write(10).is_empty());
}

#[test]
fn redo_steps_survive_a_trim() {
    let mut state = state(100);
    for step in 0..12 {
        rename(&mut state, "r1", &format!("Step {step}"));
    }
    assert!(state.undo());
    assert!(state.undo());
    let redo = state.history.future.len();
    state.trim_history_to_budget(1, 3);
    assert_eq!(state.history.past.len(), 3);
    assert_eq!(state.history.future.len(), redo);
    assert!(state.redo());
    assert_eq!(name_of(&state, "r1").as_deref(), Some("Step 10"));
}
