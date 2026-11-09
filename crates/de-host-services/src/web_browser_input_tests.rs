//! The input route against a fake sink: checks, coalescing and the
//! status of each refusal.

use std::sync::Mutex;

use de_editor_core::browser_input::{BrowserInputEvent, BrowserInputReply, MAX_INPUT_EVENTS};
use serde_json::json;

use super::*;

/// Records batches; `live` is the only (client, key) it streams.
struct FakeSink {
    live: (&'static str, &'static str),
    got: Mutex<Vec<(String, String, Vec<BrowserInputEvent>)>>,
}

impl FakeSink {
    fn new() -> Self {
        Self {
            live: ("c1", "n1"),
            got: Mutex::default(),
        }
    }
}

impl InputSink for FakeSink {
    fn dispatch(
        &self,
        client: &str,
        key: &str,
        events: Vec<BrowserInputEvent>,
    ) -> Result<usize, InputRefusal> {
        if (client, key) != self.live {
            return Err(InputRefusal::NotLive);
        }
        let count = events.len();
        self.got
            .lock()
            .unwrap()
            .push((client.into(), key.into(), events));
        Ok(count)
    }
}

fn reply(status_body: (&'static str, String)) -> (&'static str, BrowserInputReply) {
    let (status, body) = status_body;
    (status, serde_json::from_str(&body).expect("reply JSON"))
}

fn moves(n: usize) -> Vec<serde_json::Value> {
    (0..n)
        .map(|i| json!({ "type": "mouseMove", "x": i, "y": 1 }))
        .collect()
}

#[test]
fn a_batch_for_a_live_node_is_coalesced_and_dispatched() {
    let sink = FakeSink::new();
    let mut events = moves(3);
    events.push(json!({ "type": "mouseDown", "x": 2, "y": 1, "button": "left", "clickCount": 1 }));
    events.push(json!({ "type": "keyDown", "key": "a", "code": "KeyA", "text": "a" }));
    let body = json!({ "client": "c1", "key": "n1", "events": events }).to_string();
    let (status, answer) = reply(input_reply_with(&body, Some(&sink)));
    assert_eq!(status, "200 OK");
    assert!(answer.ok);
    assert_eq!(answer.dispatched, 3, "three moves fold into one");
    let got = sink.got.lock().unwrap();
    assert_eq!(got[0].2.len(), 3);
    assert!(matches!(got[0].2[0], BrowserInputEvent::MouseMove { x, .. } if x == 2.0));
}

#[test]
fn another_clients_node_is_not_live() {
    let sink = FakeSink::new();
    let body = json!({ "client": "c2", "key": "n1", "events": moves(1) }).to_string();
    let (status, answer) = reply(input_reply_with(&body, Some(&sink)));
    assert_eq!(status, "404 Not Found");
    assert!(!answer.ok);
    assert!(sink.got.lock().unwrap().is_empty());
}

#[test]
fn malformed_batches_are_refused_before_the_sink() {
    let sink = FakeSink::new();
    let cases = [
        "not json".to_string(),
        json!({ "client": "", "key": "n1", "events": [] }).to_string(),
        json!({ "client": "c1", "key": "n1", "events": moves(MAX_INPUT_EVENTS + 1) }).to_string(),
        json!({ "client": "c1", "key": "n1",
                "events": [{ "type": "keyUp", "key": "", "code": "KeyA" }] })
        .to_string(),
        json!({ "client": "c1", "key": "n1",
                "events": [{ "type": "teleport", "x": 1, "y": 1 }] })
        .to_string(),
    ];
    for body in cases {
        let (status, answer) = reply(input_reply_with(&body, Some(&sink)));
        assert_eq!(status, "400 Bad Request", "{body}");
        assert!(answer.error.is_some());
    }
    assert!(sink.got.lock().unwrap().is_empty());
}

#[test]
fn without_a_sink_the_route_is_unavailable() {
    let body = json!({ "client": "c1", "key": "n1", "events": moves(1) }).to_string();
    let (status, answer) = reply(input_reply_with(&body, None));
    assert_eq!(status, "503 Service Unavailable");
    assert_eq!(answer.error.as_deref(), Some("page input is not available"));
}

#[test]
fn an_empty_batch_is_fine() {
    let sink = FakeSink::new();
    let body = json!({ "client": "c1", "key": "n1", "events": [] }).to_string();
    let (status, answer) = reply(input_reply_with(&body, Some(&sink)));
    assert_eq!((status, answer.dispatched), ("200 OK", 0));
}
