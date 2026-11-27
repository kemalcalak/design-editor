//! What every live connection shares (#406): the state lock with its
//! holder, whether the UI thread is known not to be answering, and the kit
//! catalog `tools/list` is answered from.
//!
//! The lock used to be a bare `Mutex<()>` taken with a blocking `lock()`,
//! so a request queued behind a slow one waited for as long as it took,
//! and nothing said which request held the editor. It now records its
//! holder, waits a bounded time (then the request is answered busy, naming
//! the holder), and `server_loop` runs [`LiveShared::watchdog`] so a hold
//! past [`LOCK_HOLD_WARN`] is logged with its tool and request number.

use std::path::{Path, PathBuf};
use std::sync::{Condvar, MutexGuard};

use de_mcp::element_tools::KitToolCatalog;

use super::*;

/// How long a read waits for the UI thread (snapshot, page list) before
/// it is answered busy. A responsive editor answers within a frame; the
/// UI thread only misses this while it opens or paints a large document.
pub(super) const UI_BUSY_WAIT: Duration = Duration::from_secs(2);
/// How long a request waits for another one to release the state lock.
/// Under it sits one other request's snapshot copy and dispatch; past it
/// the client is told who holds the editor and to retry.
pub(super) const STATE_LOCK_WAIT: Duration = Duration::from_secs(10);
/// The wait once a read already found the UI thread unresponsive and it
/// has not pumped since: long enough for a UI that is back to answer,
/// short enough that a call during a long document load is told so at
/// once instead of after another [`UI_BUSY_WAIT`].
pub(super) const UI_STALLED_WAIT: Duration = Duration::from_millis(250);
/// The retry hint busy answers carry.
pub(super) const BUSY_RETRY_AFTER: Duration = Duration::from_secs(2);
/// A lock hold longer than this is logged by the watchdog, once.
pub(super) const LOCK_HOLD_WARN: Duration = Duration::from_secs(30);

pub(super) struct LiveShared {
    /// The state lock: `Some` while a request holds it, naming that
    /// request. Waiters sleep on `released` instead of polling.
    holder: Mutex<Option<LockHolder>>,
    released: Condvar,
    /// Set when a read gave up on the UI thread; cleared by the next pump.
    /// While set, reads wait only [`UI_STALLED_WAIT`] instead of each one
    /// waiting [`UI_BUSY_WAIT`] again.
    ui_stalled: AtomicBool,
    /// What `tools/list` describes: the built-in kits until the host
    /// publishes the editor's own ([`LiveUiSide::pumped`]).
    kit_catalog: Mutex<Arc<KitToolCatalog>>,
    /// The file the open document is bound to, as the host last said
    /// ([`LiveUiSide::document_path`]): where pen.dev's pathless `save`
    /// writes (#392).
    document_path: Mutex<Option<PathBuf>>,
    next_request: AtomicU64,
    /// "Ask before destructive agent actions", as the pump last saw it, so a
    /// connection skips classifying and asking while it is off (#398).
    asks_permission: AtomicBool,
}

impl Default for LiveShared {
    fn default() -> Self {
        Self {
            holder: Mutex::new(None),
            released: Condvar::new(),
            ui_stalled: AtomicBool::new(false),
            kit_catalog: Mutex::new(Arc::new(KitToolCatalog::builtin().clone())),
            document_path: Mutex::new(None),
            next_request: AtomicU64::new(0),
            asks_permission: AtomicBool::new(false),
        }
    }
}

struct LockHolder {
    label: String,
    request: u64,
    since: Instant,
    reported: bool,
}

/// The state lock, held. Releases it (and logs a long hold) on drop, which
/// also runs when the holding connection panics.
pub(super) struct StateGuard<'a> {
    shared: &'a LiveShared,
}

impl Drop for StateGuard<'_> {
    fn drop(&mut self) {
        let holder = lock_ignoring_poison(&self.shared.holder).take();
        self.shared.released.notify_one();
        if let Some(holder) = holder.filter(|holder| holder.reported) {
            eprintln!(
                "design-editor mcp: {} (request #{}) released the editor after {:.1} s",
                holder.label,
                holder.request,
                holder.since.elapsed().as_secs_f64()
            );
        }
    }
}

impl LiveShared {
    /// Take the state lock for `label` (a tool or route name), waiting at
    /// most [`STATE_LOCK_WAIT`].
    pub(super) fn lock_state(&self, label: &str) -> Result<StateGuard<'_>, McpLiveError> {
        self.lock_state_within(label, STATE_LOCK_WAIT)
    }

    /// [`Self::lock_state`] with its wait as a parameter (tests).
    pub(super) fn lock_state_within(
        &self,
        label: &str,
        wait: Duration,
    ) -> Result<StateGuard<'_>, McpLiveError> {
        let holder = lock_ignoring_poison(&self.holder);
        // A panicked sibling connection must not wedge the server: its
        // guard released the lock while unwinding, so poison carries no
        // half-written state here.
        let (mut holder, _) = self
            .released
            .wait_timeout_while(holder, wait, |holder| holder.is_some())
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(current) = holder.as_ref() {
            return Err(McpLiveError::Busy(EditorBusy::StateLock {
                holder: Some(current.label.clone()),
                held_for: current.since.elapsed(),
            }));
        }
        *holder = Some(LockHolder {
            label: label.to_string(),
            request: self.next_request.fetch_add(1, Ordering::Relaxed) + 1,
            since: Instant::now(),
            reported: false,
        });
        Ok(StateGuard { shared: self })
    }

    /// Log a hold past [`LOCK_HOLD_WARN`], once per hold. Cheap enough for
    /// the accept loop to call on every turn.
    pub(super) fn watchdog(&self) {
        self.watchdog_after(LOCK_HOLD_WARN);
    }

    /// [`Self::watchdog`] with its threshold as a parameter (tests). Returns
    /// whether it logged.
    pub(super) fn watchdog_after(&self, threshold: Duration) -> bool {
        let Ok(mut holder) = self.holder.try_lock() else {
            return false;
        };
        let Some(holder) = holder.as_mut() else {
            return false;
        };
        let held = holder.since.elapsed();
        if holder.reported || held < threshold {
            return false;
        }
        holder.reported = true;
        eprintln!(
            "design-editor mcp: {} (request #{}) has held the editor for {:.1} s; other MCP requests wait for it or are answered busy",
            holder.label,
            holder.request,
            held.as_secs_f64()
        );
        true
    }

    /// How long a read may wait for the UI thread now.
    pub(super) fn ui_wait(&self, normal: Duration) -> Duration {
        if self.ui_stalled() {
            normal.min(UI_STALLED_WAIT)
        } else {
            normal
        }
    }

    /// Whether a read gave up on the UI thread and it has not pumped since.
    pub(super) fn ui_stalled(&self) -> bool {
        self.ui_stalled.load(Ordering::Acquire)
    }

    pub(super) fn mark_ui_stalled(&self) {
        self.ui_stalled.store(true, Ordering::Release);
    }

    pub(super) fn kit_catalog(&self) -> Arc<KitToolCatalog> {
        Arc::clone(&lock_ignoring_poison(&self.kit_catalog))
    }

    /// Whether destructive calls ask the user first (see
    /// [`LiveUiSide::publish_permission_setting`]).
    pub(super) fn asks_permission(&self) -> bool {
        self.asks_permission.load(Ordering::Acquire)
    }

    pub(super) fn document_path(&self) -> Option<PathBuf> {
        lock_ignoring_poison(&self.document_path).clone()
    }
}

/// The UI thread's side of [`LiveShared`], kept on `McpLiveServer`.
pub(super) struct LiveUiSide {
    shared: Arc<LiveShared>,
    /// The catalog last published, compared against the editor's kits
    /// without taking a lock.
    published_kits: Arc<KitToolCatalog>,
}

impl LiveUiSide {
    pub(super) fn new(shared: Arc<LiveShared>) -> Self {
        let published_kits = shared.kit_catalog();
        Self {
            shared,
            published_kits,
        }
    }

    /// The UI thread is pumping, so reads may wait their full time again;
    /// publish the editor's kit catalog for `tools/list` when it changed
    /// (a kit imported or removed), an allocation-free compare otherwise.
    pub(super) fn pumped(&mut self, kits: &[de_editor_core::UIKit]) {
        self.shared.ui_stalled.store(false, Ordering::Release);
        if self.published_kits.matches(kits) {
            return;
        }
        let catalog = Arc::new(KitToolCatalog::of(kits));
        *lock_ignoring_poison(&self.shared.kit_catalog) = Arc::clone(&catalog);
        self.published_kits = catalog;
    }

    /// Publish the "ask before destructive agent actions" setting for the
    /// connection threads.
    pub(super) fn publish_permission_setting(&mut self, on: bool) {
        self.shared.asks_permission.store(on, Ordering::Release);
    }

    /// Publish the file the open document is bound to; a no-op when it did
    /// not change, so the host may call it every pump.
    pub(super) fn document_path(&mut self, path: Option<&Path>) {
        let mut current = lock_ignoring_poison(&self.shared.document_path);
        if current.as_deref() != path {
            *current = path.map(Path::to_path_buf);
        }
    }

    #[cfg(test)]
    pub(super) fn shared(&self) -> &Arc<LiveShared> {
        &self.shared
    }
}

impl Default for LiveUiSide {
    fn default() -> Self {
        Self::new(Arc::new(LiveShared::default()))
    }
}

fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}
