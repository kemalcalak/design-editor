//! A thin, blocking Chrome DevTools Protocol client over a localhost
//! websocket: numbered commands answered by id, plus a small buffer of the
//! events that arrived meanwhile so a caller can wait for one (a page's
//! `Page.loadEventFired`) without racing it. Runs on a worker thread only.

use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use crate::BrowserRenderError;

/// Read slice: a blocked read returns this often so deadlines are honoured.
const READ_SLICE: Duration = Duration::from_millis(200);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Events kept while waiting for a command reply; older ones are dropped.
const EVENT_BUFFER: usize = 256;

/// Page snapshots and screenshots arrive as one large message; allow up to
/// 256 MiB instead of tungstenite's 16 MiB frame default.
fn socket_config() -> tungstenite::protocol::WebSocketConfig {
    tungstenite::protocol::WebSocketConfig {
        max_message_size: Some(256 << 20),
        max_frame_size: Some(256 << 20),
        ..Default::default()
    }
}

pub(crate) struct CdpClient {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    events: VecDeque<Value>,
}

impl CdpClient {
    /// Connect to a `ws://127.0.0.1:<port>/devtools/…` address.
    pub(crate) fn connect(ws_url: &str) -> Result<Self, BrowserRenderError> {
        let connect = |message: String| BrowserRenderError::Connect(message);
        let authority = ws_url
            .strip_prefix("ws://")
            .and_then(|rest| rest.split('/').next())
            .ok_or_else(|| connect(format!("not a ws:// address: {ws_url}")))?;
        let addr = authority
            .to_socket_addrs()
            .map_err(|e| connect(e.to_string()))?
            .next()
            .ok_or_else(|| connect(format!("no address for {authority}")))?;
        let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
            .map_err(|e| connect(e.to_string()))?;
        stream
            .set_read_timeout(Some(CONNECT_TIMEOUT))
            .map_err(|e| connect(e.to_string()))?;
        let (socket, _) =
            tungstenite::client::client_with_config(ws_url, stream, Some(socket_config()))
                .map_err(|e| connect(e.to_string()))?;
        socket
            .get_ref()
            .set_read_timeout(Some(READ_SLICE))
            .map_err(|e| connect(e.to_string()))?;
        Ok(Self {
            socket,
            next_id: 0,
            events: VecDeque::new(),
        })
    }

    /// Send `method` (to `session` when given) and wait up to `timeout` for
    /// its reply. Returns the `result` object.
    pub(crate) fn call(
        &mut self,
        session: Option<&str>,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, BrowserRenderError> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        self.socket
            .send(Message::Text(message.to_string()))
            .map_err(|_| BrowserRenderError::Closed)?;
        let deadline = Instant::now() + timeout;
        loop {
            let Some(reply) = self.read_until(deadline)? else {
                return Err(BrowserRenderError::Timeout {
                    method: method.to_string(),
                });
            };
            if reply.get("id").and_then(Value::as_u64) != Some(id) {
                self.buffer_event(reply);
                continue;
            }
            if let Some(error) = reply.get("error") {
                return Err(BrowserRenderError::Protocol {
                    method: method.to_string(),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown error")
                        .to_string(),
                });
            }
            return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// Wait until `deadline` for the event `method` on `session`. True when
    /// it arrived (or had already arrived), false on timeout.
    pub(crate) fn wait_event(
        &mut self,
        session: &str,
        method: &str,
        deadline: Instant,
    ) -> Result<bool, BrowserRenderError> {
        let matches = |event: &Value| {
            event.get("method").and_then(Value::as_str) == Some(method)
                && event.get("sessionId").and_then(Value::as_str) == Some(session)
        };
        if let Some(at) = self.events.iter().position(matches) {
            self.events.remove(at);
            return Ok(true);
        }
        while let Some(message) = self.read_until(deadline)? {
            if matches(&message) {
                return Ok(true);
            }
            self.buffer_event(message);
        }
        Ok(false)
    }

    /// Forget buffered events of `session` (its target was closed).
    pub(crate) fn drop_session_events(&mut self, session: &str) {
        self.events
            .retain(|event| event.get("sessionId").and_then(Value::as_str) != Some(session));
    }

    fn buffer_event(&mut self, event: Value) {
        if event.get("method").is_none() {
            return;
        }
        if self.events.len() == EVENT_BUFFER {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// The next JSON message, `None` once `deadline` passes.
    fn read_until(&mut self, deadline: Instant) -> Result<Option<Value>, BrowserRenderError> {
        loop {
            if Instant::now() >= deadline {
                return Ok(None);
            }
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        return Ok(Some(value));
                    }
                }
                Ok(Message::Close(_)) => return Err(BrowserRenderError::Closed),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(_) => return Err(BrowserRenderError::Closed),
            }
        }
    }
}

#[cfg(test)]
#[path = "cdp_client_tests.rs"]
mod tests;
