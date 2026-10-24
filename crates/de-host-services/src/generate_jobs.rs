//! Background jobs behind the MCP `generate` tool (pen.dev `Generate`).
//!
//! Every generation is asynchronous, as on pen.dev: the tool call only does
//! the bookkeeping (writes a `de:pending-image-…` fill) and returns; a worker
//! thread runs the provider with the user's own key, and the result lands in
//! the document later.
//!
//! A job never touches a document itself. It finishes into a completion
//! queue, and each host drains that queue into ordinary [`EditorCommand`]s at
//! a point where it owns the document:
//!
//! - the live desktop editor in `McpLiveServer::pump` (the job's listener
//!   wakes the UI thread),
//! - the file-backed `--mcp` / `--mcp-http` servers before each request (and
//!   each idle poll for http), and once more after a bounded wait for running
//!   jobs when the session ends,
//! - the `--serve-web` daemon before each MCP request and from the job's
//!   listener.
//!
//! Landings are computed against the document as it is at drain time (see
//! `landing.rs`), so a write the client made meanwhile is respected.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use de_editor_core::{EditorCommand, EditorState};

mod backend;
mod error;
pub(crate) mod landing;
mod profile;

pub use backend::{
    screen_custom_endpoints, GenerateBackend, ImageJobRequest, ProviderBackend, StockJobRequest,
};
pub use error::GenerateJobError;
pub use profile::{
    resolve_image_profile, GEMINI_FALLBACK_IMAGE_MODEL, OPENAI_FALLBACK_IMAGE_MODEL,
};

/// Prefix of the placeholder url a fill carries while its job runs
/// (pen.dev's `pencil:pending-image-…`, hence a scheme of its own rather than
/// the `placeholder://` sentinels). The canvas paints an unknown url as the
/// dashed image placeholder.
pub const PENDING_IMAGE_PREFIX: &str = "de:pending-image-";
/// Concurrent provider calls one process allows.
pub const MAX_RUNNING_JOBS: usize = 8;
/// Finished job records kept for `get_generation_status`.
const MAX_RECORDS: usize = 256;
/// How long a finished result waits for its pending url to appear in the
/// document (the host may drain before it applied the call's own write, or
/// the client may re-apply an undone one) before it is dropped.
const UNPLACED_TTL: Duration = Duration::from_secs(600);
/// Minimum gap between retries of results whose pending url was not placed.
const UNPLACED_RETRY_GAP: Duration = Duration::from_millis(500);
/// Results waiting for their pending url; the oldest go first past this.
const MAX_UNPLACED: usize = 32;

/// What a job produces, mirroring pen.dev's `Generate` types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerateKind {
    Ai,
    Stock,
}

impl GenerateKind {
    pub const ALL: [GenerateKind; 2] = [GenerateKind::Ai, GenerateKind::Stock];

    pub fn as_str(self) -> &'static str {
        match self {
            GenerateKind::Ai => "ai",
            GenerateKind::Stock => "stock",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == raw)
    }
}

/// Where a job is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Done,
    Failed(String),
}

/// One job as `get_generation_status` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    pub id: String,
    pub kind: GenerateKind,
    pub node_id: Option<String>,
    pub pending_url: Option<String>,
    pub status: JobStatus,
}

/// The provider work a job runs.
pub enum JobWork {
    Image(ImageJobRequest),
    Stock(StockJobRequest),
}

/// A started job's handle: its id and the pending url the caller writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobTicket {
    pub id: String,
    pub pending_url: String,
}

type Listener = Arc<dyn Fn() + Send + Sync>;

struct Completion {
    pending_url: String,
    result: Result<String, GenerateJobError>,
    finished_at: Instant,
}

/// A finished result on its way into the document, retried while its
/// pending url is not found.
struct Landing {
    pending_url: String,
    result: Result<String, ()>,
    finished_at: Instant,
}

#[derive(Default)]
struct Inner {
    counter: u64,
    running: usize,
    records: VecDeque<JobRecord>,
    completed: VecDeque<Completion>,
    unplaced: Vec<Landing>,
    last_unplaced_retry: Option<Instant>,
}

/// The process's generation jobs.
pub struct GenerateJobs {
    backend: Arc<dyn GenerateBackend>,
    inner: Mutex<Inner>,
    idle: Condvar,
    listener: Mutex<Option<Listener>>,
}

impl GenerateJobs {
    pub fn new(backend: Arc<dyn GenerateBackend>) -> Arc<Self> {
        Arc::new(Self {
            backend,
            inner: Mutex::new(Inner::default()),
            idle: Condvar::new(),
            listener: Mutex::new(None),
        })
    }

    /// The process-wide queue every host drains, backed by the real
    /// providers.
    pub fn global() -> &'static Arc<GenerateJobs> {
        static GLOBAL: OnceLock<Arc<GenerateJobs>> = OnceLock::new();
        GLOBAL.get_or_init(|| GenerateJobs::new(Arc::new(ProviderBackend)))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Called (from a worker thread) after a job finishes; the live editor
    /// wakes its UI thread with it, the web daemon lands the result.
    pub fn set_listener(&self, listener: Option<Listener>) {
        *self.listener.lock().unwrap_or_else(|p| p.into_inner()) = listener;
    }

    /// Reserve a job slot and mint its pending url. Fails when
    /// [`MAX_RUNNING_JOBS`] are still running.
    pub fn reserve(
        &self,
        kind: GenerateKind,
        node_id: Option<&str>,
    ) -> Result<JobTicket, GenerateJobError> {
        let mut inner = self.lock();
        if inner.running >= MAX_RUNNING_JOBS {
            return Err(GenerateJobError::Busy {
                limit: MAX_RUNNING_JOBS,
            });
        }
        inner.counter += 1;
        let id = format!("gen-{}-{}", process_nonce(), inner.counter);
        let pending_url = format!("{PENDING_IMAGE_PREFIX}{id}");
        inner.running += 1;
        inner.records.push_back(JobRecord {
            id: id.clone(),
            kind,
            node_id: node_id.map(str::to_string),
            pending_url: Some(pending_url.clone()),
            status: JobStatus::Pending,
        });
        while inner.records.len() > MAX_RECORDS {
            inner.records.pop_front();
        }
        Ok(JobTicket { id, pending_url })
    }

    /// Run `work` for a reserved ticket on its own worker thread. A panicking
    /// provider still releases its slot and reports the job failed.
    pub fn spawn(self: &Arc<Self>, ticket: JobTicket, work: JobWork) {
        let jobs = Arc::clone(self);
        let worker_ticket = ticket.clone();
        let spawned = std::thread::Builder::new()
            .name("de-generate-job".into())
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &work {
                        JobWork::Image(request) => jobs.backend.generate_image(request),
                        JobWork::Stock(request) => jobs.backend.stock_image(request),
                    }))
                    .unwrap_or(Err(GenerateJobError::Panicked));
                jobs.record_result(&worker_ticket, result);
                jobs.notify_listener();
            });
        if let Err(error) = spawned {
            // The caller may hold a host lock the listener takes (the web
            // daemon's state), so this synchronous path never calls it.
            self.record_result(&ticket, Err(GenerateJobError::Spawn(error.to_string())));
        }
    }

    /// Record a job's result and queue its landing, without calling the
    /// listener (safe while the caller holds a host lock).
    pub(crate) fn record_result(
        &self,
        ticket: &JobTicket,
        result: Result<String, GenerateJobError>,
    ) {
        {
            let mut inner = self.lock();
            inner.running = inner.running.saturating_sub(1);
            if let Some(record) = inner.records.iter_mut().find(|r| r.id == ticket.id) {
                record.status = match &result {
                    Ok(_) => JobStatus::Done,
                    Err(error) => JobStatus::Failed(error.to_string()),
                };
            }
            inner.completed.push_back(Completion {
                pending_url: ticket.pending_url.clone(),
                result,
                finished_at: Instant::now(),
            });
        }
        self.idle.notify_all();
    }

    /// Call the listener. Only from threads that hold no host lock.
    fn notify_listener(&self) {
        let listener = self
            .listener
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(listener) = listener {
            listener();
        }
    }

    /// Whether a drain is due: a finished job, or a parked result whose retry
    /// gap has passed (that retry may still find nothing). Cheap; the live
    /// editor calls it every frame.
    pub fn has_landings(&self) -> bool {
        let inner = self.lock();
        !inner.completed.is_empty()
            || (!inner.unplaced.is_empty()
                && inner
                    .last_unplaced_retry
                    .is_none_or(|at| at.elapsed() >= UNPLACED_RETRY_GAP))
    }

    /// Take every finished job and turn it into the writes that land it on
    /// `state`. A result whose pending url is nowhere in the document yet is
    /// kept and retried on later drains for a while.
    pub fn take_landings(&self, state: &EditorState) -> Vec<EditorCommand> {
        let (completed, unplaced) = {
            let mut inner = self.lock();
            let completed: Vec<Completion> = inner.completed.drain(..).collect();
            let retry_due = inner
                .last_unplaced_retry
                .is_none_or(|at| at.elapsed() >= UNPLACED_RETRY_GAP);
            let unplaced = if retry_due {
                inner.last_unplaced_retry = Some(Instant::now());
                std::mem::take(&mut inner.unplaced)
            } else {
                Vec::new()
            };
            (completed, unplaced)
        };
        let mut commands = Vec::new();
        let mut still_unplaced = Vec::new();
        let entries = completed
            .into_iter()
            .map(|completion| Landing {
                pending_url: completion.pending_url,
                result: completion.result.map_err(|_| ()),
                finished_at: completion.finished_at,
            })
            .chain(unplaced);
        for entry in entries {
            let landed = landing::resolve_pending_url(
                state,
                &entry.pending_url,
                entry.result.as_deref().map_err(|_| ()),
            );
            if landed.is_empty() {
                if entry.finished_at.elapsed() < UNPLACED_TTL {
                    still_unplaced.push(entry);
                }
            } else {
                commands.extend(landed);
            }
        }
        if !still_unplaced.is_empty() {
            {
                let mut inner = self.lock();
                inner.unplaced.extend(still_unplaced);
                let overflow = inner.unplaced.len().saturating_sub(MAX_UNPLACED);
                inner.unplaced.drain(..overflow);
            }
            // Drains that park results run at most once per retry gap, so
            // this schedules at most one wake per gap.
            self.schedule_retry_wake();
        }
        commands
    }

    /// Wake the host again once the retry gap has passed, so a parked result
    /// lands even when nothing else redraws the editor.
    fn schedule_retry_wake(&self) {
        let Some(listener) = self
            .listener
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
        else {
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("de-generate-retry".into())
            .spawn(move || {
                std::thread::sleep(UNPLACED_RETRY_GAP);
                listener();
            });
        if spawned.is_err() {
            eprintln!("design-editor mcp: could not schedule a generation retry");
        }
    }

    /// Block until no job is running or `timeout` passes. `true` = idle.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut inner = self.lock();
        while inner.running > 0 {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            inner = match self.idle.wait_timeout(inner, deadline - now) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        true
    }

    /// Whether any job is still running.
    pub fn any_running(&self) -> bool {
        self.lock().running > 0
    }

    /// Job records, newest last; one record when `id` is given.
    pub fn records(&self, id: Option<&str>) -> Vec<JobRecord> {
        self.lock()
            .records
            .iter()
            .filter(|record| id.is_none_or(|id| record.id == id))
            .cloned()
            .collect()
    }
}

/// Distinguishes this process's job ids from an earlier session's pending
/// urls still saved in a document.
fn process_nonce() -> &'static str {
    static NONCE: OnceLock<String> = OnceLock::new();
    NONCE.get_or_init(|| {
        let mut bytes = [0u8; 4];
        if getrandom::fill(&mut bytes).is_err() {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            bytes = nanos.to_le_bytes();
        }
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    })
}

/// Apply every landed generation to a document the caller owns, through
/// `apply`. Returns whether anything changed.
pub fn land_into<F>(jobs: &GenerateJobs, state: &mut EditorState, mut apply: F) -> bool
where
    F: FnMut(&mut EditorState, EditorCommand) -> bool,
{
    if !jobs.has_landings() {
        return false;
    }
    let mut changed = false;
    for command in jobs.take_landings(state) {
        changed |= apply(state, command);
    }
    changed
}

#[cfg(test)]
#[path = "generate_jobs_tests.rs"]
mod tests;
