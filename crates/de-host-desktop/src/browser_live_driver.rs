//! One streaming browser node's thread (#324): opens its
//! `de_browser_render::PageStream`, follows the queue's updates, decodes
//! each JPEG and hands the raster to the UI thread. Everything that can
//! block (page loads, screencast restarts, decodes) runs here, never on
//! the UI thread.
//!
//! The stream shares the MCP `browser` tool's kept-open tab for the node
//! when there is one (the canvas then shows the page the agent drives) and
//! otherwise opens a tab of its own; an owned stream moves onto the MCP
//! tab once the agent opens one, so a node never has two tabs.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use de_browser_render::{BrowserRenderError, PageSessions, PageStream, StreamSettings, Viewport};
use de_editor_core::browser_stream::{node_page_session_key, BrowserStreamSpec};
use skia_safe::ConditionallySend;

use super::{DriverArgs, LiveFrame};
use crate::DesktopEvent;

/// A change the UI thread sends a running driver. Dropping the sender
/// stops the driver.
pub(crate) enum DriverControl {
    Update {
        spec: BrowserStreamSpec,
        /// The page must load again (new address or Reload).
        reload: bool,
    },
    /// The UI installed the last frame: the next one may follow at once.
    Taken,
}

/// Longest wait for a frame before the driver looks at its inbox again.
const FRAME_WAIT: Duration = Duration::from_millis(100);
/// How often an owned stream checks whether the MCP tool opened a tab for
/// its node, and how long a shared stream waits for a busy tab.
const SHARE_CHECK: Duration = Duration::from_millis(500);
/// Pause before reopening a stream that ended or failed to open.
const RETRY: Duration = Duration::from_secs(2);

pub(crate) fn spawn(args: DriverArgs) {
    let spawned = thread::Builder::new()
        .name("browser-live".into())
        .spawn(move || run(args));
    if let Err(error) = spawned {
        eprintln!("[browser-live] spawn failed: {error}");
    }
}

/// The layout of the MCP tab kept for the node: 1× like the tool's own
/// calls (`browser_tool::node_viewport`), which would undo any other scale.
fn viewport(spec: &BrowserStreamSpec) -> Viewport {
    Viewport {
        width: spec.layout_width,
        height: spec.layout_height,
        scale: 1.0,
        mobile: spec.mobile,
    }
}

/// The layout of a tab of the stream's own: at the spec's device scale, so
/// a selected node zoomed in gets frames sharper than 1×.
fn own_viewport(spec: &BrowserStreamSpec) -> Viewport {
    Viewport {
        scale: spec.page_scale,
        ..viewport(spec)
    }
}

fn settings(spec: &BrowserStreamSpec) -> StreamSettings {
    StreamSettings {
        max_width: spec.frame_width,
        max_height: spec.frame_height,
        max_fps: spec.max_fps,
        ..StreamSettings::default()
    }
}

/// How the driver loop ends.
enum Exit {
    /// The UI dropped the stream.
    Stopped,
    /// The stream ended or must be reopened (new page, tab moved).
    Reopen,
}

fn run(mut args: DriverArgs) {
    de_browser_render::start_idle_reaper();
    let key = node_page_session_key(&args.node_id);
    loop {
        let stream = match open(&args, &key) {
            Ok(Some(stream)) => stream,
            Ok(None) => return,
            Err(error) => {
                // A page that does not open is not retried until the
                // node changes (the snapshot shows why it failed).
                eprintln!("[browser-live] {}: {error}", args.node_id);
                loop {
                    match args.control.recv() {
                        Ok(DriverControl::Update { spec, .. }) => {
                            args.spec = spec;
                            break;
                        }
                        Ok(DriverControl::Taken) => {}
                        Err(_) => return,
                    }
                }
                continue;
            }
        };
        let exit = stream_frames(&mut args, &key, &stream);
        // Waits for the tab to close (or detach) on this thread.
        stream.stop();
        match exit {
            Exit::Stopped => return,
            Exit::Reopen => {}
        }
    }
}

/// Open the node's stream: on the MCP tab when one is kept for the node,
/// else on a tab of its own. `Ok(None)` when the UI stopped the driver
/// meanwhile.
fn open(args: &DriverArgs, key: &str) -> Result<Option<PageStream>, BrowserRenderError> {
    let sessions = PageSessions::shared();
    while sessions.is_kept(key) {
        if let Some(stream) = sessions.stream(key, settings(&args.spec))? {
            let shared = stream.is_shared_tab();
            debug_assert!(shared);
            // The node's address or size may have moved since the agent
            // last drove the tab.
            let viewport = viewport(&args.spec);
            let url = args.spec.url.clone();
            let loaded = sessions.with_kept(key, viewport, |session| {
                if session.url() != Some(url.as_str()) {
                    session.load(&url)?;
                }
                Ok(())
            });
            if let Err(error) = loaded.and_then(|()| stream.restart()) {
                eprintln!("[browser-live] {}: {error}", args.node_id);
            }
            return Ok(Some(stream));
        }
        // A call holds the tab right now: try again shortly.
        match args.control.recv_timeout(SHARE_CHECK) {
            Err(RecvTimeoutError::Disconnected) => return Ok(None),
            _ => continue,
        }
    }
    PageStream::open(
        &args.pool,
        &args.spec.url,
        own_viewport(&args.spec),
        settings(&args.spec),
    )
    .map(Some)
}

/// Hand frames to the UI until the stream must stop or reopen.
fn stream_frames(args: &mut DriverArgs, key: &str, stream: &PageStream) -> Exit {
    let mut share_checked = Instant::now();
    loop {
        match args.control.try_recv() {
            Ok(DriverControl::Update { spec, reload }) => {
                if let Some(exit) = apply_update(args, key, stream, spec, reload) {
                    return exit;
                }
            }
            Ok(DriverControl::Taken) | Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return Exit::Stopped,
        }
        if !stream.is_shared_tab()
            && share_checked.elapsed() >= SHARE_CHECK
            && PageSessions::shared().is_kept(key)
        {
            // The agent opened a tab for this node: show that one.
            return Exit::Reopen;
        }
        if share_checked.elapsed() >= SHARE_CHECK {
            share_checked = Instant::now();
        }
        if !args.taken.load(Ordering::Acquire) {
            // The UI has not installed the last frame yet; it sends
            // `Taken` once it has (the next frame waits unacked meanwhile).
            match args.control.recv_timeout(FRAME_WAIT) {
                Ok(DriverControl::Update { spec, reload }) => {
                    if let Some(exit) = apply_update(args, key, stream, spec, reload) {
                        return exit;
                    }
                }
                Ok(DriverControl::Taken) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Exit::Stopped,
            }
            continue;
        }
        match stream.wait_frame(FRAME_WAIT) {
            Ok(Some(frame)) => {
                let Some(image) = decode(&frame.jpeg) else {
                    continue;
                };
                args.taken.store(false, Ordering::Release);
                let sent = args.frames.send(LiveFrame {
                    node_id: args.node_id.clone(),
                    stream: args.stream,
                    seq: frame.seq,
                    image,
                });
                if sent.is_err() {
                    return Exit::Stopped;
                }
                wake(args);
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!("[browser-live] {} stream ended: {error}", args.node_id);
                return if wait_for_change(args, RETRY) {
                    Exit::Reopen
                } else {
                    Exit::Stopped
                };
            }
        }
    }
}

/// Apply a queue update to the running stream. `Some` when the stream
/// must reopen instead (or the UI stopped it meanwhile).
fn apply_update(
    args: &mut DriverArgs,
    key: &str,
    stream: &PageStream,
    spec: BrowserStreamSpec,
    reload: bool,
) -> Option<Exit> {
    let old = std::mem::replace(&mut args.spec, spec);
    let spec = &args.spec;
    let result = if stream.is_shared_tab() {
        if reload {
            let url = spec.url.clone();
            PageSessions::shared()
                .with_kept(key, viewport(spec), |session| session.load(&url))
                .and_then(|()| stream.restart())
        } else if !old.same_page(spec) {
            PageSessions::shared().resize_stream(key, viewport(spec), stream)
        } else {
            Ok(())
        }
    } else if reload {
        // An owned tab loads the new page in a fresh stream.
        return Some(Exit::Reopen);
    } else if !old.same_page(spec) || old.page_scale != spec.page_scale {
        stream.set_viewport(own_viewport(spec))
    } else {
        Ok(())
    };
    let result = result.and_then(|()| {
        if settings(&old) != settings(spec) {
            stream.set_settings(settings(spec))
        } else {
            Ok(())
        }
    });
    match result {
        Ok(()) => None,
        Err(error) => {
            eprintln!("[browser-live] {}: {error}", args.node_id);
            Some(Exit::Reopen)
        }
    }
}

/// Wait up to `delay` for an update (applied to `args.spec`). Returns
/// false when the UI stopped the driver.
fn wait_for_change(args: &mut DriverArgs, delay: Duration) -> bool {
    let deadline = Instant::now() + delay;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match args.control.recv_timeout(left) {
            Ok(DriverControl::Update { spec, .. }) => {
                args.spec = spec;
                return true;
            }
            Ok(DriverControl::Taken) => {}
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

fn decode(jpeg: &[u8]) -> Option<skia_safe::Sendable<skia_safe::Image>> {
    let (image, _) = de_host_native::decode_raster_capped(jpeg, u32::MAX)?;
    image.wrap_send().ok()
}

fn wake(args: &DriverArgs) {
    if let Some(proxy) = args.wake.lock().ok().and_then(|wake| wake.clone()) {
        let _ = proxy.send_event(DesktopEvent::BrowserFrameReady);
    }
}
