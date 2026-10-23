//! In-app update job (#111): download the platform asset with live
//! progress, verify it against the release's `SHA256SUMS.txt`, and stage
//! the new build so a restart swaps it in.
//!
//! The job runs on its own thread and reports [`UpdatePhase`]s over a
//! channel; the runner drains them into `EditorUiState::update_modal`.
//! Platform staging and the relaunch helper live in
//! `update_install_platform.rs`.

use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use de_editor_core::{UpdateFailure, UpdatePhase};
use sha2::{Digest, Sha256};

pub use crate::update_install_platform::{apply_and_relaunch, StagedUpdate};
use crate::update_install_platform::{install_target, stage};

/// How often the worker reports transfer progress.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Window the speed readout averages over.
const SPEED_WINDOW_SECS: f64 = 1.0;

/// What the worker reports.
enum JobEvent {
    Phase(UpdatePhase),
    Staged(StagedUpdate),
}

/// Why the worker stopped early. Cancellation is not a failure the modal
/// shows: the user already closed it.
#[derive(Debug, PartialEq, Eq)]
enum JobError {
    Cancelled,
    Failed(UpdateFailure),
}

impl From<UpdateFailure> for JobError {
    fn from(failure: UpdateFailure) -> Self {
        JobError::Failed(failure)
    }
}

/// One running update attempt.
pub struct UpdateJob {
    rx: Option<Receiver<JobEvent>>,
    cancel: Arc<AtomicBool>,
}

/// What [`UpdateJob::poll`] drained this frame.
#[derive(Debug, Default)]
pub struct UpdatePoll {
    /// Latest phase reported since the last poll.
    pub phase: Option<UpdatePhase>,
    /// Set once, when the build is staged and ready for a restart.
    pub staged: Option<StagedUpdate>,
}

impl UpdateJob {
    pub fn spawn(version: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        std::thread::spawn(move || run(&version, &tx, &flag));
        Self {
            rx: Some(rx),
            cancel,
        }
    }

    /// Ask the worker to stop at its next chunk. It removes the partial
    /// download and reports nothing more.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Whether the worker may still report. The runner keeps waking the
    /// event loop while this holds so progress repaints without input.
    pub fn is_pending(&self) -> bool {
        self.rx.is_some()
    }

    /// Drain every queued event, keeping the newest phase.
    pub fn poll(&mut self) -> UpdatePoll {
        let mut out = UpdatePoll::default();
        let Some(rx) = self.rx.as_ref() else {
            return out;
        };
        loop {
            match rx.try_recv() {
                Ok(JobEvent::Phase(phase)) => out.phase = Some(phase),
                Ok(JobEvent::Staged(staged)) => out.staged = Some(staged),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    break;
                }
            }
        }
        out
    }
}

fn run(version: &str, tx: &Sender<JobEvent>, cancel: &AtomicBool) {
    match run_steps(version, tx, cancel) {
        Ok(staged) => {
            // Staged first: a poll landing between the two sends must never
            // show Restart without the build it restarts into.
            let _ = tx.send(JobEvent::Staged(staged));
            let _ = tx.send(JobEvent::Phase(UpdatePhase::ReadyToRestart));
        }
        Err(JobError::Failed(failure)) => {
            let _ = tx.send(JobEvent::Phase(UpdatePhase::Failed(failure)));
        }
        Err(JobError::Cancelled) => {}
    }
}

fn run_steps(
    version: &str,
    tx: &Sender<JobEvent>,
    cancel: &AtomicBool,
) -> Result<StagedUpdate, JobError> {
    // Refuse before spending bandwidth when this copy cannot replace itself.
    let target = install_target()?;
    let name =
        crate::update_check::installer_asset_name(version).ok_or(UpdateFailure::Unsupported)?;
    let base = crate::update_check::release_download_base(version);
    let sums = fetch_text(&format!("{base}/SHA256SUMS.txt"))?;
    let expected = expected_sha256(&sums, &name).ok_or(UpdateFailure::Checksum)?;

    // One folder per job: a cancelled worker still unwinding must not touch
    // the file the next attempt is writing.
    let dir = job_dir();
    std::fs::create_dir_all(&dir).map_err(|_| UpdateFailure::Install)?;
    let dest = dir.join(&name);
    let discard = || {
        let _ = std::fs::remove_dir_all(&dir);
    };
    let digest =
        download(&format!("{base}/{name}"), &dest, tx, cancel).inspect_err(|_| discard())?;

    let _ = tx.send(JobEvent::Phase(UpdatePhase::Verifying));
    if hex(&digest) != expected {
        discard();
        return Err(UpdateFailure::Checksum.into());
    }
    if cancel.load(Ordering::SeqCst) {
        discard();
        return Err(JobError::Cancelled);
    }

    let _ = tx.send(JobEvent::Phase(UpdatePhase::Installing));
    stage(&dest, target)
        .inspect_err(|_| discard())
        .map_err(JobError::from)
}

/// `<temp>/design-editor-update-<pid>-<n>`, unique per job in this process.
fn job_dir() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("design-editor-update-{}-{n}", std::process::id()))
}

fn client(timeout: Duration) -> Result<reqwest::Client, UpdateFailure> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(timeout)
        .user_agent(concat!("design-editor/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| UpdateFailure::Network)
}

fn fetch_text(url: &str) -> Result<String, UpdateFailure> {
    // Shared runtime, not a private one — see `update_check::fetch_latest_tag`.
    de_host_services::chat_runtime::block_on_anywhere(async {
        let resp = client(Duration::from_secs(30))?
            .get(url)
            .send()
            .await
            .map_err(|_| UpdateFailure::Network)?;
        if !resp.status().is_success() {
            return Err(UpdateFailure::Network);
        }
        resp.text().await.map_err(|_| UpdateFailure::Network)
    })
}

/// Stream `url` to `dest`, hashing as it goes, and report progress at most
/// every [`PROGRESS_INTERVAL`]. Returns the SHA-256 of the bytes written.
fn download(
    url: &str,
    dest: &Path,
    tx: &Sender<JobEvent>,
    cancel: &AtomicBool,
) -> Result<[u8; 32], JobError> {
    de_host_services::chat_runtime::block_on_anywhere(async {
        // Installers run tens of MB — allow a long transfer.
        let mut resp = client(Duration::from_secs(1800))?
            .get(url)
            .send()
            .await
            .map_err(|_| UpdateFailure::Network)?;
        if !resp.status().is_success() {
            return Err(UpdateFailure::Network.into());
        }
        let total = resp.content_length();
        let mut file = std::fs::File::create(dest).map_err(|_| UpdateFailure::Install)?;
        let mut hasher = Sha256::new();
        let mut received = 0u64;
        let started = Instant::now();
        let mut meter = SpeedMeter::default();
        let mut last_report: Option<Instant> = None;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(JobError::Cancelled);
            }
            let Some(chunk) = resp.chunk().await.map_err(|_| UpdateFailure::Network)? else {
                break;
            };
            file.write_all(&chunk).map_err(|_| UpdateFailure::Install)?;
            hasher.update(&chunk);
            received += chunk.len() as u64;
            let now = Instant::now();
            let bytes_per_sec = meter.sample(now.duration_since(started).as_secs_f64(), received);
            if last_report.is_none_or(|t| now.duration_since(t) >= PROGRESS_INTERVAL) {
                last_report = Some(now);
                let _ = tx.send(JobEvent::Phase(UpdatePhase::Downloading {
                    received,
                    total,
                    bytes_per_sec,
                }));
            }
        }
        file.flush().map_err(|_| UpdateFailure::Install)?;
        if total.is_some_and(|t| t != received) {
            return Err(UpdateFailure::Network.into());
        }
        Ok(hasher.finalize().into())
    })
}

/// Transfer speed over the last [`SPEED_WINDOW_SECS`], so the readout
/// follows the connection instead of the whole-download average.
#[derive(Debug, Default)]
struct SpeedMeter {
    samples: VecDeque<(f64, u64)>,
}

impl SpeedMeter {
    /// Record `received` bytes at `t` seconds; returns bytes per second.
    fn sample(&mut self, t: f64, received: u64) -> f64 {
        self.samples.push_back((t, received));
        while self
            .samples
            .front()
            .is_some_and(|(t0, _)| t - t0 > SPEED_WINDOW_SECS && self.samples.len() > 2)
        {
            self.samples.pop_front();
        }
        let (t0, r0) = self.samples[0];
        let dt = t - t0;
        if dt <= 0.0 {
            return 0.0;
        }
        (received - r0) as f64 / dt
    }
}

/// The hash `sha256sum` printed for `name`. Lines read `<hex>  <name>`, or
/// `<hex> *<name>` in binary mode.
fn expected_sha256(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        let file = file.trim_start().trim_start_matches('*');
        (file == name && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMS: &str = "\
0f343b0931126a20f133d67c2b018a3b0f343b0931126a20f133d67c2b018a3b  Design-Editor-0.2.1-arm64-mac.dmg
AAAABBBBCCCCDDDDEEEEFFFF00001111AAAABBBBCCCCDDDDEEEEFFFF00001111 *Design-Editor-0.2.1-x64-win-setup.exe
deadbeef  de-cli-windows-x86_64.zip
";

    #[test]
    fn checksum_list_lookup_matches_the_exact_name() {
        assert_eq!(
            expected_sha256(SUMS, "Design-Editor-0.2.1-arm64-mac.dmg").as_deref(),
            Some("0f343b0931126a20f133d67c2b018a3b0f343b0931126a20f133d67c2b018a3b")
        );
        // Binary-mode marker and upper-case hex are normalised.
        assert_eq!(
            expected_sha256(SUMS, "Design-Editor-0.2.1-x64-win-setup.exe").as_deref(),
            Some("aaaabbbbccccddddeeeeffff00001111aaaabbbbccccddddeeeeffff00001111")
        );
        assert_eq!(expected_sha256(SUMS, "Design-Editor-0.2.1"), None);
        // A malformed hash never verifies anything.
        assert_eq!(expected_sha256(SUMS, "de-cli-windows-x86_64.zip"), None);
    }

    #[test]
    fn hex_matches_sha256sum_output() {
        let digest: [u8; 32] = Sha256::digest(b"abc").into();
        assert_eq!(
            hex(&digest),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn speed_meter_tracks_the_recent_window() {
        let mut meter = SpeedMeter::default();
        assert_eq!(meter.sample(0.0, 0), 0.0);
        assert_eq!(meter.sample(0.5, 500_000), 1_000_000.0);
        assert_eq!(meter.sample(1.0, 1_000_000), 1_000_000.0);
        // The connection slows down: only the last second counts.
        meter.sample(2.0, 1_250_000);
        let speed = meter.sample(2.5, 1_375_000);
        assert!((speed - 250_000.0).abs() < 1.0, "{speed}");
    }
}
