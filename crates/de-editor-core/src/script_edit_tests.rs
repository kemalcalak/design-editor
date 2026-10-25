use jian_ops_schema::node::{PenNode, ScriptInputValue};

use super::script_input_from_draft;
use crate::script_inputs::ScriptInputKind;
use crate::test_support::state_with;
use crate::{EditorState, NodeId};

const SOURCE: &str = "/** @input bars: number = 3\n * @input stacked: boolean\n * \
                      @input mode: enum(a, b, c) = b\n * @input accent: color */\nreturn [];";

fn with_script() -> EditorState {
    let script: PenNode = serde_json::from_value(serde_json::json!({
        "type": "script", "id": "s1", "width": 100, "height": 50, "script": SOURCE,
    }))
    .unwrap();
    let mut state = state_with(vec![script]);
    state.set_single_selection(NodeId::new("s1"));
    state
}

fn input(state: &EditorState, name: &str) -> Option<ScriptInputValue> {
    state.selected_script()?.inputs.as_ref()?.get(name).cloned()
}

#[test]
fn typed_drafts_commit_by_kind_and_dollar_binds_a_variable() {
    let mut state = with_script();
    assert!(state.commit_selected_script_input(0, " 7 "));
    assert_eq!(input(&state, "bars"), Some(ScriptInputValue::Number(7.0)));
    assert!(!state.commit_selected_script_input(0, "seven"));
    assert!(state.commit_selected_script_input(3, "#FF8800"));
    assert_eq!(
        input(&state, "accent"),
        Some(ScriptInputValue::Text("#ff8800".into()))
    );
    assert!(state.commit_selected_script_input(3, "$brand"));
    assert_eq!(
        input(&state, "accent"),
        Some(ScriptInputValue::Text("$brand".into()))
    );
    // Same value again: no write.
    assert!(!state.commit_selected_script_input(3, "$brand"));
}

#[test]
fn each_edit_is_one_undo_step() {
    let mut state = with_script();
    let before = state.history.past.len();
    let snapshot = state.snapshot_for_history();
    assert!(state.commit_selected_script_input(0, "9"));
    state.history_push_past(snapshot);
    assert_eq!(state.history.past.len(), before + 1);
    assert!(state.undo());
    assert_eq!(input(&state, "bars"), None);
}

#[test]
fn booleans_toggle_and_enums_cycle_from_their_default() {
    let mut state = with_script();
    assert!(state.toggle_selected_script_input(1));
    assert_eq!(input(&state, "stacked"), Some(ScriptInputValue::Bool(true)));
    assert!(state.toggle_selected_script_input(1));
    assert_eq!(
        input(&state, "stacked"),
        Some(ScriptInputValue::Bool(false))
    );
    // Default is "b": the first step goes to "c", then wraps to "a".
    assert!(state.cycle_selected_script_input(2));
    assert_eq!(
        input(&state, "mode"),
        Some(ScriptInputValue::Text("c".into()))
    );
    assert!(state.cycle_selected_script_input(2));
    assert_eq!(
        input(&state, "mode"),
        Some(ScriptInputValue::Text("a".into()))
    );
    // The wrong control for the kind does nothing.
    assert!(!state.toggle_selected_script_input(0));
    assert!(!state.cycle_selected_script_input(1));
}

#[test]
fn the_source_is_replaced_and_stays_the_input_declarations() {
    let mut state = with_script();
    assert!(state.set_selected_script_source("// @input n: number\nreturn [];"));
    assert!(!state.set_selected_script_source("// @input n: number\nreturn [];"));
    let names: Vec<String> = state
        .selected_script_input_specs()
        .into_iter()
        .map(|spec| spec.name)
        .collect();
    assert_eq!(names, ["n"]);
}

#[test]
fn drafts_that_do_not_fit_are_refused() {
    assert_eq!(
        script_input_from_draft(&ScriptInputKind::Color, "#zz0000"),
        None
    );
    assert_eq!(
        script_input_from_draft(&ScriptInputKind::Boolean, "true"),
        None
    );
    assert_eq!(
        script_input_from_draft(&ScriptInputKind::Number, "inf"),
        None
    );
    assert_eq!(
        script_input_from_draft(&ScriptInputKind::Ref, " n12 "),
        Some(ScriptInputValue::Text("n12".into()))
    );
}
