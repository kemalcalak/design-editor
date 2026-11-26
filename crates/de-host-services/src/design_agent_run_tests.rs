//! Engine tests: cancellation, progress, budget, and a full run against a
//! loopback Anthropic-wire provider (never the network).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use de_ai::chat_provider::{ChatDelta, ChatRequest};
use de_editor_core::{
    BuiltinAgentConfig, BuiltinAgentKind, BuiltinAgentPresetKey, EditorState, PenNodeExt,
};

use super::*;

/// A driver that never finishes until it is told to stop.
struct StalledDriver;

impl DesignLoopDriver for StalledDriver {
    fn run(
        &self,
        _request: ChatRequest,
        cancel: Arc<AtomicBool>,
    ) -> Box<dyn Iterator<Item = ChatDelta> + Send> {
        Box::new(std::iter::from_fn(move || loop {
            if cancel.load(Ordering::Acquire) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }))
    }
}

/// Streams one text delta, then stalls.
struct ChattyThenStalled;

impl DesignLoopDriver for ChattyThenStalled {
    fn run(
        &self,
        _request: ChatRequest,
        cancel: Arc<AtomicBool>,
    ) -> Box<dyn Iterator<Item = ChatDelta> + Send> {
        let mut sent = false;
        Box::new(std::iter::from_fn(move || {
            if !sent {
                sent = true;
                return Some(ChatDelta::TextDelta("thinking about it".into()));
            }
            loop {
                if cancel.load(Ordering::Acquire) {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }))
    }
}

#[test]
fn a_raised_cancel_flag_stops_the_run_without_a_command() {
    let state = EditorState::default();
    let cancel = Arc::new(AtomicBool::new(false));
    let raiser = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        raiser.store(true, Ordering::Release);
    });
    let started = std::time::Instant::now();
    let result = run_with_driver(
        &state,
        "a pricing page",
        Duration::from_secs(60),
        &|_| Box::new(StalledDriver),
        RunControl {
            cancel: Some(cancel),
            on_delta: None,
            usage: None,
        },
    );
    assert!(matches!(result, Err(DesignAgentRunError::Cancelled)));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "cancel must be noticed promptly"
    );
}

#[test]
fn progress_sees_every_delta_and_the_budget_still_applies() {
    let state = EditorState::default();
    let mut seen = Vec::new();
    let mut on_delta = |delta: &ChatDelta| seen.push(format!("{delta:?}"));
    let result = run_with_driver(
        &state,
        "a pricing page",
        Duration::from_millis(300),
        &|_| Box::new(ChattyThenStalled),
        RunControl {
            cancel: None,
            on_delta: Some(&mut on_delta),
            usage: None,
        },
    );
    assert!(matches!(
        result,
        Err(DesignAgentRunError::Timeout { seconds: 0 })
    ));
    assert_eq!(seen.len(), 1);
    assert!(seen[0].contains("thinking about it"));
}

#[test]
fn validate_brief_trims_and_bounds() {
    assert_eq!(validate_brief("  hi  ").unwrap(), "hi");
    assert_eq!(validate_brief(" \n "), Err(DesignAgentRunError::EmptyBrief));
    let huge = "x".repeat(MAX_BRIEF_BYTES + 1);
    assert_eq!(
        validate_brief(&huge).unwrap_err().to_string(),
        format!(
            "brief is too large ({} bytes; maximum {MAX_BRIEF_BYTES})",
            MAX_BRIEF_BYTES + 1
        )
    );
}

fn provider(base_url: &str, enabled: bool) -> BuiltinAgentConfig {
    BuiltinAgentConfig {
        id: "builtin-1".into(),
        preset: BuiltinAgentPresetKey::Custom,
        display_name: "Loopback".into(),
        kind: BuiltinAgentKind::Anthropic,
        api_key: "sk-test".into(),
        models: vec!["claude-test".into()],
        base_url: base_url.into(),
        enabled,
    }
}

#[test]
fn select_saved_provider_rules() {
    let ready = provider("http://127.0.0.1:1", true);
    let disabled = BuiltinAgentConfig {
        id: "builtin-2".into(),
        ..provider("http://127.0.0.1:1", false)
    };
    let agents = vec![disabled.clone(), ready.clone()];
    assert_eq!(
        select_saved_provider(&agents, None).unwrap().id,
        "builtin-1"
    );
    assert_eq!(
        select_saved_provider(&agents, Some("builtin-2")),
        Err(DesignAgentRunError::ProviderNotReady("builtin-2".into()))
    );
    assert_eq!(
        select_saved_provider(&agents, Some("nope")),
        Err(DesignAgentRunError::UnknownProvider("nope".into()))
    );
    assert_eq!(
        select_saved_provider(&[disabled], None),
        Err(DesignAgentRunError::NoConfiguredProvider)
    );
}

/// Anthropic SSE for one `batch_design` call that authors a frame.
fn tool_use_turn() -> String {
    let input = serde_json::json!({
        "operations": "I(null, {type:'frame', name:'Landing', width:1200, height:800, fill:[{type:'solid', color:'#FFFFFF'}], layout:'vertical', children:[{type:'text', name:'Headline', content:'Hello', fontFamily:'Inter, system-ui, sans-serif', fontSize:32, fontWeight:700}]});"
    })
    .to_string();
    let partial = serde_json::to_string(&input).expect("json string");
    format!(
        "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"usage\":{{\"input_tokens\":100,\"output_tokens\":1}}}}}}\n\n\
         event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"batch_design\"}}}}\n\n\
         event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"input_json_delta\",\"partial_json\":{partial}}}}}\n\n\
         event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"tool_use\"}},\"usage\":{{\"output_tokens\":20}}}}\n\n\
         event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n"
    )
}

/// Anthropic SSE for a closing text turn.
fn end_turn() -> String {
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":200,\"output_tokens\":1}}}\n\n\
     event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
     event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Done.\"}}\n\n\
     event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n\
     event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
        .to_string()
}

/// A loopback provider: the first request gets the tool call, every later
/// one (fill / corrective nudges included) the closing turn.
fn serve_loopback() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback provider");
    let addr = listener.local_addr().expect("local addr");
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = requests.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            read_request(&mut stream);
            let index = counter.fetch_add(1, Ordering::SeqCst);
            let body = if index == 0 {
                tool_use_turn()
            } else {
                end_turn()
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{addr}"), requests)
}

fn read_request(stream: &mut std::net::TcpStream) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let Ok(read) = stream.read(&mut chunk) else {
            return;
        };
        if read == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&buf);
        if let Some(header_end) = text.find("\r\n\r\n") {
            let length = text[..header_end]
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if buf.len() >= header_end + 4 + length {
                return;
            }
        }
    }
}

#[test]
fn a_loopback_provider_run_lands_the_design_and_reports_usage() {
    let (base_url, requests) = serve_loopback();
    let state = EditorState::default();
    let run = DesignAgentRun {
        brief: "a landing page".into(),
        timeout: Duration::from_secs(60),
        max_turns: Some(4),
        provider: provider(&base_url, true),
        model: None,
    };
    let outcome = run_design_agent(&state, &run, RunControl::default()).expect("run lands");
    let mut landed = state.clone();
    assert!(landed.apply(outcome.command));
    assert!(landed
        .active_children()
        .iter()
        .any(|node| node.base().name.as_deref() == Some("Landing")));
    let report = outcome.report;
    assert_eq!(report.model, "claude-test");
    assert_eq!(report.tool_calls, 1);
    let made = requests.load(Ordering::SeqCst) as u64;
    assert_eq!(report.provider_requests, made);
    assert_eq!(report.usage.input_tokens, Some(100 + 200 * (made - 1)));
    assert_eq!(report.usage.output_tokens, Some(20 + 5 * (made - 1)));
    assert_eq!(report.to_json()["usage"]["requests"], made);
}

#[test]
fn an_unsaved_model_is_refused_before_any_request() {
    let run = DesignAgentRun {
        brief: "a landing page".into(),
        timeout: Duration::from_secs(5),
        max_turns: None,
        provider: provider("http://127.0.0.1:1", true),
        model: Some("other-model".into()),
    };
    let error = run_design_agent(&EditorState::default(), &run, RunControl::default())
        .expect_err("unsaved model");
    assert_eq!(
        error,
        DesignAgentRunError::ModelNotSaved {
            provider: "builtin-1".into(),
            model: "other-model".into(),
        }
    );
}

#[test]
fn a_failed_run_still_counts_its_requests_on_the_callers_meter() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind refusing provider");
    let addr = listener.local_addr().expect("local addr");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            read_request(&mut stream);
            let body =
                r#"{"type":"error","error":{"type":"invalid_request_error","message":"no"}}"#;
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let meter = Arc::new(UsageMeter::new());
    let run = DesignAgentRun {
        brief: "a landing page".into(),
        timeout: Duration::from_secs(30),
        max_turns: Some(2),
        provider: provider(&format!("http://{addr}"), true),
        model: None,
    };
    let error = run_design_agent(
        &EditorState::default(),
        &run,
        RunControl {
            usage: Some(meter.clone()),
            ..RunControl::default()
        },
    )
    .expect_err("a refused request fails the run");
    assert!(matches!(error, DesignAgentRunError::Loop(_)), "{error:?}");
    assert!(meter.requests() >= 1);
    assert!(!meter.total().reported());
}
