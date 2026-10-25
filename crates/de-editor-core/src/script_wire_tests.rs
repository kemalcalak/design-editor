use super::*;
use crate::NodeId;
use serde_json::json;

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
    let body = script_run_answer(ScriptOutcome::Generated(nodes.clone()), 7);
    let (outcome, elapsed) = parse_script_run_answer(200, &body);
    assert_eq!(outcome, ScriptOutcome::Generated(nodes));
    assert_eq!(elapsed, 7);
}

#[test]
fn failures_carry_the_daemons_message_or_say_why() {
    let body = script_run_answer(ScriptOutcome::Failed("script error: no data".into()), 2);
    assert_eq!(
        parse_script_run_answer(200, &body),
        (ScriptOutcome::Failed("script error: no data".into()), 2)
    );
    assert_eq!(
        parse_script_run_answer(500, "<html>oops</html>"),
        (
            ScriptOutcome::Failed("script engine answered 500 without a result".into()),
            0
        )
    );
}

#[test]
fn no_daemon_is_unreachable_not_a_failed_program() {
    for (status, text) in [
        (0, ""),
        (404, "<html>Not Found</html>"),
        (405, ""),
        (404, r#"{"error":"unknown route"}"#),
    ] {
        assert_eq!(
            parse_script_run_answer(status, text),
            (ScriptOutcome::Unreachable, 0),
            "{status} {text}"
        );
    }
}
