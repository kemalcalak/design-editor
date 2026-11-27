//! Keeps `de_editor_core::memory_reading` current for the Settings ▸
//! System memory row and the `debug_memory_report` tool (#456).
//!
//! Runs from `new_events`, never from paint. The process footprint and the
//! host's cache lines are cheap (no line walks the document), so they are
//! taken on the UI thread: every [`VISIBLE_PERIOD`] while the System tab
//! shows the row, else at most every [`HIDDEN_PERIOD`] when the loop is
//! awake anyway (no wake of its own). The undo history's estimate walks
//! every shared node (seconds on #412's document), so a worker thread does
//! it from `Arc` clones of the history's documents, at most every
//! [`HISTORY_PERIOD`] and only while the row is on screen.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use de_editor_core::memory_reading;
use de_editor_core::memory_report::{self, CacheUsage};
use de_editor_core::process_memory;
use de_host_native::{NativeBackend, WidgetHostNative};

/// Refresh period while the memory row is on screen.
pub(crate) const VISIBLE_PERIOD: Duration = Duration::from_secs(2);
/// Refresh period otherwise (for the debug tool), only on wakes that
/// happen anyway.
const HIDDEN_PERIOD: Duration = Duration::from_secs(15);
/// Least time between two history walks.
const HISTORY_PERIOD: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(crate) struct MemoryRefresh {
    last_caches: Option<Instant>,
    last_history: Option<Instant>,
    history_job: Option<Receiver<CacheUsage>>,
}

impl MemoryRefresh {
    /// Publish what is due. `true` when the row on screen changed, so the
    /// caller repaints.
    pub(crate) fn tick(&mut self, host: &WidgetHostNative, backend: Option<&NativeBackend>) -> bool {
        let visible = host.settings_system_tab_open();
        let mut changed = self.drain_history_job() && visible;
        let period = if visible {
            VISIBLE_PERIOD
        } else {
            HIDDEN_PERIOD
        };
        let due = self.last_caches.is_none_or(|at| at.elapsed() >= period);
        if due {
            if let Some(backend) = backend {
                memory_reading::publish_caches(
                    process_memory::current(),
                    host.cache_report(backend),
                );
                self.last_caches = Some(Instant::now());
                changed |= visible;
            }
        }
        let history_due = self.last_history.is_none_or(|at| at.elapsed() >= HISTORY_PERIOD);
        if visible && history_due && self.history_job.is_none() {
            self.start_history_job(host);
        }
        changed
    }

    /// When the loop should wake for the next refresh: only while the row
    /// is on screen.
    pub(crate) fn next_wake(&self, host: &WidgetHostNative) -> Option<Instant> {
        if !host.settings_system_tab_open() {
            return None;
        }
        let next = self
            .last_caches
            .map_or_else(Instant::now, |at| at + VISIBLE_PERIOD);
        // Poll a running history walk sooner.
        Some(if self.history_job.is_some() {
            next.min(Instant::now() + Duration::from_millis(250))
        } else {
            next
        })
    }

    fn start_history_job(&mut self, host: &WidgetHostNative) {
        let docs = host.history_docs();
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("memory-history".into())
            .spawn(move || {
                let _ = tx.send(memory_report::history_usage_of(&docs));
            });
        if spawned.is_ok() {
            self.history_job = Some(rx);
            self.last_history = Some(Instant::now());
        }
    }

    /// Publish a finished history walk; `true` when one landed.
    fn drain_history_job(&mut self) -> bool {
        let Some(job) = &self.history_job else {
            return false;
        };
        match job.try_recv() {
            Ok(usage) => {
                memory_reading::publish_history(usage);
                self.history_job = None;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.history_job = None;
                false
            }
        }
    }
}
