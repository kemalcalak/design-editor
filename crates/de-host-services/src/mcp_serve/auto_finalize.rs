//! Idle/session-end finalization for file-backed MCP sessions.

use std::time::{Duration, Instant};

/// Nap between non-blocking accept attempts while no client is connected.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(250);

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use de_editor_core::{EditorState, PenNodeExt};
use de_orchestrator::repair_summary::RepairSummary;

use super::*;

const AUTO_FINALIZE_ENV: &str = "DESIGN_EDITOR_MCP_AUTO_FINALIZE";
const AUTO_FINALIZE_IDLE_SECS_ENV: &str = "DESIGN_EDITOR_MCP_AUTO_FINALIZE_IDLE_SECS";
const DEFAULT_IDLE_AFTER: Duration = Duration::from_secs(45);

/// Tracks the last file-backed MCP write and prevents repeated finalization
/// of the same document revision.
pub struct AutoFinalize {
    last_write_at: Option<Instant>,
    finalized_at_revision: Option<u64>,
    idle_after: Duration,
    /// Top-level roots this session's writes created: the only roots
    /// auto-finalize may rewrite (#107). Everything else on the page was
    /// there before the session and stays as authored.
    session_roots: BTreeSet<String>,
}

impl AutoFinalize {
    #[cfg(test)]
    pub(crate) fn for_test(idle_after: Duration) -> Self {
        Self {
            last_write_at: None,
            finalized_at_revision: None,
            idle_after,
            session_roots: BTreeSet::new(),
        }
    }

    /// Read the file-mode auto-finalize switch and idle threshold.
    /// `DESIGN_EDITOR_MCP_AUTO_FINALIZE=0` disables the feature. Invalid or
    /// missing threshold values use the 45-second default.
    pub fn from_env() -> Self {
        let disabled = std::env::var(AUTO_FINALIZE_ENV)
            .map(|value| value.trim() == "0")
            .unwrap_or(false);
        let idle_after = if disabled {
            Duration::MAX
        } else {
            std::env::var(AUTO_FINALIZE_IDLE_SECS_ENV)
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .map(Duration::from_secs)
                .unwrap_or(DEFAULT_IDLE_AFTER)
        };
        Self {
            last_write_at: None,
            finalized_at_revision: None,
            idle_after,
            session_roots: BTreeSet::new(),
        }
    }

    pub(crate) fn note_write(&mut self, now: Instant, scope: &WriteScope) {
        self.last_write_at = Some(now);
        self.session_roots.extend(scope.new_roots.iter().cloned());
    }

    pub fn due(&self, now: Instant, current_revision: u64) -> bool {
        self.enabled()
            && !self.session_roots.is_empty()
            && self.last_write_at.is_some_and(|written| {
                now.checked_duration_since(written)
                    .is_some_and(|idle| idle >= self.idle_after)
            })
            && self.finalized_at_revision != Some(current_revision)
    }

    /// Mark a successful explicit `finalize_design` MCP call so the session
    /// close/idle trigger does not repeat it for the same revision.
    /// The document revision the most recent finalize ran against, if any.
    #[cfg(test)]
    pub(crate) fn last_finalized_revision(&self) -> Option<u64> {
        self.finalized_at_revision
    }

    pub fn note_finalize(&mut self, revision: u64) {
        if self.enabled() {
            self.finalized_at_revision = Some(revision);
        }
    }

    /// Finalize the roots this session created. A read-only session must
    /// leave the file untouched, so it never finalizes (the caller saves only
    /// when this returns `Some`); neither does a session that only edited
    /// roots that were already there.
    ///
    /// The finalizer runs on a copy that holds only the session's roots; its
    /// recorded commands then apply to the real document as one batch, so
    /// undo history sees one ordinary step over the whole page. A replay that
    /// is rejected, touches another root, or diverges from the finalizer's
    /// own result is skipped.
    pub fn run(&mut self, state: &mut EditorState, reason: &'static str) -> Option<RepairSummary> {
        let revision = state.document_revision();
        if !self.enabled()
            || self.last_write_at.is_none()
            || self.finalized_at_revision == Some(revision)
        {
            return None;
        }
        // Every skip below still marks this revision handled, so the idle
        // loop does not retry the same document on every poll.
        self.finalized_at_revision = Some(revision);
        let scoped = session_scoped_state(state, &self.session_roots)?;
        let recorded = match de_orchestrator::record_loop_finalize_counted(&scoped) {
            Ok(recorded) => recorded,
            Err(error) => {
                eprintln!("design-editor mcp: auto-finalize ({reason}) failed: {error}");
                return None;
            }
        };
        if !recorded.commands.is_empty() {
            let Some(finalized) = replay_on_page(state, &recorded, &self.session_roots) else {
                eprintln!(
                    "design-editor mcp: auto-finalize ({reason}) skipped: its commands did not replay onto the session's roots alone"
                );
                return None;
            };
            *state = finalized;
        }
        let finalized_revision = state.document_revision();
        self.finalized_at_revision = Some(finalized_revision);
        eprintln!(
            "design-editor mcp: auto-finalize ({reason}) repairs={} revision={finalized_revision}",
            recorded.summary.total_repairs()
        );
        Some(recorded.summary)
    }

    fn enabled(&self) -> bool {
        self.idle_after != Duration::MAX
    }
}

/// A clone of `state` whose active page holds only the session's roots, or
/// `None` when none of them is still on the page.
fn session_scoped_state(
    state: &EditorState,
    session_roots: &BTreeSet<String>,
) -> Option<EditorState> {
    if !state
        .active_children()
        .iter()
        .any(|root| session_roots.contains(root.id_str()))
    {
        return None;
    }
    let mut scoped = state.clone();
    scoped
        .active_children_mut()
        .retain(|root| session_roots.contains(root.id_str()));
    Some(scoped)
}

/// Apply the finalizer's recorded commands to a copy of the whole document.
/// `None` unless the batch applies, every root outside the session is left
/// exactly as it was, and the session's roots come out as the finalizer
/// produced them.
fn replay_on_page(
    state: &EditorState,
    recorded: &de_orchestrator::RecordedLoopFinalize,
    session_roots: &BTreeSet<String>,
) -> Option<EditorState> {
    let outside_roots = |state: &EditorState| -> Vec<serde_json::Value> {
        state
            .active_children()
            .iter()
            .filter(|root| !session_roots.contains(root.id_str()))
            .filter_map(|root| serde_json::to_value(root).ok())
            .collect()
    };
    let mut next = state.clone();
    if !next.apply(EditorCommand::Batch {
        commands: recorded.commands.clone(),
    }) {
        return None;
    }
    if outside_roots(&next) != outside_roots(state) {
        return None;
    }
    let finalized_roots_match = recorded.state.active_children().iter().all(|expected| {
        next.active_children()
            .iter()
            .find(|root| root.id_str() == expected.id_str())
            .is_some_and(|root| {
                serde_json::to_value(root).ok() == serde_json::to_value(expected).ok()
            })
    });
    finalized_roots_match.then_some(next)
}

pub(super) fn run(path: PathBuf) -> Result<(), McpServeError> {
    let mut state = super::load_editor_state(&path)?;
    let signals = install_shutdown_signals()?;
    let mut auto_finalize = AutoFinalize::from_env();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    run_stdio_session(
        &mut reader,
        &mut writer,
        &mut state,
        &path,
        &mut auto_finalize,
        &signals.flag,
    )
}

pub(super) fn run_stdio_session<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    state: &mut EditorState,
    path: &Path,
    auto_finalize: &mut AutoFinalize,
    shutdown_flag: &AtomicBool,
) -> Result<(), McpServeError> {
    let mut line = String::new();
    loop {
        line.clear();
        if shutdown_flag.load(Ordering::Acquire) {
            run_auto_finalize_and_save(auto_finalize, state, path, "shutdown")?;
            return Ok(());
        }
        let n = match reader.read_line(&mut line) {
            Ok(n) => n,
            Err(error)
                if error.kind() == std::io::ErrorKind::Interrupted
                    && shutdown_flag.load(Ordering::Acquire) =>
            {
                run_auto_finalize_and_save(auto_finalize, state, path, "shutdown")?;
                return Ok(());
            }
            Err(e) => return Err(McpServeError::Io(format!("stdin read: {e}"))),
        };
        if n == 0 {
            run_auto_finalize_and_save(auto_finalize, state, path, "stdin-eof")?;
            return Ok(());
        }
        if let Some(resp) =
            super::process_message_with_auto_finalize(state, path, &line, Some(auto_finalize))?
        {
            writeln!(writer, "{resp}")
                .map_err(|e| McpServeError::Io(format!("stdout write: {e}")))?;
            writer
                .flush()
                .map_err(|e| McpServeError::Io(format!("stdout flush: {e}")))?;
        }
    }
}

pub(super) fn run_http(path: PathBuf, port: u16) -> Result<(), McpServeError> {
    const HTTP_IO_TIMEOUT: Duration = Duration::from_secs(30);
    let mut state = super::load_editor_state(&path)?;
    let signals = install_shutdown_signals()?;
    let mut auto_finalize = AutoFinalize::from_env();
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| McpServeError::Config(format!("bind 127.0.0.1:{port}: {e}")))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| McpServeError::Config(format!("set nonblocking listener: {e}")))?;
    eprintln!("design-editor --mcp-http: listening on 127.0.0.1:{port}");
    loop {
        if signals.requested() {
            break;
        }
        if auto_finalize.due(Instant::now(), state.document_revision()) {
            run_auto_finalize_and_save(&mut auto_finalize, &mut state, &path, "idle")?;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(HTTP_IO_TIMEOUT));
                let _ = stream.set_write_timeout(Some(HTTP_IO_TIMEOUT));
                match serve_http_connection(
                    &mut stream,
                    &mut state,
                    &path,
                    Some(&mut auto_finalize),
                ) {
                    Ok(true) => {
                        eprintln!("design-editor --mcp-http: shutdown requested; exiting");
                        break;
                    }
                    Ok(false) => {}
                    Err(e) => eprintln!("design-editor --mcp-http: {e}"),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                // Short nap, not the idle threshold: a tool call arriving
                // during this sleep waits for it, so it must stay well under
                // the per-call latency the model notices. The idle check above
                // runs every iteration, so finalize still fires within ~250 ms
                // of the threshold.
                std::thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) => eprintln!("design-editor --mcp-http: accept: {error}"),
        }
    }
    run_auto_finalize_and_save(&mut auto_finalize, &mut state, &path, "shutdown")
}

pub(super) fn serve_http_connection<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    state: &mut EditorState,
    path: &Path,
    auto_finalize: Option<&mut AutoFinalize>,
) -> Result<bool, McpServeError> {
    let reply = |stream: &mut S, status: &str, body: &str| {
        super::write_mcp_http_response_with_origin(stream, status, body, Some("*"))
    };
    let req = super::read_http_request(stream)?;
    if req.method == "OPTIONS" {
        return reply(stream, "204 No Content", "").map(|()| false);
    }
    if req.path != "/mcp" && req.path != "/" {
        return reply(stream, "404 Not Found", r#"{"error":"Not found"}"#).map(|()| false);
    }
    if req.method != "POST" {
        return reply(
            stream,
            "400 Bad Request",
            r#"{"jsonrpc":"2.0","error":{"code":-32000,"message":"Invalid or missing session ID"},"id":null}"#,
        )
        .map(|()| false);
    }
    if let Some(id) = super::shutdown_request_id(
        &req.body,
        &super::headless_token_from_env().unwrap_or_default(),
    ) {
        reply(stream, "200 OK", &super::shutdown_ok_response(&id))?;
        return Ok(true);
    }
    match super::process_message_with_auto_finalize(state, path, &req.body, auto_finalize)? {
        Some(response) => reply(stream, "200 OK", &response).map(|()| false),
        None => reply(stream, "202 Accepted", "").map(|()| false),
    }
}

fn run_auto_finalize_and_save(
    auto_finalize: &mut AutoFinalize,
    state: &mut EditorState,
    path: &Path,
    reason: &'static str,
) -> Result<(), McpServeError> {
    if auto_finalize.run(state, reason).is_some() {
        super::save_editor_state(state, path)?;
    }
    Ok(())
}

struct ShutdownSignals {
    flag: Arc<AtomicBool>,
    registrations: Vec<signal_hook::SigId>,
}

impl ShutdownSignals {
    fn requested(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

impl Drop for ShutdownSignals {
    fn drop(&mut self) {
        for registration in self.registrations.drain(..) {
            let _ = signal_hook::low_level::unregister(registration);
        }
    }
}

fn install_shutdown_signals() -> Result<ShutdownSignals, McpServeError> {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};

    let flag = Arc::new(AtomicBool::new(false));
    let term = signal_hook::flag::register(SIGTERM, Arc::clone(&flag))
        .map_err(|e| McpServeError::Config(format!("register SIGTERM handler: {e}")))?;
    let interrupt = match signal_hook::flag::register(SIGINT, Arc::clone(&flag)) {
        Ok(id) => id,
        Err(error) => {
            let _ = signal_hook::low_level::unregister(term);
            return Err(McpServeError::Config(format!(
                "register SIGINT handler: {error}"
            )));
        }
    };
    Ok(ShutdownSignals {
        flag,
        registrations: vec![term, interrupt],
    })
}

#[cfg(test)]
#[path = "auto_finalize_tests.rs"]
mod tests;
