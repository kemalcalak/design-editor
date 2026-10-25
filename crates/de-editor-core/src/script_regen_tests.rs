use super::*;
use crate::walkers::find_node;

const DOC: &str = r##"{
  "version":"1.0.0",
  "children":[
    {"type":"frame","id":"page","width":800,"height":600,"children":[
      {"type":"script","id":"chart","width":200,"height":100,
       "script":"// @input bars: number = 3\nreturn [];","inputs":{"bars":4}}
    ]}
  ]
}"##;

fn state() -> EditorState {
    EditorState::from_document(jian_ops_schema::load_str(DOC).unwrap().value)
}

fn rect(id: &str) -> PenNode {
    serde_json::from_str(&format!(
        r#"{{"type":"rectangle","id":"{id}","width":10,"height":10}}"#
    ))
    .unwrap()
}

fn script(state: &EditorState) -> &ScriptNode {
    match find_node(&state.doc.children, &NodeId::new("chart")) {
        Some(PenNode::Script(script)) => script,
        other => panic!("no script node: {other:?}"),
    }
}

fn set_bars(state: &mut EditorState, bars: f64) {
    let id = NodeId::new("chart");
    if let Some(PenNode::Script(script)) = find_node_mut(&mut state.doc.children, &id) {
        script.inputs.get_or_insert_with(Default::default).insert(
            "bars".into(),
            jian_ops_schema::node::ScriptInputValue::Number(bars),
        );
    }
    state.mark_document_changed();
}

#[test]
fn a_stale_script_yields_one_job_with_its_inputs_and_size() {
    let mut state = state();
    let jobs = state.take_script_jobs();
    assert_eq!(jobs.len(), 1);
    let job = &jobs[0];
    assert_eq!(job.node_id.as_str(), "chart");
    assert_eq!((job.width, job.height), (200.0, 100.0));
    assert_eq!(job.inputs["bars"], Value::from(4.0));
    assert_eq!(
        state.script_regen.status(&job.node_id),
        Some(&ScriptRunStatus::Running)
    );
    // No change since the scan: no walk, no job.
    assert!(state.take_script_jobs().is_empty());
}

#[test]
fn a_landed_result_becomes_the_children_without_an_undo_step() {
    let mut state = state();
    let job = state.take_script_jobs().remove(0);
    let history = state.history.past.len();
    let outcome = ScriptOutcome::Generated(vec![rect("x"), rect("y")]);
    assert!(state.land_script_output(&job.node_id, &job.hash, outcome, 3));
    let children: Vec<&str> = script(&state)
        .frame
        .children
        .iter()
        .flatten()
        .map(|c| c.id_str())
        .collect();
    assert_eq!(children, ["chart-0", "chart-1"]);
    assert_eq!(
        script(&state).script_hash.as_deref(),
        Some(job.hash.as_str())
    );
    assert_eq!(state.history.past.len(), history);
    assert!(state.is_dirty());
    // The saved hash now matches: nothing left to run.
    assert!(state.take_script_jobs().is_empty());
}

#[test]
fn an_edit_during_a_run_drops_its_result_and_asks_again() {
    let mut state = state();
    let job = state.take_script_jobs().remove(0);
    set_bars(&mut state, 9.0);
    // Single flight: the edited node waits for its running job.
    assert!(state.take_script_jobs().is_empty());
    let outcome = ScriptOutcome::Generated(vec![rect("x")]);
    assert!(!state.land_script_output(&job.node_id, &job.hash, outcome, 1));
    assert!(script(&state).frame.children.is_none());
    let next = state.take_script_jobs();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].inputs["bars"], Value::from(9.0));
}

#[test]
fn a_failed_run_keeps_the_children_and_is_not_retried_until_an_edit() {
    let mut state = state();
    let job = state.take_script_jobs().remove(0);
    let failed = ScriptOutcome::Failed("boom".into());
    assert!(!state.land_script_output(&job.node_id, &job.hash, failed, 1));
    assert!(matches!(
        state.script_regen.status(&job.node_id),
        Some(ScriptRunStatus::Failed { message, .. }) if message == "boom"
    ));
    state.mark_document_changed();
    assert!(state.take_script_jobs().is_empty());
    set_bars(&mut state, 5.0);
    assert_eq!(state.take_script_jobs().len(), 1);
}

#[test]
fn generated_ids_skip_ids_other_nodes_use() {
    let mut state = state();
    let job = state.take_script_jobs().remove(0);
    if let Some(page) = find_node_mut(&mut state.doc.children, &NodeId::new("page")) {
        page.children_mut().unwrap().push(rect("chart-0"));
    }
    let outcome = ScriptOutcome::Generated(vec![rect("a"), rect("b")]);
    assert!(state.land_script_output(&job.node_id, &job.hash, outcome, 1));
    let children: Vec<&str> = script(&state)
        .frame
        .children
        .iter()
        .flatten()
        .map(|c| c.id_str())
        .collect();
    assert_eq!(children, ["chart-0-1", "chart-1"]);
}

#[test]
fn the_fingerprint_follows_source_size_and_inputs() {
    let inputs = BTreeMap::from([("a".to_string(), Value::from(1.0))]);
    let base = script_fingerprint("x", 10.0, 10.0, &inputs);
    assert_eq!(base, script_fingerprint("x", 10.0, 10.0, &inputs));
    assert_ne!(base, script_fingerprint("y", 10.0, 10.0, &inputs));
    assert_ne!(base, script_fingerprint("x", 11.0, 10.0, &inputs));
    let other = BTreeMap::from([("a".to_string(), Value::from(2.0))]);
    assert_ne!(base, script_fingerprint("x", 10.0, 10.0, &other));
}
