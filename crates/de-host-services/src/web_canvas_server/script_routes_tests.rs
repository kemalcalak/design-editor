use super::*;
use serde_json::Value;

fn answer(body: &str) -> (&'static str, Value) {
    let reply = run_script(body);
    (
        reply.status,
        serde_json::from_str(&reply.body).expect("json answer"),
    )
}

#[test]
fn a_job_runs_with_its_size_and_inputs() {
    let (status, body) = answer(
        r#"{"source":"return Array.from({length: pencil.input.n}, (_, i) => ({type: 'rectangle', x: i * 10, width: 8, height: pencil.height}));",
            "width":100,"height":30,"inputs":{"n":3}}"#,
    );
    assert_eq!(status, "200 OK");
    assert_eq!(body["ok"], true);
    let nodes = body["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[1]["type"], "rectangle");
    assert_eq!(nodes[1]["height"], 30.0);
    // Canonical nodes: they deserialize straight into the schema.
    let typed: Vec<jian_ops_schema::node::PenNode> =
        serde_json::from_value(body["nodes"].clone()).expect("canonical nodes");
    assert_eq!(typed.len(), 3);
}

#[test]
fn a_failing_program_is_an_answer_not_an_http_error() {
    let (status, body) =
        answer(r#"{"source":"throw new Error('no data')","width":10,"height":10}"#);
    assert_eq!(status, "200 OK");
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "script error: no data");
    let (status, body) = answer(r#"{"source":"while (true) {}","width":10,"height":10}"#);
    assert_eq!(status, "200 OK");
    assert!(body["error"].as_str().unwrap().contains("stopped after"));
}

#[test]
fn a_body_that_is_not_a_job_is_refused() {
    let (status, body) = answer(r#"{"script":"return []"}"#);
    assert_eq!(status, "400 Bad Request");
    assert_eq!(body["ok"], false);
}

#[test]
fn output_past_the_node_cap_is_a_failed_run() {
    let (status, body) = answer(
        r#"{"source":"return Array.from({length: 2001}, () => ({type: 'rectangle', width: 1, height: 1}));","width":10,"height":10}"#,
    );
    assert_eq!(status, "200 OK");
    assert_eq!(body["ok"], false);
    assert!(body.get("nodes").is_none());
    assert!(body["error"].as_str().unwrap().contains("2000"), "{body}");
}
