//! A loopback Anthropic Messages API for the `de agent` tests: the real
//! provider client, agent loop and tools run against it, only the model is
//! scripted. Markers in the prompt pick the script:
//!
//! - `<<frame:Name>>` — the first turn authors a frame called `Name`
//!   (default `Landing`) with `batch_design`; later turns end the run.
//! - `<<fail>>` — every request is refused with HTTP 400.
//! - `<<hang>>` — requests are read and never answered.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

/// Input / output tokens every scripted turn reports.
pub const TURN_INPUT_TOKENS: u64 = 1000;
pub const TURN_OUTPUT_TOKENS: u64 = 50;

pub struct FakeProvider {
    pub base_url: String,
    requests: Arc<AtomicUsize>,
    api_keys: Arc<Mutex<Vec<String>>>,
}

impl FakeProvider {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake provider");
        let addr = listener.local_addr().expect("fake provider address");
        let requests = Arc::new(AtomicUsize::new(0));
        let api_keys = Arc::new(Mutex::new(Vec::new()));
        let (count, keys) = (requests.clone(), api_keys.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let (count, keys) = (count.clone(), keys.clone());
                std::thread::spawn(move || serve(stream, &count, &keys));
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            requests,
            api_keys,
        }
    }

    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    /// Every `x-api-key` header received.
    pub fn api_keys(&self) -> Vec<String> {
        self.api_keys.lock().expect("keys mutex").clone()
    }
}

fn serve(mut stream: TcpStream, count: &AtomicUsize, keys: &Mutex<Vec<String>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Some((headers, body)) = read_request(&mut stream) else {
        return;
    };
    count.fetch_add(1, Ordering::SeqCst);
    if let Some(key) = header(&headers, "x-api-key") {
        keys.lock().expect("keys mutex").push(key);
    }
    let request: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let prompt = first_user_text(&request);
    if prompt.contains("<<hang>>") {
        // Hold the connection open until the client gives up.
        std::thread::sleep(Duration::from_secs(300));
        return;
    }
    let response = if prompt.contains("<<fail>>") {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"fake provider refused the request"}}"#;
        format!(
            "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    } else {
        let sse = if has_tool_result(&request) {
            end_turn()
        } else {
            tool_use_turn(&frame_name(&prompt))
        };
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{sse}",
            sse.len()
        )
    };
    let _ = stream.write_all(response.as_bytes());
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&buf);
        let Some(end) = text.find("\r\n\r\n") else {
            continue;
        };
        let headers = text[..end].to_string();
        let length = header(&headers, "content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        if buf.len() >= end + 4 + length {
            let body = String::from_utf8_lossy(&buf[end + 4..end + 4 + length]).into_owned();
            return Some((headers, body));
        }
    }
}

fn header(headers: &str, name: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

/// The text of the first user message (the brief rides there).
fn first_user_text(request: &Value) -> String {
    let Some(first) = request["messages"].as_array().and_then(|m| m.first()) else {
        return String::new();
    };
    match &first["content"] {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn has_tool_result(request: &Value) -> bool {
    request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|message| message["content"].as_array())
        .flatten()
        .any(|block| block["type"] == "tool_result")
}

fn frame_name(prompt: &str) -> String {
    prompt
        .split("<<frame:")
        .nth(1)
        .and_then(|rest| rest.split(">>").next())
        .map(str::to_string)
        .unwrap_or_else(|| "Landing".to_string())
}

fn tool_use_turn(frame: &str) -> String {
    let operations = format!(
        "I(null, {{type:'frame', name:'{frame}', width:390, height:600, \
         fill:[{{type:'solid', color:'#FFFFFF'}}], layout:'vertical', \
         children:[{{type:'text', name:'Headline', content:'{frame} headline', \
         fontFamily:'Inter, system-ui, sans-serif', fontSize:28, fontWeight:700}}]}});"
    );
    let input = serde_json::json!({ "operations": operations }).to_string();
    let partial = serde_json::to_string(&input).expect("json string");
    format!(
        "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"usage\":{{\"input_tokens\":{TURN_INPUT_TOKENS},\"output_tokens\":1}}}}}}\n\n\
         event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"batch_design\"}}}}\n\n\
         event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"input_json_delta\",\"partial_json\":{partial}}}}}\n\n\
         event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"tool_use\"}},\"usage\":{{\"output_tokens\":{TURN_OUTPUT_TOKENS}}}}}\n\n\
         event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n"
    )
}

fn end_turn() -> String {
    format!(
        "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"usage\":{{\"input_tokens\":{TURN_INPUT_TOKENS},\"output_tokens\":1}}}}}}\n\n\
         event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n\
         event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":\"Done.\"}}}}\n\n\
         event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"output_tokens\":{TURN_OUTPUT_TOKENS}}}}}\n\n\
         event: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n"
    )
}
