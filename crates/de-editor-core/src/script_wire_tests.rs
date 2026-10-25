use super::*;
use crate::NodeId;

fn job() -> ScriptJob {
    ScriptJob {
        node_id: NodeId::new("chart"),
        source: "return [];".into(),
        width: 120.0,
        height: 40.0,
        inputs: BTreeMap::from([("n".to_string(), Value::from(3.0))]),
        hash: "abc".into(),
    }
}

#[test]
fn a_request_round_trips_without_the_bookkeeping() {
    let body = script_run_request(&job());
    assert!(!body.contains("chart") && !body.contains("abc"));
    let request: ScriptRunRequest = serde_json::from_str(&body).unwrap();
    let back = request.into_job();
    assert_eq!(back.source, "return [];");
    assert_eq!((back.width, back.height), (120.0, 40.0));
    assert_eq!(back.inputs["n"], Value::from(3.0));
}

#[test]
fn an_answer_with_nodes_is_generated_output() {
    let nodes: Vec<PenNode> = serde_json::from_value(json!([
        { "type": "rectangle", "id": "a", "width": 10, "height": 10 }
    ]))
    .unwrap();
    let body = script_run_answer(Ok(&nodes), 7);
    let (outcome, elapsed) = parse_script_run_answer(200, &body);
    assert_eq!(outcome, ScriptOutcome::Generated(nodes));
    assert_eq!(elapsed, 7);
}

#[test]
fn failures_carry_the_daemons_message_or_say_why() {
    let body = script_run_answer(Err("script error: no data"), 2);
    assert_eq!(
        parse_script_run_answer(200, &body),
        (ScriptOutcome::Failed("script error: no data".into()), 2)
    );
    assert_eq!(
        parse_script_run_answer(0, ""),
        (ScriptOutcome::Failed(SCRIPT_ENGINE_UNREACHABLE.into()), 0)
    );
    assert_eq!(
        parse_script_run_answer(404, "not found"),
        (
            ScriptOutcome::Failed("script engine answered 404 without a result".into()),
            0
        )
    );
}
