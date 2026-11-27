//! Keeps `de_editor_core::memory_reading` current for the Settings ▸
//! System memory row and the `debug_memory_report` tool (#456).
//!
//! Runs from `new_events`, never from paint. The process footprint and the
//! host's cache lines are cheap (no line walks the document), so they are
//! taken on the UI thread: every [`VISIBLE_PERIOD`] while the System tab
//! shows the row in a visible window, else at most every [`HIDDEN_PERIOD`]
//! when the loop is awake anyway (no wake of its own). The undo history's
//! estimate walks every shared node (seconds on #412's document), so a
//! worker thread does it from `Arc` clones of the history's top-level
//! nodes, only while the row is on screen and at least [`HISTORY_PERIOD`]
//! after the last walk ended. A new document drops the old estimate.
//!
//! The timing decisions take the clock as an argument
//! ([`MemoryRefresh::plan`], [`MemoryRefresh::next_wake`]), so the tests
//! drive them without sleeping.

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
/// Least time from the end of one history walk to the start of the next.
const HISTORY_PERIOD: Duration = Duration::from_secs(10);
/// How often a running history walk is polled.
const HISTORY_POLL: Duration = Duration::from_millis(250);

/// What the refresher may do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct View {
    /// Settings ▸ System, which shows the row, is open.
    pub row_open: bool,
    /// The window is hidden or minimized: nothing is on screen.
    pub window_hidden: bool,
    /// A render backend exists, so the cache lines can be read.
    pub can_measure: bool,
}

impl View {
    fn row_on_screen(self) -> bool {
        self.row_open && !self.window_hidden
    }
}

/// What one tick does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Plan {
    pub publish_caches: bool,
    pub start_history: bool,
}

#[derive(Default)]
pub(crate) struct MemoryRefresh {
    last_caches: Option<Instant>,
    /// When the last history walk ended.
    last_history_end: Option<Instant>,
    history_job: Option<Receiver<CacheUsage>>,
    /// The document the published history estimate describes.
    document: Option<u64>,
}

impl MemoryRefresh {
    /// Publish what is due. `true` when the row on screen changed, so the
    /// caller repaints.
    pub(crate) fn tick(
        &mut self,
        now: Instant,
        host: &WidgetHostNative,
        backend: Option<&NativeBackend>,
        window_hidden: bool,
    ) -> bool {
        let view = view(host, backend.is_some(), window_hidden);
        let before = memory_reading::generation();
        self.follow_document(host.document_generation());
        self.drain_history_job(now);
        let plan = self.plan(now, view);
        if let (true, Some(backend)) = (plan.publish_caches, backend) {
            memory_reading::publish_caches(process_memory::current(), host.cache_report(backend));
            self.last_caches = Some(now);
        }
        if plan.start_history {
            self.start_history_job(host);
        }
        view.row_on_screen() && memory_reading::generation() != before
    }

    /// The loop's next wake for the row: none unless the row is on screen
    /// and something can be measured.
    pub(crate) fn wake(
        &self,
        now: Instant,
        host: &WidgetHostNative,
        has_backend: bool,
        window_hidden: bool,
    ) -> Option<Instant> {
        self.next_wake(now, view(host, has_backend, window_hidden))
    }

    /// What is due at `now`.
    pub(crate) fn plan(&self, now: Instant, view: View) -> Plan {
        let period = if view.row_on_screen() {
            VISIBLE_PERIOD
        } else {
            HIDDEN_PERIOD
        };
        let publish_caches =
            view.can_measure && self.last_caches.is_none_or(|at| at + period <= now);
        let start_history = view.row_on_screen()
            && self.history_job.is_none()
            && self
                .last_history_end
                .is_none_or(|end| end + HISTORY_PERIOD <= now);
        Plan {
            publish_caches,
            start_history,
        }
    }

    /// When the next tick is due while the row is on screen.
    pub(crate) fn next_wake(&self, now: Instant, view: View) -> Option<Instant> {
        if !view.row_on_screen() || !view.can_measure {
            return None;
        }
        let caches = self.last_caches.map_or(now, |at| at + VISIBLE_PERIOD);
        let history = if self.history_job.is_some() {
            now + HISTORY_POLL
        } else {
            self.last_history_end
                .map_or(now, |end| end + HISTORY_PERIOD)
        };
        Some(caches.min(history).max(now))
    }

    /// A new document: its history is not the one the estimate walked.
    fn follow_document(&mut self, generation: u64) {
        if self.document == Some(generation) {
            return;
        }
        if self.document.is_some() {
            memory_reading::clear_history();
            self.history_job = None;
            self.last_history_end = None;
        }
        self.document = Some(generation);
    }

    fn start_history_job(&mut self, host: &WidgetHostNative) {
        let steps = host.history_nodes();
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("memory-history".into())
            .spawn(move || {
                let _ = tx.send(memory_report::history_usage_of(&steps));
            });
        if spawned.is_ok() {
            self.history_job = Some(rx);
        }
    }

    /// Publish a finished history walk and note when it ended.
    fn drain_history_job(&mut self, now: Instant) {
        let Some(job) = &self.history_job else {
            return;
        };
        match job.try_recv() {
            Ok(usage) => {
                memory_reading::publish_history(usage);
                self.history_job = None;
                self.last_history_end = Some(now);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.history_job = None;
                self.last_history_end = Some(now);
            }
        }
    }
}

fn view(host: &WidgetHostNative, can_measure: bool, window_hidden: bool) -> View {
    View {
        row_open: host.settings_system_tab_open(),
        window_hidden,
        can_measure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON_SCREEN: View = View {
        row_open: true,
        window_hidden: false,
        can_measure: true,
    };

    #[test]
    fn the_row_on_screen_refreshes_every_two_seconds() {
        let t0 = Instant::now();
        let mut refresh = MemoryRefresh::default();
        assert!(refresh.plan(t0, ON_SCREEN).publish_caches, "first tick");
        refresh.last_caches = Some(t0);
        assert!(
            !refresh
                .plan(t0 + Duration::from_secs(1), ON_SCREEN)
                .publish_caches
        );
        assert!(refresh.plan(t0 + VISIBLE_PERIOD, ON_SCREEN).publish_caches);
        assert_eq!(
            refresh.next_wake(t0, ON_SCREEN),
            Some(t0),
            "history due now"
        );
        refresh.last_history_end = Some(t0);
        assert_eq!(refresh.next_wake(t0, ON_SCREEN), Some(t0 + VISIBLE_PERIOD));
    }

    #[test]
    fn without_the_row_it_refreshes_rarely_and_never_wakes() {
        let t0 = Instant::now();
        let closed = View {
            row_open: false,
            ..ON_SCREEN
        };
        let mut refresh = MemoryRefresh {
            last_caches: Some(t0),
            ..MemoryRefresh::default()
        };
        assert!(!refresh.plan(t0 + VISIBLE_PERIOD, closed).publish_caches);
        assert!(refresh.plan(t0 + HIDDEN_PERIOD, closed).publish_caches);
        assert!(!refresh.plan(t0 + HIDDEN_PERIOD, closed).start_history);
        assert_eq!(refresh.next_wake(t0, closed), None);
        refresh.last_caches = None;
        assert_eq!(refresh.next_wake(t0, closed), None);
    }

    #[test]
    fn a_hidden_window_neither_wakes_nor_walks_the_history() {
        let t0 = Instant::now();
        let hidden = View {
            window_hidden: true,
            ..ON_SCREEN
        };
        let refresh = MemoryRefresh::default();
        assert_eq!(refresh.next_wake(t0, hidden), None);
        assert!(!refresh.plan(t0, hidden).start_history);
    }

    #[test]
    fn without_a_backend_nothing_is_measured_or_woken_for() {
        let t0 = Instant::now();
        let no_backend = View {
            can_measure: false,
            ..ON_SCREEN
        };
        let refresh = MemoryRefresh::default();
        assert!(!refresh.plan(t0, no_backend).publish_caches);
        assert_eq!(refresh.next_wake(t0, no_backend), None, "no busy wake");
    }

    #[test]
    fn the_history_interval_counts_from_the_end_of_a_walk() {
        let t0 = Instant::now();
        let ended = t0 + Duration::from_secs(4);
        let refresh = MemoryRefresh {
            last_caches: Some(ended),
            last_history_end: Some(ended),
            ..MemoryRefresh::default()
        };
        assert!(!refresh.plan(t0 + HISTORY_PERIOD, ON_SCREEN).start_history);
        assert!(
            refresh
                .plan(ended + HISTORY_PERIOD, ON_SCREEN)
                .start_history
        );
    }
}
