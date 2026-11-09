//! Page input against a scripted websocket peer, plus a real-Chrome test
//! (`--ignored`) that clicks a button and types into a field.

use std::io::ErrorKind;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::Message;

use super::*;

type Log = Arc<Mutex<Vec<(String, Value, Option<String>)>>>;

/// A fake browser answering every command; records method, params and
/// session.
fn peer() -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = Arc::clone(&log);
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(5)))
            .unwrap();
        let mut held: Option<Value> = None;
        loop {
            let text = match socket.read() {
                Ok(Message::Text(text)) => text,
                Ok(_) => continue,
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    continue
                }
                Err(_) => return,
            };
            let command: Value = serde_json::from_str(&text).unwrap();
            let method = command["method"].as_str().unwrap_or("").to_string();
            let session = command["sessionId"].as_str().map(str::to_string);
            record
                .lock()
                .unwrap()
                .push((method.clone(), command["params"].clone(), session));
            let result = match method.as_str() {
                "Target.attachToTarget" => json!({ "sessionId": "in1" }),
                _ => json!({}),
            };
            let reply = json!({ "id": command["id"], "result": result });
            // A press at x 99 opens an alert, one at x 98 a confirm: like
            // Chrome, its reply waits until the dialog is handled.
            let dialog = match command["params"]["x"].as_i64() {
                Some(99) => Some("alert"),
                Some(98) => Some("confirm"),
                _ => None,
            };
            if let (true, Some(kind)) = (method == "Input.dispatchMouseEvent", dialog) {
                let opening = json!({
                    "method": "Page.javascriptDialogOpening",
                    "sessionId": "in1",
                    "params": { "type": kind, "message": "hi" },
                });
                let _ = socket.send(Message::Text(opening.to_string()));
                held = Some(reply);
                continue;
            }
            if socket.send(Message::Text(reply.to_string())).is_err() {
                return;
            }
            if method == "Page.handleJavaScriptDialog" {
                if let Some(held) = held.take() {
                    let _ = socket.send(Message::Text(held.to_string()));
                }
            }
        }
    });
    (format!("ws://127.0.0.1:{port}/devtools/browser/test"), log)
}

fn methods(log: &Log) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .map(|(m, ..)| m.clone())
        .collect()
}

fn wait_for(log: &Log, method: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if methods(log).iter().any(|m| m == method) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn input_attaches_to_the_tab_and_sends_on_its_session() {
    let (url, log) = peer();
    let mut input = PageInput::connect(&url, "t1").unwrap();
    input
        .dispatch(
            "Input.dispatchMouseEvent",
            json!({ "type": "mousePressed", "x": 3, "y": 4, "button": "left", "clickCount": 1 }),
        )
        .unwrap();
    let calls = log.lock().unwrap().clone();
    assert_eq!(calls[0].0, "Target.attachToTarget");
    assert_eq!(calls[0].1["targetId"], "t1");
    let (method, params, session) = calls.last().unwrap();
    assert_eq!(method, "Input.dispatchMouseEvent");
    assert_eq!(params["x"], 3);
    assert_eq!(session.as_deref(), Some("in1"));
}

#[test]
fn a_dialog_the_input_opens_is_accepted() {
    let (url, log) = peer();
    let mut input = PageInput::connect(&url, "t1").unwrap();
    let started = Instant::now();
    input
        .dispatch(
            "Input.dispatchMouseEvent",
            json!({ "type": "mousePressed", "x": 99, "y": 1, "button": "left" }),
        )
        .expect("the press completes once the dialog is handled");
    assert!(started.elapsed() < Duration::from_secs(2));
    let calls = log.lock().unwrap().clone();
    let handled = calls
        .iter()
        .find(|(m, ..)| m == "Page.handleJavaScriptDialog")
        .expect("the dialog was handled");
    assert_eq!(handled.1["accept"], true);
    assert_eq!(handled.2.as_deref(), Some("in1"));
    // The connection stays usable.
    input
        .dispatch("Input.insertText", json!({ "text": "x" }))
        .unwrap();
}

#[test]
fn a_confirm_the_input_opens_is_cancelled() {
    let (url, log) = peer();
    let mut input = PageInput::connect(&url, "t1").unwrap();
    input
        .dispatch(
            "Input.dispatchMouseEvent",
            json!({ "type": "mousePressed", "x": 98, "y": 1, "button": "left" }),
        )
        .expect("the press completes once the dialog is handled");
    let calls = log.lock().unwrap().clone();
    let handled = calls
        .iter()
        .find(|(m, ..)| m == "Page.handleJavaScriptDialog")
        .expect("the dialog was handled");
    assert_eq!(handled.1["accept"], false, "never say yes for the user");
}

#[test]
fn only_the_input_domain_passes() {
    let (url, log) = peer();
    let mut input = PageInput::connect(&url, "t1").unwrap();
    let refused = input.dispatch("Runtime.evaluate", json!({ "expression": "1" }));
    assert!(matches!(
        refused,
        Err(BrowserRenderError::MethodRefused { .. })
    ));
    assert!(input.dispatch("Input", json!({})).is_err());
    assert!(!methods(&log).iter().any(|m| m == "Runtime.evaluate"));
}

#[test]
fn dropping_detaches_and_leaves_the_tab_open() {
    let (url, log) = peer();
    drop(PageInput::connect(&url, "t1").unwrap());
    assert!(wait_for(&log, "Target.detachFromTarget"));
    assert!(!methods(&log).iter().any(|m| m == "Target.closeTarget"));
    let detach = log
        .lock()
        .unwrap()
        .iter()
        .find(|(m, ..)| m == "Target.detachFromTarget")
        .cloned()
        .unwrap();
    assert_eq!(detach.1["sessionId"], "in1");
}

/// A click and typing through a real stream's input connection reach the
/// page: run with an installed Chromium-based browser (or
/// `DE_BROWSER_PATH`): `cargo test -p de-browser-render -- --ignored`.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_click_and_typing_reach_the_page() {
    use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
    use de_editor_core::browser_input_cdp::cdp_calls;

    let html = "<button id=b style='position:absolute;left:10px;top:10px;width:100px;height:40px'\
                onclick=\"alert('hi');document.title='clicked'\">Go</button>\
                <input id=f style='position:absolute;left:10px;top:80px;width:200px'>";
    let url = format!(
        "data:text/html,{}",
        html.replace(' ', "%20").replace('#', "%23")
    );
    let pool = crate::BrowserPool::default();
    let stream = crate::PageStream::open(
        &pool,
        &url,
        crate::Viewport {
            width: 400,
            height: 300,
            scale: 1.0,
            mobile: false,
        },
        crate::StreamSettings::default(),
    )
    .expect("stream opens");
    let mut input = stream.input().expect("input attaches");
    let none = InputModifiers::default();
    let click = |x: f64, y: f64| {
        [
            BrowserInputEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                click_count: 1,
                buttons: 0,
                modifiers: none,
            },
            BrowserInputEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
                click_count: 1,
                buttons: 1,
                modifiers: none,
            },
        ]
    };
    let mut events: Vec<BrowserInputEvent> = click(50.0, 30.0).into();
    events.extend(click(60.0, 90.0));
    for (key, code) in [("h", "KeyH"), ("i", "KeyI")] {
        events.push(BrowserInputEvent::KeyDown {
            key: key.into(),
            code: code.into(),
            text: Some(key.into()),
            modifiers: none,
        });
        events.push(BrowserInputEvent::KeyUp {
            key: key.into(),
            code: code.into(),
            modifiers: none,
        });
    }
    events.push(BrowserInputEvent::KeyDown {
        key: "Backspace".into(),
        code: "Backspace".into(),
        text: None,
        modifiers: none,
    });
    events.push(BrowserInputEvent::InsertText { text: "!".into() });
    for call in cdp_calls(&events, cfg!(target_os = "macos")) {
        input
            .dispatch(call.method, call.params)
            .expect("dispatched");
    }
    let read = |input: &mut PageInput, expression: &str| {
        input
            .tab_command(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true }),
            )
            .expect("evaluates")["result"]["value"]
            .clone()
    };
    assert_eq!(read(&mut input, "document.title"), json!("clicked"));
    assert_eq!(
        read(&mut input, "document.getElementById('f').value"),
        json!("h!")
    );
    drop(input);
    stream.stop();
}
