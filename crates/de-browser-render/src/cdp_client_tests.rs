//! The client against a scripted websocket peer standing in for Chrome.

use std::net::TcpListener;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::Message;

use super::CdpClient;
use crate::BrowserRenderError;

/// A one-connection peer: for every command it receives, `reply` returns
/// the messages to send back (events and / or the answer).
fn peer(reply: impl Fn(&Value) -> Vec<Value> + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        while let Ok(Message::Text(text)) = socket.read() {
            let command: Value = serde_json::from_str(&text).unwrap();
            for message in reply(&command) {
                if socket.send(Message::Text(message.to_string())).is_err() {
                    return;
                }
            }
        }
    });
    format!("ws://127.0.0.1:{port}/devtools/browser/test")
}

#[test]
fn a_reply_is_matched_by_id_and_earlier_events_are_kept() {
    let url = peer(|command| {
        vec![
            json!({ "method": "Page.loadEventFired", "sessionId": "s1", "params": {} }),
            json!({ "id": command["id"], "result": { "echo": command["method"] } }),
        ]
    });
    let mut client = CdpClient::connect(&url).unwrap();
    let result = client
        .call(
            Some("s1"),
            "Page.navigate",
            json!({}),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(result["echo"], "Page.navigate");
    let deadline = Instant::now() + Duration::from_millis(10);
    assert!(
        client
            .wait_event("s1", "Page.loadEventFired", deadline)
            .unwrap(),
        "an event that arrived before the wait is not lost"
    );
    assert!(!client
        .wait_event("s1", "Page.loadEventFired", deadline)
        .unwrap());
}

#[test]
fn an_error_reply_and_a_silent_peer_are_typed_errors() {
    let url = peer(|command| match command["method"].as_str() {
        Some("Bad.method") => vec![json!({ "id": command["id"], "error": { "message": "nope" } })],
        _ => Vec::new(),
    });
    let mut client = CdpClient::connect(&url).unwrap();
    match client.call(None, "Bad.method", json!({}), Duration::from_secs(2)) {
        Err(BrowserRenderError::Protocol { method, message }) => {
            assert_eq!((method.as_str(), message.as_str()), ("Bad.method", "nope"));
        }
        other => panic!("expected a protocol error, got {other:?}"),
    }
    let started = Instant::now();
    assert!(matches!(
        client.call(None, "Silent.method", json!({}), Duration::from_millis(300)),
        Err(BrowserRenderError::Timeout { .. })
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_bad_address_does_not_connect() {
    assert!(matches!(
        CdpClient::connect("http://127.0.0.1:1/x"),
        Err(BrowserRenderError::Connect(_))
    ));
}
