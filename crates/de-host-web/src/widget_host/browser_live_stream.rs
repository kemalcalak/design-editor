//! Live browser pages in the web editor (#324): the selected browser nodes
//! play their page as it runs, streamed by the serve-web daemon.
//!
//! The shared `BrowserStreamQueue` (web caps: `browser_stream::stream_tier`)
//! is pumped once per repaint, right after the still-snapshot queue
//! (`browser_snapshot_fetch`), and its commands go to the transport-free
//! [`LiveClientState`]. This module is the transport: ONE `EventSource` on
//! the daemon's `/api/browser/live` for every live node (the browser's
//! per-origin connection limit is not spent per node), opened only once
//! something streams; the `hello` event names this client, and the whole
//! live set is POSTed whenever it changes (a failed call is retried). A
//! broken stream is closed and reopened with backoff
//! (`browser_live_client::reconnect_delay_ms`); the next hello re-sends the
//! set. Once nothing streams and the daemon holds the empty set, the stream
//! closes, so an idle tab holds none of the daemon's client slots.
//!
//! Each `frame` event's JPEG is decoded by the browser off the main thread
//! (`createImageBitmap`); the CanvasKit host installs the bitmap under the
//! node's stable texture id (`browser_textures::live_texture`) before its
//! next paint (`canvaskit::live_frames`). One decode per node runs at a
//! time and only the newest frame waits, so a slow decode drops frames.
//!
//! Hidden or unfocused, the tab posts an empty set at once (no paint
//! needs to run for that, which a hidden tab would not) and drops frames;
//! the queue stops the streams with their last frame kept, as on desktop.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use base64::Engine as _;
use de_editor_core::browser_live_wire::{BrowserLiveEvent, BROWSER_LIVE_PATH};
use de_editor_core::browser_stream::StreamHost;
use de_editor_ui::widgets::browser_stream_queue::{BrowserStreamCommand, BrowserStreamQueue};
use de_editor_ui::widgets::browser_textures;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::browser_live_client::{FrameJob, LiveClientState};
use super::WidgetHost;

/// A decoded frame waiting for the next paint.
struct LandedFrame {
    node_id: String,
    seq: u64,
    /// The `ImageBitmap`; whoever takes it closes it.
    bitmap: JsValue,
}

/// The event stream and the callbacks it holds.
struct Connection {
    source: web_sys::EventSource,
    /// Not closed yet (by an error or as idle). A closed connection stays
    /// here until the next connect replaces it, so its callbacks are never
    /// dropped while one of them runs.
    alive: bool,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
    _on_error: Closure<dyn FnMut(web_sys::Event)>,
}

thread_local! {
    static QUEUE: RefCell<BrowserStreamQueue> =
        RefCell::new(BrowserStreamQueue::new(StreamHost::Web));
    static CLIENT: RefCell<LiveClientState> = RefCell::new(LiveClientState::default());
    static CONNECTION: RefCell<Option<Connection>> = const { RefCell::new(None) };
    /// A reconnect timer is armed.
    static RECONNECT_PENDING: Cell<bool> = const { Cell::new(false) };
    /// A timer to resend a refused or failed set call is armed.
    static POST_RETRY_PENDING: Cell<bool> = const { Cell::new(false) };
    /// Newest decoded frame per node, for the next paint.
    static LANDED: RefCell<BTreeMap<String, LandedFrame>> = const { RefCell::new(BTreeMap::new()) };
    /// Nodes whose live image the CanvasKit cache holds, with its texture.
    static INSTALLED: RefCell<BTreeMap<String, u64>> = const { RefCell::new(BTreeMap::new()) };
    /// The focus and visibility listeners are installed.
    static LISTENING: Cell<bool> = const { Cell::new(false) };
}

/// Per-repaint step, after the snapshot queue's pump: start, change and
/// stop streams for what the frame showed. Returns whether a live picture
/// was dropped (the canvas repaints with the still).
pub(crate) fn pump(host: &WidgetHost, dpi: f32) -> bool {
    ensure_listeners();
    let active = page_active();
    let state = host.editor_state();
    let commands = super::browser_snapshot_fetch::with_queue(|snapshots| {
        QUEUE.with(|queue| queue.borrow_mut().pump(state, snapshots, dpi, active))
    });
    let mut changed = false;
    let live = &state.editor_ui.browser_live;
    if !live.live_pages() || !live.pages_allowed() {
        // Kept last frames give way to the stills too.
        changed |= !browser_textures::forget_kept_live().is_empty();
    }
    CLIENT.with(|client| {
        let mut client = client.borrow_mut();
        for command in commands {
            if let BrowserStreamCommand::Stop {
                node_id,
                keep_frame,
            } = &command
            {
                if *keep_frame {
                    browser_textures::end_live(node_id);
                } else {
                    changed |= browser_textures::forget_live(node_id).is_some();
                }
            }
            client.apply(command);
        }
        client.set_active(active);
    });
    sync();
    changed
}

/// Before a paint: hand each frame decoded since the last one whose
/// stream still runs to `install` (texture id, bitmap; it takes the
/// bitmap, true once installed) and mark it as the node's live frame.
pub(crate) fn install_landed(mut install: impl FnMut(u64, &JsValue) -> bool) {
    let landed = LANDED.with(|landed| std::mem::take(&mut *landed.borrow_mut()));
    for frame in landed.into_values() {
        let current = CLIENT.with(|client| client.borrow().streams_node(&frame.node_id));
        if !current {
            close_bitmap(&frame.bitmap);
            continue;
        }
        let texture = browser_textures::live_texture(&frame.node_id);
        if install(texture, &frame.bitmap) {
            browser_textures::set_live_frame(&frame.node_id, frame.seq);
            INSTALLED.with(|installed| installed.borrow_mut().insert(frame.node_id, texture));
        }
    }
}

/// Where page input for `node_id` goes: this client's id and the node's
/// live key, once the daemon streams it (`browser_input_post`).
pub(crate) fn input_target(node_id: &str) -> Option<(String, String)> {
    CLIENT.with(|client| client.borrow().input_target(node_id))
}

/// Live images whose node no longer shows a live picture (forgotten, or
/// replaced by a newer still): the host frees them.
pub(crate) fn textures_to_release() -> Vec<u64> {
    INSTALLED.with(|installed| {
        let mut released = Vec::new();
        installed.borrow_mut().retain(|node_id, texture| {
            let shown = browser_textures::live_frame(node_id).is_some();
            if !shown {
                released.push(*texture);
            }
            shown
        });
        released
    })
}

/// Visible and focused, like the desktop window's focus.
fn page_active() -> bool {
    web_sys::window()
        .and_then(|window| window.document())
        .is_none_or(|document| !document.hidden() && document.has_focus().unwrap_or(true))
}

/// Pause or resume as soon as the tab hides, shows, blurs or focuses.
fn ensure_listeners() {
    if LISTENING.with(|listening| listening.replace(true)) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let on_change = Closure::<dyn FnMut()>::new(|| {
        let active = page_active();
        CLIENT.with(|client| client.borrow_mut().set_active(active));
        sync();
        crate::repaint_coalescer::request();
    });
    let callback = on_change.as_ref().unchecked_ref();
    let _ = window.add_event_listener_with_callback("focus", callback);
    let _ = window.add_event_listener_with_callback("blur", callback);
    if let Some(document) = window.document() {
        let _ = document.add_event_listener_with_callback("visibilitychange", callback);
    }
    // Lives for the page.
    on_change.forget();
}

/// Open the event stream when something streams, send the set when the
/// daemon's copy is out of date, and close the stream once nothing streams
/// and the daemon holds the empty set (an idle tab keeps none of the
/// daemon's client slots).
fn sync() {
    let (wants, idle) = CLIENT.with(|client| {
        let client = client.borrow();
        (client.wants_connection(), client.idle())
    });
    let alive = CONNECTION.with(|connection| connection.borrow().as_ref().is_some_and(|c| c.alive));
    if wants && !alive && !RECONNECT_PENDING.with(Cell::get) {
        connect();
    } else if !wants && alive && idle {
        CONNECTION.with(|connection| {
            if let Some(connection) = connection.borrow_mut().as_mut() {
                close(connection);
            }
        });
        CLIENT.with(|client| client.borrow_mut().on_closed());
    }
    if let Some(set) = CLIENT.with(|client| client.borrow_mut().next_post()) {
        post_set(&set);
    }
}

/// Close `connection`'s stream; its callbacks stay alive (see `alive`).
/// Returns whether it was still alive.
fn close(connection: &mut Connection) -> bool {
    connection.source.set_onmessage(None);
    connection.source.set_onerror(None);
    connection.source.close();
    std::mem::replace(&mut connection.alive, false)
}

/// Run `f` after `delay_ms`; false when no timer could be armed.
pub(crate) fn after(delay_ms: u32, f: impl FnOnce() + 'static) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let callback = Closure::once_into_js(f);
    let delay = i32::try_from(delay_ms).unwrap_or(i32::MAX);
    window
        .set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), delay)
        .is_ok()
}

fn connect() {
    let url = crate::daemon_base::daemon_url(BROWSER_LIVE_PATH);
    let Ok(source) = web_sys::EventSource::new(&url) else {
        disconnected();
        return;
    };
    let on_message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(|event: web_sys::MessageEvent| {
            if let Some(text) = event.data().as_string() {
                on_event(&text);
            }
        });
    let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(|_event: web_sys::Event| {
        // The browser would retry on its own as a NEW client; close and
        // retry with backoff instead.
        let was_alive =
            CONNECTION.with(|connection| connection.borrow_mut().as_mut().is_some_and(close));
        if was_alive {
            disconnected();
        }
    });
    source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    source.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    let previous = CONNECTION.with(|connection| {
        connection.borrow_mut().replace(Connection {
            source,
            alive: true,
            _on_message: on_message,
            _on_error: on_error,
        })
    });
    if let Some(mut previous) = previous {
        close(&mut previous);
    }
}

/// The stream broke or never opened: retry after the backoff delay.
fn disconnected() {
    let delay = CLIENT.with(|client| client.borrow_mut().on_disconnect());
    let armed = after(delay, || {
        RECONNECT_PENDING.with(|pending| pending.set(false));
        sync();
    });
    RECONNECT_PENDING.with(|pending| pending.set(armed));
}

fn on_event(text: &str) {
    let Ok(event) = serde_json::from_str::<BrowserLiveEvent>(text) else {
        return;
    };
    match event {
        BrowserLiveEvent::Hello { client } => {
            CLIENT.with(|state| state.borrow_mut().on_hello(client));
            sync();
        }
        BrowserLiveEvent::Frame {
            key,
            seq,
            jpeg_base64,
            ..
        } => {
            let unpack = || {
                base64::engine::general_purpose::STANDARD
                    .decode(jpeg_base64)
                    .ok()
            };
            let job = CLIENT.with(|state| state.borrow_mut().on_frame(&key, seq, unpack));
            if let Some(job) = job {
                decode(job);
            }
        }
        // The node keeps its still (blocked address, no browser, a failed
        // page); the daemon does not retry until the set changes it.
        BrowserLiveEvent::Blocked { .. } | BrowserLiveEvent::Ended { .. } => {}
    }
}

/// A refused or failed set call is sent again after this long (a hidden
/// tab paints nothing that would resend it).
const POST_RETRY_MS: u32 = 1_000;

/// POST the live set; the answer lets the next change go out.
fn post_set(set: &de_editor_core::browser_live_wire::BrowserLiveSet) {
    let url = crate::daemon_base::daemon_url(BROWSER_LIVE_PATH);
    let on_reply: std::rc::Rc<dyn Fn(u16, String)> =
        std::rc::Rc::new(|status, _body| post_settled(status == 200));
    let sent = serde_json::to_string(set)
        .is_ok_and(|body| crate::live_sync::post_json_with_status(&url, &body, on_reply));
    if !sent {
        post_settled(false);
    }
}

fn post_settled(ok: bool) {
    CLIENT.with(|client| client.borrow_mut().post_done(ok));
    if ok {
        // A change made while this call was out.
        sync();
    } else if !POST_RETRY_PENDING.with(Cell::get) {
        let armed = after(POST_RETRY_MS, || {
            POST_RETRY_PENDING.with(|pending| pending.set(false));
            sync();
        });
        POST_RETRY_PENDING.with(|pending| pending.set(armed));
    }
}

/// Decode `job`'s JPEG with the browser, off the main thread.
fn decode(mut job: FrameJob) {
    // The bytes go to the Blob; the rest of the job comes back with the
    // bitmap.
    let jpeg = std::mem::take(&mut job.jpeg);
    let promise = (|| {
        let window = web_sys::window()?;
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(jpeg.as_slice()));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("image/jpeg");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).ok()?;
        window.create_image_bitmap_with_blob(&blob).ok()
    })();
    drop(jpeg);
    let Some(promise) = promise else {
        decoded(job, None);
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        let bitmap = wasm_bindgen_futures::JsFuture::from(promise).await.ok();
        decoded(job, bitmap);
    });
}

/// A decode finished: keep the bitmap for the next paint (newest wins)
/// and start the frame that waited.
fn decoded(job: FrameJob, bitmap: Option<JsValue>) {
    let (current, next) = CLIENT.with(|client| {
        let mut client = client.borrow_mut();
        let current = client.is_current(&job.node_id, &job.key);
        (current, client.decode_done(&job))
    });
    if let Some(bitmap) = bitmap {
        if current {
            let frame = LandedFrame {
                node_id: job.node_id.clone(),
                seq: job.seq,
                bitmap,
            };
            let older =
                LANDED.with(|landed| landed.borrow_mut().insert(job.node_id.clone(), frame));
            if let Some(older) = older {
                close_bitmap(&older.bitmap);
            }
            crate::repaint_coalescer::request();
        } else {
            close_bitmap(&bitmap);
        }
    }
    if let Some(next) = next {
        decode(next);
    }
}

/// Free a bitmap nobody will draw.
fn close_bitmap(bitmap: &JsValue) {
    let close = js_sys::Reflect::get(bitmap, &JsValue::from_str("close"))
        .ok()
        .and_then(|close| close.dyn_into::<js_sys::Function>().ok());
    if let Some(close) = close {
        let _ = close.call0(bitmap);
    }
}
