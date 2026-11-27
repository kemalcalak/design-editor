//! `stream_options.include_usage` wire regressions for the OpenAI-compatible
//! agent loop (#446). Loopback mock servers only; no real endpoint or key.

use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::time::Duration;

use de_ai::chat_usage::UsageMeter;
use serde_json::Value;

use super::openai_stream_usage::wants_stream_usage;
use super::tests::{read_http_request, run_loop_collect, update_node_tool_def, ScriptedExecutor};
use super::*;

const MODEL: &str = "gpt-5.6";

/// A text-only turn ending with OpenAI's trailing usage chunk (empty
/// `choices`, sent only when the request asked for it).
fn text_turn_with_usage() -> String {
    [
        r#"data: {"choices":[{"delta":{"content":"done"}}]}"#,
        "",
        r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        "",
        r#"data: {"choices":[],"usage":{"prompt_tokens":1200,"completion_tokens":34,"total_tokens":1234}}"#,
        "",
        "data: [DONE]",
        "",
        "",
    ]
    .join("\n")
}

/// A tool-call turn, so the loop sends a second request.
fn tool_call_turn() -> String {
    [
        r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"update_node","arguments":"{\"nodeId\":\"root\"}"}}]}}]}"#,
        "",
        r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        "",
        "data: [DONE]",
        "",
        "",
    ]
    .join("\n")
}

/// Strict-endpoint refusal of an unknown body field (vLLM / pydantic shape).
fn refusal_body() -> String {
    r#"{"error":{"message":"Extra inputs are not permitted: stream_options","type":"invalid_request_error"}}"#
        .to_string()
}

fn other_400_body() -> String {
    r#"{"error":{"message":"context too long"}}"#.to_string()
}

/// Serve one connection per `(status, content_type, body)`, in order. Each
/// captured request rides the returned receiver.
fn serve_responses(
    responses: Vec<(u16, &'static str, String)>,
) -> (String, std_mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    let (req_tx, req_rx) = std_mpsc::channel::<String>();
    std::thread::spawn(move || {
        for (status, content_type, body) in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("set read timeout");
            let _ = req_tx.send(read_http_request(&mut stream));
            let response = format!(
                "HTTP/1.1 {status} Status\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len(),
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{addr}"), req_rx)
}

/// The refusal memory is process-wide and keyed by URL, and the OS may hand
/// a later test a port an earlier one used, so every test gets its own path.
fn endpoint(base: &str, test: &str) -> String {
    format!("{base}/{test}/chat/completions")
}

fn request_body(request: &str) -> Value {
    let start = request.find("\r\n\r\n").expect("request body separator") + 4;
    serde_json::from_str(&request[start..]).expect("request body JSON")
}

fn bad_request() -> Result<bool, BuiltinHttpError> {
    Err(BuiltinHttpError::HttpStatus {
        label: "openai-compatible".into(),
        status: reqwest::StatusCode::BAD_REQUEST,
    })
}

fn run_against(url: &str, meter: &Arc<UsageMeter>) -> Result<bool, BuiltinHttpError> {
    run_model(url, MODEL, meter)
}

fn run_model(url: &str, model: &str, meter: &Arc<UsageMeter>) -> Result<bool, BuiltinHttpError> {
    let cfg = AgentLoopConfig {
        url: url.to_string(),
        api_key: "sk-test".into(),
        model: model.into(),
        system_prompt: "You are a design editor.".into(),
        history: Vec::new(),
        user_prompt: "continue the design".into(),
        max_output_tokens: 512,
        tools: vec![update_node_tool_def()],
        executor: ScriptedExecutor::ok(r#"{"success":true,"data":{}}"#),
        max_turns: 2,
        finalize_on_exit: false,
        disable_thinking: false,
        dial_policy: crate::provider_dial::EndpointDialPolicy::Trusted,
        usage: Some(meter.clone()),
    };
    run_loop_collect(cfg, false).0
}

#[test]
fn openai_loop_asks_for_the_usage_chunk_and_the_meter_counts_it() {
    let (base, requests) =
        serve_responses(vec![(200, "text/event-stream", text_turn_with_usage())]);
    let meter = Arc::new(UsageMeter::new());
    let url = endpoint(&base, "asks");
    assert_eq!(run_against(&url, &meter), Ok(true));

    let body = request_body(&requests.recv().expect("captured request"));
    assert_eq!(body["stream_options"]["include_usage"], true, "{body}");
    let total = meter.total();
    assert_eq!(total.input_tokens, Some(1200));
    assert_eq!(total.output_tokens, Some(34));
    assert_eq!(meter.requests(), 1);
}

#[test]
fn a_strict_endpoint_refusing_stream_options_is_retried_once_without_it() {
    let (base, requests) = serve_responses(vec![
        (400, "application/json", refusal_body()),
        (200, "text/event-stream", text_turn_with_usage()),
        // A second run against the same endpoint, after the refusal.
        (200, "text/event-stream", text_turn_with_usage()),
        // Another model behind the same URL (a gateway).
        (200, "text/event-stream", text_turn_with_usage()),
    ]);
    let meter = Arc::new(UsageMeter::new());
    let url = endpoint(&base, "refusal");
    assert_eq!(run_against(&url, &meter), Ok(true));

    let refused = request_body(&requests.recv().expect("refused request"));
    assert_eq!(refused["stream_options"]["include_usage"], true);
    let retried = request_body(&requests.recv().expect("retried request"));
    assert!(retried.get("stream_options").is_none(), "{retried}");
    assert_eq!(
        retried["messages"], refused["messages"],
        "same request otherwise"
    );
    // The refused request counts; the retry's usage chunk still lands.
    assert_eq!(meter.requests(), 2);
    assert_eq!(meter.total().input_tokens, Some(1200));

    // The endpoint + model is remembered: the next run skips the field up
    // front instead of paying the refused request again.
    let second = Arc::new(UsageMeter::new());
    assert_eq!(run_against(&url, &second), Ok(true));
    let next = request_body(&requests.recv().expect("next run request"));
    assert!(next.get("stream_options").is_none(), "{next}");
    assert_eq!(second.requests(), 1);

    // The memory is per model: another model at the same URL still asks.
    let other = Arc::new(UsageMeter::new());
    assert_eq!(run_model(&url, "other-model", &other), Ok(true));
    let other_body = request_body(&requests.recv().expect("other model request"));
    assert_eq!(other_body["stream_options"]["include_usage"], true);
    assert_eq!(other.total().input_tokens, Some(1200));
}

#[test]
fn a_400_with_another_cause_is_retried_once_then_reported() {
    let (base, requests) = serve_responses(vec![
        (400, "application/json", other_400_body()),
        (400, "application/json", other_400_body()),
    ]);
    let meter = Arc::new(UsageMeter::new());
    let url = endpoint(&base, "other-cause");
    assert_eq!(run_against(&url, &meter), bad_request());
    let first = request_body(&requests.recv().expect("first request"));
    assert!(first.get("stream_options").is_some(), "{first}");
    let retry = request_body(&requests.recv().expect("retry"));
    assert!(retry.get("stream_options").is_none(), "{retry}");
    assert!(
        requests.recv_timeout(Duration::from_millis(200)).is_err(),
        "only one retry"
    );
    assert_eq!(meter.requests(), 2);
    // A retry that also failed proves nothing about the field: the endpoint
    // is not marked, so the next run asks for usage again.
    assert!(wants_stream_usage(&url, MODEL));
}

#[test]
fn a_failed_retry_reports_the_original_refusal() {
    let (base, requests) = serve_responses(vec![
        (400, "application/json", refusal_body()),
        // The retry fails differently; that error must not replace the 400.
        (
            500,
            "application/json",
            r#"{"error":{"message":"upstream"}}"#.into(),
        ),
    ]);
    let meter = Arc::new(UsageMeter::new());
    let url = endpoint(&base, "failed-retry");
    assert_eq!(run_against(&url, &meter), bad_request());
    requests.recv().expect("first request");
    requests.recv().expect("retry");
    assert_eq!(meter.requests(), 2);
    assert!(wants_stream_usage(&url, MODEL));
}

#[test]
fn a_400_after_the_field_was_accepted_is_reported_without_a_retry() {
    let (base, requests) = serve_responses(vec![
        (200, "text/event-stream", tool_call_turn()),
        (400, "application/json", other_400_body()),
        // Must never be requested.
        (200, "text/event-stream", text_turn_with_usage()),
    ]);
    let meter = Arc::new(UsageMeter::new());
    let url = endpoint(&base, "accepted");
    assert_eq!(run_against(&url, &meter), bad_request());
    let first = request_body(&requests.recv().expect("first request"));
    assert_eq!(first["stream_options"]["include_usage"], true);
    let second = request_body(&requests.recv().expect("second request"));
    assert_eq!(second["stream_options"]["include_usage"], true);
    assert!(
        requests.recv_timeout(Duration::from_millis(200)).is_err(),
        "no retry once the endpoint accepted the field"
    );
    assert_eq!(meter.requests(), 2);
    assert!(wants_stream_usage(&url, MODEL));
}
