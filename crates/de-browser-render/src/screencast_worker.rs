//! The thread behind a [`crate::PageStream`]: it owns the stream's CDP
//! connection, reads `Page.screencastFrame` events, hands them to the
//! shared slot and acks them, applies restarts, and on the way out stops
//! the screencast, closes (or detaches from) the tab and releases its
//! holds.
//!
//! Pacing, as measured against Chrome: every frame of one screencast
//! carries the same `sessionId`, Chrome keeps up to three frames unacked
//! and drops repaints beyond that. So the worker acks **one** frame per
//! `1 / max_fps`, and only once the consumer took the last frame handed
//! out (Chrome then encodes about one frame per ack), and hands out at
//! most one frame per interval (the newest; extras in the burst after a
//! restart are replaced). A repaint Chrome dropped while frames were in
//! flight could leave the page's final state unseen, so once the page goes
//! quiet after such a stretch the screencast is restarted once, which
//! sends the current picture.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};

use crate::cdp_client::{CdpClient, READ_SLICE};
use crate::page_session::TabStreamClaim;
use crate::screencast::{FrameMetadata, StreamSettings, StreamShared, STREAM_READ_SLICE};
use crate::tab::{Tab, Viewport};
use crate::{BrowserRenderError, PoolLease};

/// How long the page must send nothing, after frames may have been
/// dropped, before the screencast is restarted for its current picture.
const QUIET_REFRESH: Duration = Duration::from_millis(300);
/// Frames Chrome sends unacked before it drops repaints (measured).
const CHROME_UNACKED_LIMIT: usize = 3;

/// Whose tab the stream runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabMode {
    /// Its own: closed when the stream ends.
    Owned,
    /// A kept-open MCP tab: only this connection's session ends.
    Shared,
}

/// A message from the handle.
pub(crate) enum Control {
    /// Stop the screencast, apply what is given and start it again.
    Restart {
        viewport: Option<Viewport>,
        settings: Option<StreamSettings>,
        reply: Sender<Result<(), BrowserRenderError>>,
    },
}

/// What the stream keeps alive while it runs.
pub(crate) struct Holds {
    /// The shared browser (none in tests against a fake peer).
    pub(crate) lease: Option<PoolLease>,
    /// The kept-open tab's idle close, for a shared tab.
    pub(crate) session: Option<TabStreamClaim>,
}

pub(crate) struct Worker {
    client: CdpClient,
    tab: Tab,
    mode: TabMode,
    settings: StreamSettings,
    shared: Arc<StreamShared>,
    inbox: Receiver<Control>,
    holds: Holds,
    /// Frames received and not acked yet (CDP screencast session ids),
    /// oldest first.
    owed: VecDeque<i64>,
    last_ack: Option<Instant>,
    /// The newest frame not handed out yet (rate-capped).
    pending: Option<(Vec<u8>, FrameMetadata)>,
    last_publish: Option<Instant>,
    last_frame_at: Instant,
    /// Chrome had (nearly) its in-flight limit unacked, so it may have
    /// dropped a repaint since.
    maybe_dropped: bool,
    /// The connection's read slice now: short only while a frame waits to
    /// be taken or acked, so an idle stream wakes 5 times a second.
    slice: Duration,
}

impl Worker {
    /// Focus-emulate the tab and start the screencast; on failure the tab
    /// is closed (or detached) and the holds released.
    pub(crate) fn start(
        client: CdpClient,
        tab: Tab,
        mode: TabMode,
        settings: StreamSettings,
        shared: Arc<StreamShared>,
        inbox: Receiver<Control>,
        holds: Holds,
    ) -> Result<Self, BrowserRenderError> {
        let mut worker = Self {
            client,
            tab,
            mode,
            settings,
            shared,
            inbox,
            holds,
            owed: VecDeque::new(),
            last_ack: None,
            pending: None,
            last_publish: None,
            last_frame_at: Instant::now(),
            maybe_dropped: false,
            slice: READ_SLICE,
        };
        match worker.begin() {
            Ok(()) => Ok(worker),
            Err(error) => {
                worker.finish(error.needs_restart());
                Err(error)
            }
        }
    }

    fn begin(&mut self) -> Result<(), BrowserRenderError> {
        // A headless tab is never focused; without this, pages that pause
        // on blur (carousels, focus-gated animation) stand still.
        self.tab.command(
            &mut self.client,
            "Emulation.setFocusEmulationEnabled",
            json!({ "enabled": true }),
        )?;
        self.start_screencast()
    }

    fn start_screencast(&mut self) -> Result<(), BrowserRenderError> {
        self.tab
            .command(
                &mut self.client,
                "Page.startScreencast",
                self.settings.start_params(),
            )
            .map(drop)
    }

    /// Run until the handle is gone or the stream fails, then clean up.
    pub(crate) fn run(mut self) {
        let error = self.pump().err();
        let broken = error
            .as_ref()
            .is_some_and(BrowserRenderError::needs_restart);
        // Before `finish` drops the inbox, so a restart waiting on its
        // reply reads the real reason.
        self.shared.end(error);
        self.finish(broken);
    }

    fn pump(&mut self) -> Result<(), BrowserRenderError> {
        loop {
            match self.inbox.try_recv() {
                Ok(Control::Restart {
                    viewport,
                    settings,
                    reply,
                }) => match self.restart(viewport, settings) {
                    Ok(applied) => {
                        let _ = reply.send(applied);
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error.duplicate()));
                        return Err(error);
                    }
                },
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
            let busy = self.pending.is_some() || !self.owed.is_empty() || self.maybe_dropped;
            let slice = if busy { STREAM_READ_SLICE } else { READ_SLICE };
            if slice != self.slice {
                self.client.set_read_slice(slice)?;
                self.slice = slice;
            }
            let now = Instant::now();
            let mut wake = now + slice;
            if let Some(due) = self.publish_due(now) {
                if due <= now {
                    self.publish();
                } else {
                    wake = wake.min(due);
                }
            }
            if let Some(due) = self.ack_due(now) {
                if due <= now {
                    self.ack_one()?;
                } else {
                    wake = wake.min(due);
                }
            }
            if let Some(due) = self.refresh_due() {
                if due <= now {
                    self.refresh()?;
                } else {
                    wake = wake.min(due);
                }
            }
            if let Some(event) = self.client.next_event(wake)? {
                self.handle(&event)?;
            }
        }
    }

    /// When the pending frame may be handed out: once the consumer took
    /// the last one, and no sooner than the rate cap after it.
    fn publish_due(&self, now: Instant) -> Option<Instant> {
        if self.pending.is_none() || !self.shared.is_taken() {
            return None;
        }
        Some(due_after(
            self.last_publish,
            self.settings.min_interval(),
            now,
        ))
    }

    fn publish(&mut self) {
        if let Some((jpeg, metadata)) = self.pending.take() {
            self.shared.publish(jpeg, metadata);
            self.last_publish = Some(Instant::now());
        }
    }

    /// When the oldest owed frame may be acked: once every frame received
    /// was handed out and taken, and no sooner than the rate cap after the
    /// previous ack. A consumer that does not take frames stops the page.
    fn ack_due(&self, now: Instant) -> Option<Instant> {
        if self.owed.is_empty() || self.pending.is_some() || !self.shared.is_taken() {
            return None;
        }
        Some(due_after(self.last_ack, self.settings.min_interval(), now))
    }

    fn ack_one(&mut self) -> Result<(), BrowserRenderError> {
        let Some(id) = self.owed.pop_front() else {
            return Ok(());
        };
        let acked = self.tab.command(
            &mut self.client,
            "Page.screencastFrameAck",
            json!({ "sessionId": id }),
        );
        match acked {
            Err(error) if error.needs_restart() => return Err(error),
            // A stale id after a restart is refused; nothing to undo.
            _ => {}
        }
        self.last_ack = Some(Instant::now());
        Ok(())
    }

    /// When to fetch the current picture after frames may have been
    /// dropped: everything acked and taken, and the page quiet since.
    fn refresh_due(&self) -> Option<Instant> {
        let settled = self.owed.is_empty() && self.pending.is_none();
        (self.maybe_dropped && settled).then(|| self.last_frame_at + QUIET_REFRESH)
    }

    fn refresh(&mut self) -> Result<(), BrowserRenderError> {
        self.maybe_dropped = false;
        self.restart(None, None).map(drop)
    }

    fn handle(&mut self, event: &Value) -> Result<(), BrowserRenderError> {
        let method = event.get("method").and_then(Value::as_str).unwrap_or("");
        let params = event.get("params").unwrap_or(&Value::Null);
        let ours = event.get("sessionId").and_then(Value::as_str) == Some(self.tab.session());
        match method {
            "Page.screencastFrame" if ours => {
                self.on_frame(params);
                Ok(())
            }
            "Inspector.detached" | "Inspector.targetCrashed" if ours => {
                Err(BrowserRenderError::TabClosed)
            }
            // Sent on the browser session: our session on the tab ended
            // (the MCP tab was closed, or the target went away).
            "Target.detachedFromTarget"
                if params.get("sessionId").and_then(Value::as_str) == Some(self.tab.session()) =>
            {
                Err(BrowserRenderError::TabClosed)
            }
            _ => Ok(()),
        }
    }

    fn on_frame(&mut self, params: &Value) {
        let Some(id) = params.get("sessionId").and_then(Value::as_i64) else {
            return;
        };
        self.owed.push_back(id);
        self.last_frame_at = Instant::now();
        // One short of Chrome's limit: the next repaint may be dropped.
        if self.owed.len() >= CHROME_UNACKED_LIMIT - 1 {
            self.maybe_dropped = true;
        }
        let jpeg = params
            .get("data")
            .and_then(Value::as_str)
            .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok());
        // An undecodable frame is still acked; the newest frame wins.
        if let Some(jpeg) = jpeg {
            let metadata = FrameMetadata::from_json(params.get("metadata").unwrap_or(&Value::Null));
            self.pending = Some((jpeg, metadata));
        }
    }

    /// Stop the screencast, apply what is given and start it again. The
    /// outer error ends the stream (it could not be started again, so it
    /// would otherwise stand silent); the inner one, a viewport the tab
    /// refused, is only the caller's.
    fn restart(
        &mut self,
        viewport: Option<Viewport>,
        settings: Option<StreamSettings>,
    ) -> Result<Result<(), BrowserRenderError>, BrowserRenderError> {
        match self
            .tab
            .command(&mut self.client, "Page.stopScreencast", json!({}))
        {
            Err(error) if error.needs_restart() => return Err(error),
            _ => {}
        }
        // Frames of the old screencast parked while the stop ran would be
        // handed out (at the old size) and owe acks Chrome refuses.
        self.client
            .drop_method_events(self.tab.session(), "Page.screencastFrame");
        // Chrome forgets in-flight frames on a new start.
        self.owed.clear();
        self.last_ack = None;
        let applied = match viewport {
            Some(viewport) => self.tab.emulate(&mut self.client, &viewport),
            None => Ok(()),
        };
        if let (Ok(()), Some(settings)) = (&applied, settings) {
            self.settings = settings;
        }
        self.start_screencast()?;
        Ok(applied)
    }

    /// Stop the screencast and close or detach the tab (skipped when the
    /// connection broke), then release the holds; a broken connection
    /// also ends the browser it was on.
    fn finish(self, broken: bool) {
        let Self {
            mut client,
            tab,
            mode,
            holds,
            ..
        } = self;
        if !broken {
            let _ = tab.command(&mut client, "Page.stopScreencast", json!({}));
            match mode {
                TabMode::Owned => tab.close(&mut client),
                TabMode::Shared => tab.detach(&mut client),
            }
        }
        let Holds { lease, session } = holds;
        // Releasing the lease (a drop) is what lets the browser idle out.
        if let Some(mut lease) = lease {
            if broken {
                lease.mark_broken();
            }
            drop(lease);
        }
        drop(session);
    }
}

/// `since` plus `interval`, or `now` when nothing happened yet. `now` is
/// the instant the pump loop compares the result against: reading the
/// clock afresh here would give a time just past it, so a first frame (or
/// the first ack after a start) would never be due and the loop would spin
/// on an already-passed read deadline (#324, Linux CI).
fn due_after(since: Option<Instant>, interval: Duration, now: Instant) -> Instant {
    since.map_or(now, |at| at + interval)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_yet_is_due_at_the_loops_own_now() {
        let now = Instant::now();
        let interval = Duration::from_millis(100);
        assert_eq!(due_after(None, interval, now), now);
        assert_eq!(due_after(Some(now), interval, now), now + interval);
    }
}
