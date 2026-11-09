//! The stream against a scripted websocket peer standing in for Chrome's
//! screencast: it sends `initial` frames on start and one more per ack,
//! and records every command with the time it came. A reply and the
//! frames it brings go out in one write, as Chrome sends them.

use std::io::ErrorKind;
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};
use tungstenite::Message;

use super::*;
use crate::screencast_worker::TabMode;

type Log = Arc<Mutex<Vec<(String, Value, Instant)>>>;

struct Peer {
    url: String,
    log: Log,
    /// Events the test pushes to the client.
    push: mpsc::Sender<Value>,
}

impl Peer {
    fn methods(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .map(|(m, ..)| m.clone())
            .collect()
    }

    fn calls(&self, method: &str) -> Vec<(Value, Instant)> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, ..)| m == method)
            .map(|(_, params, at)| (params.clone(), *at))
            .collect()
    }

    /// Wait up to 2 s for `method` to have been called `count` times.
    fn wait_calls(&self, method: &str, count: usize) -> Vec<(Value, Instant)> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let calls = self.calls(method);
            if calls.len() >= count || Instant::now() >= deadline {
                return calls;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn frame_event(id: i64) -> Value {
    let data = base64::engine::general_purpose::STANDARD.encode(format!("frame-{id}"));
    json!({
        "method": "Page.screencastFrame",
        "sessionId": "s1",
        "params": {
            "data": data,
            "sessionId": id,
            "metadata": { "deviceWidth": 400, "deviceHeight": 300, "scrollOffsetY": 12.5 },
        },
    })
}

/// A fake browser: `initial` frames per start, one per ack.
fn peer(initial: i64) -> Peer {
    peer_with(initial, i64::MAX)
}

/// Like [`peer`], sending at most `budget` frames in all (then the page
/// is still).
fn peer_with(initial: i64, budget: i64) -> Peer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let (push, pushed) = mpsc::channel::<Value>();
    let record = Arc::clone(&log);
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        // Without this a frame written right behind its reply can wait
        // for the client's delayed ACK (~40 ms on Linux).
        stream.set_nodelay(true).unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket
            .get_ref()
            .set_read_timeout(Some(Duration::from_millis(5)))
            .unwrap();
        let mut next_frame = 0;
        loop {
            while let Ok(event) = pushed.try_recv() {
                let _ = socket.send(Message::Text(event.to_string()));
            }
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
            record.lock().unwrap().push((
                method.clone(),
                command["params"].clone(),
                Instant::now(),
            ));
            let result = match method.as_str() {
                "Target.createTarget" => json!({ "targetId": "t1" }),
                "Target.attachToTarget" => json!({ "sessionId": "s1" }),
                "Runtime.evaluate" => json!({ "result": { "value": "Agent" } }),
                _ => json!({}),
            };
            let mut out = vec![json!({ "id": command["id"], "result": result })];
            let frames = match method.as_str() {
                "Page.startScreencast" => initial,
                "Page.screencastFrameAck" => 1,
                _ => 0,
            };
            for _ in 0..frames.min(budget - next_frame) {
                next_frame += 1;
                out.push(frame_event(next_frame));
            }
            for message in out {
                if socket.write(Message::Text(message.to_string())).is_err() {
                    return;
                }
            }
            if socket.flush().is_err() {
                return;
            }
        }
    });
    Peer {
        url: format!("ws://127.0.0.1:{port}/devtools/browser/test"),
        log,
        push,
    }
}

fn uncapped() -> StreamSettings {
    StreamSettings {
        max_fps: 0.0,
        ..StreamSettings::default()
    }
}

fn stream_on(peer: &Peer, mode: TabMode, settings: StreamSettings) -> PageStream {
    stream_with(peer, mode, settings, None)
}

fn stream_with(
    peer: &Peer,
    mode: TabMode,
    settings: StreamSettings,
    lease: Option<PoolLease>,
) -> PageStream {
    let mut client = CdpClient::connect(&peer.url).unwrap();
    let tab = match mode {
        TabMode::Owned => Tab::open(&mut client).unwrap(),
        TabMode::Shared => Tab::attach(&mut client, "t1").unwrap(),
    };
    PageStream::from_tab(client, tab, mode, settings, lease, None).unwrap()
}

fn jpeg_text(frame: &StreamFrame) -> String {
    String::from_utf8(frame.jpeg.clone()).unwrap()
}

/// A signal per frame handed out (or the stream's end), from the waker.
fn handed_out(stream: &PageStream) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    stream.set_frame_waker(move || {
        let _ = tx.lock().unwrap().send(());
    });
    rx
}

fn wait_handed_out(signal: &mpsc::Receiver<()>) {
    signal
        .recv_timeout(Duration::from_secs(2))
        .expect("a frame was handed out");
}

#[test]
fn settings_default_to_a_1024_wide_jpeg_at_ten_fps() {
    let settings = StreamSettings::default();
    let params = settings.start_params();
    assert_eq!(params["format"], "jpeg");
    assert_eq!(params["maxWidth"], 1024);
    assert_eq!(params["everyNthFrame"], 1);
    assert_eq!(settings.min_interval(), Duration::from_millis(100));
    let web = StreamSettings {
        max_fps: 4.0,
        every_nth_frame: 0,
        ..settings
    };
    assert_eq!(web.min_interval(), Duration::from_millis(250));
    assert_eq!(web.start_params()["everyNthFrame"], 1, "never 0");
    assert_eq!(uncapped().min_interval(), Duration::ZERO);
}

#[test]
fn a_frame_is_acked_only_after_the_consumer_took_it() {
    let peer = peer(1);
    let stream = stream_on(&peer, TabMode::Owned, uncapped());
    let signal = handed_out(&stream);
    let started = peer.wait_calls("Emulation.setFocusEmulationEnabled", 1);
    assert_eq!(started[0].0["enabled"], true, "headless tabs are focused");
    wait_handed_out(&signal);
    // Give the worker time it would need to ack, were it allowed to.
    std::thread::sleep(Duration::from_millis(100));
    assert!(peer.calls("Page.screencastFrameAck").is_empty(), "untaken");
    let frame = stream.take_frame().unwrap().expect("the first frame");
    assert_eq!((frame.seq, jpeg_text(&frame).as_str()), (1, "frame-1"));
    assert_eq!(frame.metadata.device_width, 400.0);
    assert_eq!(frame.metadata.scroll_offset_y, 12.5);
    let acks = peer.wait_calls("Page.screencastFrameAck", 1);
    assert_eq!(acks[0].0["sessionId"], 1);
    let next = stream
        .wait_frame(Duration::from_secs(2))
        .unwrap()
        .expect("the ack let the next frame come");
    assert_eq!(jpeg_text(&next), "frame-2");
    stream.stop();
}

#[test]
fn acks_keep_to_the_frame_rate_cap() {
    let peer = peer(1);
    let settings = StreamSettings {
        max_fps: 10.0,
        ..StreamSettings::default()
    };
    let stream = stream_on(&peer, TabMode::Owned, settings);
    for _ in 0..5 {
        stream
            .wait_frame(Duration::from_secs(2))
            .unwrap()
            .expect("a frame");
    }
    let acks = peer.wait_calls("Page.screencastFrameAck", 4);
    assert!(acks.len() >= 4);
    for pair in acks.windows(2) {
        let gap = pair[1].1 - pair[0].1;
        // The peer stamps on receipt; allow scheduling slop.
        assert!(gap >= Duration::from_millis(90), "acks {gap:?} apart");
    }
    stream.stop();
}

#[test]
fn the_newest_frame_wins_and_frames_are_acked_one_at_a_time() {
    // Frame 1 is handed out; 2 and 3 arrive while it waits, 3 replaces 2.
    // Capped, as the desktop runs: the cap gives the frame an ack brings
    // time to arrive before the next owed frame could be acked.
    let peer = peer(3);
    let stream = stream_on(&peer, TabMode::Owned, StreamSettings::default());
    let interval = StreamSettings::default().min_interval();
    let signal = handed_out(&stream);
    wait_handed_out(&signal);
    // Frames 2 and 3 came in the same write as frame 1; let the worker
    // read them before frame 1 is taken.
    std::thread::sleep(Duration::from_millis(100));
    let first = stream.take_frame().unwrap().expect("a frame");
    assert_eq!((first.seq, jpeg_text(&first).as_str()), (1, "frame-1"));
    let newest = stream.wait_frame(Duration::from_secs(2)).unwrap().unwrap();
    assert_eq!((newest.seq, jpeg_text(&newest).as_str()), (2, "frame-3"));
    // One ack lets one new frame come; the next owed frame waits until
    // that one was taken too, past the cap as well.
    let acks = peer.wait_calls("Page.screencastFrameAck", 1);
    assert_eq!(acks[0].0["sessionId"], 1);
    std::thread::sleep(interval * 2);
    assert_eq!(peer.calls("Page.screencastFrameAck").len(), 1);
    let next = stream.wait_frame(Duration::from_secs(2)).unwrap().unwrap();
    assert_eq!(jpeg_text(&next), "frame-4");
    let acks = peer.wait_calls("Page.screencastFrameAck", 2);
    assert_eq!(acks[1].0["sessionId"], 2);
    stream.stop();
}

#[test]
fn a_quiet_page_after_a_burst_is_restarted_once_for_its_final_picture() {
    // Two unacked frames, then the page goes still: Chrome may have
    // dropped its last repaint, so the stream asks for the picture again.
    let peer = peer_with(2, 2);
    let stream = stream_on(&peer, TabMode::Owned, uncapped());
    let signal = handed_out(&stream);
    wait_handed_out(&signal);
    // Both frames came in one write; hold frame 1 until the worker read
    // frame 2, so both are unacked at once (a consumer that takes frame 1
    // at once lets it be acked first, and nothing was dropped).
    std::thread::sleep(Duration::from_millis(100));
    for _ in 0..2 {
        stream.wait_frame(Duration::from_secs(2)).unwrap().unwrap();
    }
    let starts = peer.wait_calls("Page.startScreencast", 2);
    assert_eq!(starts.len(), 2, "one refresh after the quiet period");
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(peer.calls("Page.startScreencast").len(), 2, "only once");
    stream.stop();

    // A consumer that stops taking holds the stream: no refresh then.
    let peer = self::peer(2);
    let stream = stream_on(&peer, TabMode::Owned, uncapped());
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        peer.calls("Page.startScreencast").len(),
        1,
        "busy, not quiet"
    );
    stream.stop();

    // A still page sends one frame: never refreshed.
    let peer = peer_with(1, 1);
    let stream = stream_on(&peer, TabMode::Owned, uncapped());
    stream.wait_frame(Duration::from_secs(2)).unwrap().unwrap();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(peer.calls("Page.startScreencast").len(), 1);
    stream.stop();
}

#[test]
fn a_new_viewport_or_settings_restart_the_screencast() {
    let peer = peer(0);
    let stream = stream_on(&peer, TabMode::Owned, uncapped());
    let viewport = Viewport {
        width: 640,
        height: 480,
        scale: 1.0,
        mobile: false,
    };
    stream.set_viewport(viewport).unwrap();
    let narrow = StreamSettings {
        max_width: 320,
        ..uncapped()
    };
    stream.set_settings(narrow).unwrap();
    let methods = peer.methods();
    let tail: Vec<&str> = methods
        .iter()
        .map(String::as_str)
        .skip_while(|m| *m != "Page.stopScreencast")
        .filter(|m| m.starts_with("Page.") || *m == "Emulation.setDeviceMetricsOverride")
        .collect();
    assert_eq!(
        tail,
        [
            "Page.stopScreencast",
            "Emulation.setDeviceMetricsOverride",
            "Page.startScreencast",
            "Page.stopScreencast",
            "Page.startScreencast",
        ]
    );
    let starts = peer.calls("Page.startScreencast");
    assert_eq!(starts.last().unwrap().0["maxWidth"], 320);
    stream.stop();
}

#[test]
fn stopping_closes_an_owned_tab_and_releases_the_browser() {
    let peer = peer(1);
    let pool = BrowserPool::new();
    pool.lock().holders += 1;
    let lease = PoolLease {
        pool: pool.clone(),
        generation: 0,
        broken: false,
    };
    let stream = stream_with(&peer, TabMode::Owned, uncapped(), Some(lease));
    assert_eq!(pool.lock().holders, 1, "a streaming browser is held");
    stream.stop();
    let methods = peer.methods();
    let tail = &methods[methods.len() - 2..];
    assert_eq!(tail, ["Page.stopScreencast", "Target.closeTarget"]);
    assert_eq!(pool.lock().holders, 0);
}

#[test]
fn a_shared_tab_is_detached_not_closed_and_keeps_its_viewport() {
    let peer = peer(1);
    let stream = stream_on(&peer, TabMode::Shared, uncapped());
    assert!(stream.is_shared_tab());
    let viewport = Viewport {
        width: 1,
        height: 1,
        scale: 1.0,
        mobile: false,
    };
    assert!(matches!(
        stream.set_viewport(viewport),
        Err(BrowserRenderError::SharedTabViewport)
    ));
    stream.restart().expect("a plain restart is fine");
    stream.stop();
    let methods = peer.methods();
    assert_eq!(methods.last().unwrap(), "Target.detachFromTarget");
    assert!(!methods.iter().any(|m| m == "Target.closeTarget"));
    assert!(!methods.iter().any(|m| m == "Target.createTarget"));
}

#[test]
fn a_detached_tab_ends_the_stream_with_a_typed_error() {
    let peer = peer(0);
    let stream = stream_on(&peer, TabMode::Shared, uncapped());
    let signal = handed_out(&stream);
    peer.push
        .send(json!({ "method": "Target.detachedFromTarget", "params": { "sessionId": "s1" } }))
        .unwrap();
    assert!(matches!(
        stream.wait_frame(Duration::from_secs(2)),
        Err(BrowserRenderError::TabClosed)
    ));
    assert!(stream.is_ended());
    // The waker runs after the end is visible, so it may still be on its
    // way when `wait_frame` returns.
    assert!(
        signal.recv_timeout(Duration::from_secs(2)).is_ok(),
        "the waker ran"
    );
    assert!(matches!(
        stream.take_frame(),
        Err(BrowserRenderError::Closed)
    ));
    assert!(stream.restart().is_err());
}

#[test]
fn a_waker_set_after_a_frame_came_is_called_at_once() {
    let peer = peer(1);
    let stream = stream_on(&peer, TabMode::Owned, uncapped());
    wait_handed_out(&handed_out(&stream));
    // Frame 1 waits untaken: a waker set now must not miss it.
    let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&called);
    stream.set_frame_waker(move || flag.store(true, std::sync::atomic::Ordering::SeqCst));
    assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    assert!(stream.take_frame().unwrap().is_some());
    stream.stop();
}

#[test]
fn another_sessions_detach_does_not_end_the_stream() {
    let peer = peer(1);
    let stream = stream_on(&peer, TabMode::Shared, uncapped());
    peer.push
        .send(json!({ "method": "Target.detachedFromTarget", "params": { "sessionId": "other" } }))
        .unwrap();
    assert!(stream.wait_frame(Duration::from_secs(2)).unwrap().is_some());
    std::thread::sleep(Duration::from_millis(50));
    assert!(!stream.is_ended());
    stream.stop();
}

/// Streams local pages with the installed browser:
/// `cargo test -p de-browser-render -- --ignored`.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn an_animated_page_streams_and_a_still_page_goes_quiet() {
    let dir = std::env::temp_dir().join(format!("de-stream-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let animated = dir.join("animated.html");
    std::fs::write(
        &animated,
        "<html><body style='margin:0'><div id='b' style='width:100px;height:100px;\
         background:#3e6df5;animation:m 1s linear infinite alternate'></div>\
         <style>@keyframes m{from{margin-left:0}to{margin-left:300px}}</style></body></html>",
    )
    .unwrap();
    let still = dir.join("still.html");
    std::fs::write(
        &still,
        "<html><body style='background:#eee'>still</body></html>",
    )
    .unwrap();
    let pool = BrowserPool::new();
    let viewport = Viewport {
        width: 400,
        height: 300,
        scale: 1.0,
        mobile: false,
    };
    let url = format!("file://{}", animated.display());
    let stream = PageStream::open(&pool, &url, viewport, StreamSettings::default()).unwrap();
    let started = Instant::now();
    let mut frames = 0;
    while started.elapsed() < Duration::from_secs(2) {
        if let Some(frame) = stream.wait_frame(Duration::from_millis(200)).unwrap() {
            assert!(frame.jpeg.starts_with(&[0xFF, 0xD8]), "a JPEG");
            frames += 1;
        }
    }
    eprintln!("[perf] animated page: {frames} frames in 2 s at a 10 fps cap");
    assert!(frames > 5, "the animation streams ({frames} frames)");
    assert!(frames <= 22, "the cap holds ({frames} frames)");
    assert!(
        !pool.shutdown_if_idle(Duration::ZERO),
        "held while streaming"
    );
    stream.stop();

    let url = format!("file://{}", still.display());
    let stream = PageStream::open(&pool, &url, viewport, StreamSettings::default()).unwrap();
    // The first paint, then nothing.
    let _ = stream.wait_frame(Duration::from_secs(2)).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let _ = stream.take_frame().unwrap();
    let quiet = stream.wait_frame(Duration::from_secs(1)).unwrap();
    assert!(quiet.is_none(), "a still page sends no frames");
    stream.restart().unwrap();
    assert!(
        stream.wait_frame(Duration::from_secs(2)).unwrap().is_some(),
        "a restart sends the current picture"
    );
    stream.stop();
    assert!(pool.shutdown_if_idle(Duration::ZERO), "released after stop");

    // A kept-open MCP tab streams over a second connection.
    let sessions = crate::PageSessions::new(pool.clone());
    let url = format!("file://{}", animated.display());
    sessions.with("node", viewport, |s| s.load(&url)).unwrap();
    let stream = sessions
        .stream("node", StreamSettings::default())
        .unwrap()
        .expect("a kept tab");
    assert!(stream.wait_frame(Duration::from_secs(2)).unwrap().is_some());
    let wider = Viewport {
        width: 600,
        ..viewport
    };
    sessions.resize_stream("node", wider, &stream).unwrap();
    let frame = stream
        .wait_frame(Duration::from_secs(2))
        .unwrap()
        .expect("frames after the resize");
    assert_eq!(frame.metadata.device_width, 600.0);
    stream.stop();
    assert_eq!(sessions.len(), 1, "the MCP tab outlives the stream");
    assert!(sessions
        .stream("missing", StreamSettings::default())
        .unwrap()
        .is_none());
    sessions.close("node");
    let _ = std::fs::remove_dir_all(dir);
}
