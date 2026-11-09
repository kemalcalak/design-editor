use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Weak;
use std::time::Instant;

use de_browser_render::{FrameMetadata, StreamSettings, Viewport};
use de_editor_core::browser_live::BrowserSnapshotSpec;
use de_editor_core::browser_live_wire::{BrowserLiveEntry, WEB_LIVE_MAX_FPS};

use super::*;

/// A stream with the screencast's contract: one latest-wins slot.
struct FakeStream {
    url: String,
    slot: Mutex<Option<StreamFrame>>,
    seq: AtomicUsize,
    ended: AtomicBool,
    resizes: AtomicUsize,
    wake: source::Waker,
    alive: Arc<AtomicUsize>,
}

impl FakeStream {
    fn publish(&self) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        *lock(&self.slot) = Some(StreamFrame {
            seq,
            jpeg: tiny_jpeg(),
            metadata: FrameMetadata::default(),
        });
        (self.wake)();
    }

    fn end(&self) {
        self.ended.store(true, Ordering::SeqCst);
        (self.wake)();
    }
}

impl Drop for FakeStream {
    fn drop(&mut self) {
        self.alive.fetch_sub(1, Ordering::SeqCst);
    }
}

impl LiveStream for FakeStream {
    fn take_frame(&self) -> Result<Option<StreamFrame>, BrowserRenderError> {
        if let Some(frame) = lock(&self.slot).take() {
            return Ok(Some(frame));
        }
        if self.ended.load(Ordering::SeqCst) {
            return Err(BrowserRenderError::Closed);
        }
        Ok(None)
    }

    fn set_viewport(&self, _: Viewport) -> Result<(), BrowserRenderError> {
        self.resizes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn set_settings(&self, _: StreamSettings) -> Result<(), BrowserRenderError> {
        self.resizes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Default)]
struct FakeSource {
    opened: Mutex<Vec<Weak<FakeStream>>>,
    opens: AtomicUsize,
    alive: Arc<AtomicUsize>,
}

impl FakeSource {
    /// The live stream for `url`, newest first.
    fn stream(&self, url: &str) -> Option<Arc<FakeStream>> {
        lock(&self.opened)
            .iter()
            .rev()
            .filter_map(Weak::upgrade)
            .find(|stream| stream.url == url)
    }
}

impl LiveSource for FakeSource {
    fn open(
        &self,
        spec: &LiveSpec,
        wake: source::Waker,
    ) -> Result<Arc<dyn LiveStream>, BrowserRenderError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        if spec.url.contains("broken") {
            return Err(BrowserRenderError::NoBrowser);
        }
        self.alive.fetch_add(1, Ordering::SeqCst);
        let stream = Arc::new(FakeStream {
            url: spec.url.clone(),
            slot: Mutex::new(None),
            seq: AtomicUsize::new(0),
            ended: AtomicBool::new(false),
            resizes: AtomicUsize::new(0),
            wake,
            alive: Arc::clone(&self.alive),
        });
        lock(&self.opened).push(Arc::downgrade(&stream));
        Ok(stream)
    }
}

/// A writer the test can cut, like a client closing its tab.
#[derive(Clone, Default)]
struct Pipe {
    data: Arc<Mutex<Vec<u8>>>,
    cut: Arc<AtomicBool>,
}

impl Write for Pipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cut.load(Ordering::SeqCst) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        lock(&self.data).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Pipe {
    fn events(&self) -> Vec<BrowserLiveEvent> {
        let text = String::from_utf8(lock(&self.data).clone()).unwrap();
        text.lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|json| serde_json::from_str(json).unwrap())
            .collect()
    }

    fn wait_for(&self, what: impl Fn(&[BrowserLiveEvent]) -> bool) -> Vec<BrowserLiveEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let events = self.events();
            if what(&events) {
                return events;
            }
            assert!(Instant::now() < deadline, "timed out: {events:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn tiny_jpeg() -> Vec<u8> {
    let mut out = Vec::new();
    let pixels = [0u8; 4 * 2 * 3];
    image::codecs::jpeg::JpegEncoder::new(&mut out)
        .encode(&pixels, 4, 2, image::ExtendedColorType::Rgb8)
        .unwrap();
    out
}

fn entry(key: &str, url: &str, width: u32) -> BrowserLiveEntry {
    BrowserLiveEntry {
        key: key.into(),
        page: BrowserSnapshotSpec {
            url: url.into(),
            layout_width: width,
            layout_height: 800,
            scale: 1.0,
            mobile: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        },
        frame_width: 512,
        frame_height: 512,
    }
}

fn spec(key: &str, url: &str, width: u32) -> LiveSpec {
    select(vec![entry(key, url, width)], None, |_| true)
        .wanted
        .remove(0)
}

fn hub() -> (Arc<FakeSource>, LiveHub) {
    let source = Arc::new(FakeSource::default());
    let hub = LiveHub::new(source.clone(), Duration::from_millis(20));
    (source, hub)
}

fn set(client: &str, nodes: Vec<BrowserLiveEntry>) -> BrowserLiveSet {
    BrowserLiveSet {
        client: client.into(),
        nodes,
    }
}

#[test]
fn the_selection_screens_checks_and_caps() {
    let entries = vec![
        entry("a", "example.com/a", 1280),
        entry("blocked", "http://169.254.169.254/", 1280),
        entry("bad", "javascript:alert(1)", 1280),
        entry("b", "https://example.com/b", 1280),
        entry("a", "https://example.com/again", 1280),
        entry("c", "https://example.com/c", 1280),
        entry("d", "https://example.com/d", 1280),
    ];
    let screen = |url: &str| crate::browser_url_policy::url_allowed(url, false, None);
    let selection = select(entries, None, screen);
    let keys: Vec<_> = selection.wanted.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, ["a", "b", "c"]);
    assert_eq!(selection.wanted[0].url, "https://example.com/a");
    assert_eq!(selection.blocked, ["blocked"]);
    assert_eq!(selection.invalid[0].0, "bad");
    assert_eq!(selection.dropped, ["d"]);
    let settings = selection.wanted[0].settings;
    assert_eq!(settings.max_fps, WEB_LIVE_MAX_FPS);
    assert_eq!((settings.max_width, settings.max_height), (512, 512));

    let mut huge = entry("h", "https://example.com", 1280);
    huge.frame_width = 9000;
    let mut zero = entry("z", "https://example.com", 1280);
    zero.frame_height = 0;
    let selection = select(vec![huge, zero], None, |_| true);
    assert_eq!(selection.wanted[0].settings.max_width, 1024);
    assert_eq!(selection.invalid[0].0, "z");
}

#[test]
fn the_plan_opens_resizes_and_stops() {
    let current: BTreeMap<_, _> = [
        spec("keep", "https://a.example", 1280),
        spec("resize", "https://b.example", 1280),
        spec("gone", "https://c.example", 1280),
        spec("moved", "https://d.example", 1280),
    ]
    .into_iter()
    .map(|s| (s.key.clone(), s))
    .collect();
    let mut smaller = spec("resize", "https://b.example", 1280);
    smaller.settings.max_width = 256;
    let wanted = [
        spec("keep", "https://a.example", 1280),
        smaller.clone(),
        spec("moved", "https://elsewhere.example", 1280),
        spec("new", "https://e.example", 390),
    ];
    let plan = plan(&current, &wanted);
    assert_eq!(plan.stop, ["gone", "moved"]);
    let opened: Vec<_> = plan.open.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(opened, ["moved", "new"]);
    assert_eq!(
        plan.update,
        [plan::Update {
            spec: smaller,
            viewport: false,
            settings: true,
        }]
    );
    let wider = [spec("keep", "https://a.example", 1440)];
    let current: BTreeMap<_, _> = [("keep".to_string(), spec("keep", "https://a.example", 1280))]
        .into_iter()
        .collect();
    let update = &plan::plan(&current, &wider).update[0];
    assert!(update.viewport && !update.settings);
}

#[test]
fn a_set_call_runs_at_most_three_streams_and_follows_changes() {
    let (source, hub) = hub();
    let lease = hub.connect().unwrap();
    let id = lease.client.id.clone();
    let nodes: Vec<_> = (0..5)
        .map(|i| entry(&format!("n{i}"), &format!("https://example.com/{i}"), 1280))
        .collect();
    let (status, reply) = hub.apply(set(&id, nodes), None, |_| true);
    assert_eq!(status, "200 OK");
    assert_eq!(reply.live, ["n0", "n1", "n2"]);
    assert_eq!(reply.dropped, ["n3", "n4"]);
    assert_eq!(source.alive.load(Ordering::SeqCst), 3);

    // Resize one, drop one, keep one.
    let nodes = vec![
        entry("n0", "https://example.com/0", 1280),
        entry("n1", "https://example.com/1", 390),
    ];
    let (_, reply) = hub.apply(set(&id, nodes), None, |_| true);
    assert_eq!(reply.live, ["n0", "n1"]);
    assert_eq!(source.alive.load(Ordering::SeqCst), 2);
    let resized = source.stream("https://example.com/1").unwrap();
    assert_eq!(resized.resizes.load(Ordering::SeqCst), 1);
    assert_eq!(source.opens.load(Ordering::SeqCst), 3, "nothing reopened");

    let (status, reply) = hub.apply(set("nobody", vec![]), None, |_| true);
    assert_eq!(status, "404 Not Found");
    assert!(!reply.ok);
}

#[test]
fn a_refused_address_is_blocked_and_never_opened() {
    let (source, hub) = hub();
    let lease = hub.connect().unwrap();
    let screen = |url: &str| crate::browser_url_policy::url_allowed(url, false, None);
    let nodes = vec![entry("meta", "http://169.254.169.254/latest", 1280)];
    let (_, reply) = hub.apply(set(&lease.client.id, nodes), None, screen);
    assert_eq!(reply.blocked, ["meta"]);
    assert!(reply.live.is_empty());
    assert_eq!(source.opens.load(Ordering::SeqCst), 0);
    let events = lease.client.drain();
    assert_eq!(events, [BrowserLiveEvent::Blocked { key: "meta".into() }]);
}

#[test]
fn a_failed_open_is_reported_once_and_not_retried() {
    let (source, hub) = hub();
    let lease = hub.connect().unwrap();
    let id = lease.client.id.clone();
    let nodes = || vec![entry("x", "https://broken.example", 1280)];
    let (_, reply) = hub.apply(set(&id, nodes()), None, |_| true);
    assert_eq!(reply.failed, ["x"]);
    let events = lease.client.drain();
    assert!(matches!(
        &events[..],
        [BrowserLiveEvent::Ended {
            reason: BrowserLiveEndReason::NoBrowser,
            ..
        }]
    ));
    hub.apply(set(&id, nodes()), None, |_| true);
    assert_eq!(
        source.opens.load(Ordering::SeqCst),
        1,
        "same spec, no retry"
    );
    hub.apply(set(&id, vec![]), None, |_| true);
    hub.apply(set(&id, nodes()), None, |_| true);
    assert_eq!(source.opens.load(Ordering::SeqCst), 2, "a new set retries");
}

#[test]
fn a_slow_client_gets_only_the_newest_frame() {
    let (source, hub) = hub();
    let lease = hub.connect().unwrap();
    let nodes = vec![entry("n", "https://example.com", 1280)];
    hub.apply(set(&lease.client.id, nodes), None, |_| true);
    let stream = source.stream("https://example.com").unwrap();
    // Three frames while the writer was busy elsewhere.
    for _ in 0..3 {
        stream.publish();
    }
    let events = lease.client.drain();
    let [BrowserLiveEvent::Frame {
        key,
        seq,
        width,
        height,
        jpeg_base64,
    }] = &events[..]
    else {
        panic!("one frame: {events:?}");
    };
    assert_eq!((key.as_str(), *seq), ("n", 3));
    assert_eq!((*width, *height), (4, 2));
    let jpeg = base64::engine::general_purpose::STANDARD
        .decode(jpeg_base64)
        .unwrap();
    assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    assert!(lease.client.drain().is_empty(), "nothing queued behind it");
}

#[test]
fn an_ended_stream_is_reported_and_removed() {
    let (source, hub) = hub();
    let lease = hub.connect().unwrap();
    let nodes = vec![entry("n", "https://example.com", 1280)];
    hub.apply(set(&lease.client.id, nodes), None, |_| true);
    source.stream("https://example.com").unwrap().end();
    let events = lease.client.drain();
    assert!(matches!(
        &events[..],
        [BrowserLiveEvent::Ended {
            reason: BrowserLiveEndReason::Failed,
            ..
        }]
    ));
    assert_eq!(source.alive.load(Ordering::SeqCst), 0);
}

#[test]
fn the_event_stream_says_hello_sends_frames_and_cleans_up_on_disconnect() {
    let (source, hub) = hub();
    let hub = Arc::new(hub);
    let pipe = Pipe::default();
    let writer = {
        let (hub, mut pipe) = (Arc::clone(&hub), pipe.clone());
        std::thread::spawn(move || {
            let lease = hub.connect().unwrap();
            lease.serve_events(&mut pipe, Some("*"))
        })
    };
    let events = pipe.wait_for(|events| !events.is_empty());
    let BrowserLiveEvent::Hello { client } = &events[0] else {
        panic!("hello first: {events:?}");
    };
    let head = String::from_utf8(lock(&pipe.data).clone()).unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream"));
    let nodes = vec![
        entry("a", "https://example.com/a", 1280),
        entry("b", "https://example.com/b", 1280),
    ];
    hub.apply(set(client, nodes), None, |_| true);
    source.stream("https://example.com/a").unwrap().publish();
    let events = pipe.wait_for(|events| {
        events
            .iter()
            .any(|e| matches!(e, BrowserLiveEvent::Frame { .. }))
    });
    assert!(events
        .iter()
        .any(|e| matches!(e, BrowserLiveEvent::Frame { key, .. } if key == "a")));
    assert_eq!(source.alive.load(Ordering::SeqCst), 2);

    // The client leaves; the next heartbeat write fails.
    pipe.cut.store(true, Ordering::SeqCst);
    assert!(writer.join().unwrap().is_err());
    assert_eq!(source.alive.load(Ordering::SeqCst), 0, "streams stopped");
    assert!(lock(&hub.clients).is_empty());
    let (status, _) = hub.apply(set(client, vec![]), None, |_| true);
    assert_eq!(status, "404 Not Found");
}

#[test]
fn clients_are_capped() {
    let (_, hub) = hub();
    let leases: Vec<_> = (0..MAX_CLIENTS).map(|_| hub.connect().unwrap()).collect();
    assert!(hub.connect().is_none());
    let ids: std::collections::HashSet<_> = leases.iter().map(|l| l.client.id.clone()).collect();
    assert_eq!(ids.len(), MAX_CLIENTS);
    assert!(ids.iter().all(|id| id.len() == 32));
    drop(leases);
    assert!(hub.connect().is_some());
}

#[test]
fn a_set_body_must_parse() {
    let (status, body) = LiveHub::shared().set_reply("nope", None);
    assert_eq!(status, "400 Bad Request");
    assert!(body.contains("\"ok\":false"), "{body}");
}

/// Streams a local animated page through the hub with the installed
/// browser: `cargo test -p de-host-services web_browser_live -- --ignored`.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_real_page_streams_at_the_web_cap_and_stops_with_its_client() {
    let dir = std::env::temp_dir().join(format!("de-live-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let page = dir.join("animated.html");
    std::fs::write(
        &page,
        "<html><body style='margin:0'><div style='width:100px;height:100px;\
         background:#3e6df5;animation:m 1s linear infinite alternate'></div>\
         <style>@keyframes m{from{margin-left:0}to{margin-left:300px}}</style></body></html>",
    )
    .unwrap();
    let pool = BrowserPool::new();
    let source = BrowserSource { pool: pool.clone() };
    let hub = Arc::new(LiveHub::new(Arc::new(source), Duration::from_millis(200)));
    let pipe = Pipe::default();
    let writer = {
        let (hub, mut pipe) = (Arc::clone(&hub), pipe.clone());
        std::thread::spawn(move || hub.connect().unwrap().serve_events(&mut pipe, None))
    };
    let events = pipe.wait_for(|events| !events.is_empty());
    let BrowserLiveEvent::Hello { client } = &events[0] else {
        panic!("hello first");
    };
    let url = format!("file://{}", page.display());
    let (_, reply) = hub.apply(set(client, vec![entry("n", &url, 400)]), None, |_| true);
    assert_eq!(reply.live, ["n"], "{reply:?}");
    std::thread::sleep(Duration::from_secs(3));
    let frames = pipe
        .events()
        .iter()
        .filter(|e| matches!(e, BrowserLiveEvent::Frame { .. }))
        .count();
    eprintln!("[perf] live channel: {frames} frames in 3 s at a {WEB_LIVE_MAX_FPS} fps cap");
    assert!(frames >= 6, "the page streams ({frames})");
    let cap = (3.0 * WEB_LIVE_MAX_FPS) as usize + 4;
    assert!(frames <= cap, "the web cap holds ({frames})");
    pipe.cut.store(true, Ordering::SeqCst);
    assert!(writer.join().unwrap().is_err());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !pool.shutdown_if_idle(Duration::ZERO) {
        assert!(Instant::now() < deadline, "the browser is released");
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = std::fs::remove_dir_all(dir);
}
