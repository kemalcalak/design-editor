//! The single preview worker. Requests are coalesced per preview file and
//! debounced: the worker renders once a burst has gone quiet for
//! [`SETTLE`] (or after [`MAX_WAIT`]), so an agent writing a document over
//! `--mcp` renders it about once per pause, not once per write.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::{modified_time, preview_path, render_document_png, write_preview};

/// Quiet time that ends a burst of requests.
const SETTLE: Duration = Duration::from_millis(300);
/// Longest a burst may hold a render back.
const MAX_WAIT: Duration = Duration::from_secs(2);

struct Job {
    doc_path: PathBuf,
    target: PathBuf,
}

type Notifier = Arc<dyn Fn() + Send + Sync>;

/// Bumped each time the worker writes a preview.
static WRITTEN: AtomicU64 = AtomicU64::new(0);
/// Requests queued but not yet handled (tests wait on it).
static PENDING: AtomicUsize = AtomicUsize::new(0);

fn notifier() -> &'static Mutex<Option<Notifier>> {
    static NOTIFIER: OnceLock<Mutex<Option<Notifier>>> = OnceLock::new();
    NOTIFIER.get_or_init(|| Mutex::new(None))
}

/// How many previews the worker has written in this process.
pub fn written_count() -> u64 {
    WRITTEN.load(Ordering::Relaxed)
}

/// Called (on the worker thread) after each preview is written, so an idle
/// host can wake up and show it.
pub fn set_written_notifier(notify: impl Fn() + Send + Sync + 'static) {
    if let Ok(mut slot) = notifier().lock() {
        *slot = Some(Arc::new(notify));
    }
}

/// Queue a preview of the saved `.op` file at `doc_path` in the user's
/// preview folder. Best-effort: a failure only means the card keeps its
/// glyph.
pub fn queue(doc_path: &Path) {
    if let Some(dir) = super::previews_dir() {
        queue_into(&dir, doc_path);
    }
}

/// [`queue`] into an explicit folder.
pub fn queue_into(dir: &Path, doc_path: &Path) {
    let job = Job {
        doc_path: doc_path.to_path_buf(),
        target: preview_path(dir, doc_path),
    };
    PENDING.fetch_add(1, Ordering::SeqCst);
    let sent = worker().lock().is_ok_and(|tx| tx.send(job).is_ok());
    if !sent {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        eprintln!("[home-preview] worker unavailable");
    }
}

/// Block until every queued request is handled, or `timeout` passes.
/// Returns whether the worker went idle. For tests.
#[doc(hidden)]
pub fn wait_idle(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while PENDING.load(Ordering::SeqCst) > 0 {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
}

fn worker() -> &'static Mutex<Sender<Job>> {
    static WORKER: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("home-preview".into())
            .spawn(move || run(rx));
        if let Err(error) = spawned {
            eprintln!("[home-preview] worker: {error}");
        }
        Mutex::new(tx)
    })
}

fn run(rx: Receiver<Job>) {
    while let Ok(first) = rx.recv() {
        let (jobs, received) = collect_burst(&rx, first);
        for job in jobs.into_values() {
            // A document that panics the loader must not end the worker
            // (and with it every later preview of the session).
            let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(&job)));
            match rendered {
                Ok(Ok(())) => {
                    WRITTEN.fetch_add(1, Ordering::Relaxed);
                    let notify = notifier().lock().ok().and_then(|slot| slot.clone());
                    if let Some(notify) = notify {
                        notify();
                    }
                }
                Ok(Err(error)) => {
                    eprintln!("[home-preview] {}: {error}", job.doc_path.display());
                }
                Err(_) => {
                    eprintln!("[home-preview] {}: render panicked", job.doc_path.display());
                }
            }
        }
        PENDING.fetch_sub(received, Ordering::SeqCst);
    }
}

/// Gather requests until the burst goes quiet, one job per preview file.
/// Returns the jobs and how many requests they absorbed.
fn collect_burst(rx: &Receiver<Job>, first: Job) -> (HashMap<PathBuf, Job>, usize) {
    let deadline = Instant::now() + MAX_WAIT;
    let mut received = 1;
    let mut jobs = HashMap::from([(first.target.clone(), first)]);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match rx.recv_timeout(SETTLE.min(left)) {
            Ok(job) => {
                received += 1;
                jobs.insert(job.target.clone(), job);
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }
    (jobs, received)
}

fn render(job: &Job) -> Result<(), super::HomePreviewError> {
    // Read the time before the content, so an edit landing during the
    // render leaves the preview stale rather than falsely fresh.
    let doc_modified = modified_time(&job.doc_path);
    let bytes = render_document_png(&job.doc_path)?;
    write_preview(&job.target, &bytes, doc_modified)
}
