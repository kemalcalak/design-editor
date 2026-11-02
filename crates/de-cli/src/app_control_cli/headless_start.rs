//! `de start --headless` (#281): launch the windowless file-backed MCP
//! server and wait for it to answer.
//!
//! `--mcp-http` loads the whole document before it binds its port, so the
//! wait grows with the file (or is set with `--wait`). A server that does
//! not answer in time is stopped, so a failed start never leaves one
//! running behind the error. A start that names a different `--port` than
//! the running server's fails and says which server is running, since the
//! manager files track one server.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use de_process_io::{spawn_null, wait_for_child_or, WaitOutcome};

use super::{
    default_document_path, ensure_document_file, find_desktop_binary, make_token,
    preflight_document, remove_manager_files, start_json, write_manager_files, StartPort,
};
use crate::cli_error::CliError;

/// Probe interval while the server starts.
const PROBE_INTERVAL: Duration = Duration::from_millis(100);
/// Readiness budget for an empty or small document.
const BASE_WAIT_SECS: u64 = 10;
/// Extra seconds per MiB of document: loading dominates the start.
const WAIT_SECS_PER_MIB: u64 = 10;
/// Ceiling for the sized budget; `--wait` can go up to 600 s.
const MAX_SIZED_WAIT_SECS: u64 = 120;

pub(super) fn run_start_headless(
    port: u16,
    document_path: Option<&str>,
    wait_secs: Option<u64>,
) -> Result<String, CliError> {
    let document = match document_path {
        Some(path) => PathBuf::from(path),
        None => default_document_path()?,
    };
    ensure_document_file(&document)?;
    preflight_document(&document)?;
    let budget = wait_secs.unwrap_or_else(|| sized_wait_secs(document_len(&document)));

    let binary = find_desktop_binary()?;
    // Per-instance token passed to the server (echoed in its `ping`) so a
    // later `de stop` can prove the pid in our manager file owns the port.
    let token = make_token();
    let mut command = Command::new(&binary);
    command
        .arg("--mcp-http")
        .arg(port.to_string())
        .arg(&document)
        .env(de_config_store::env_vars::MCP_TOKEN, &token);
    let mut child = spawn_null(&mut command)
        .map_err(|e| CliError::Daemon(format!("spawn {} --mcp-http: {e}", binary.display())))?;

    let pid = child.id();
    write_manager_files(pid, port, &token)?;

    // Confirm the MCP server actually answers WITH our token (not just
    // an open port), so we never report success for a child that failed
    // to bind or for an unrelated service on the port.
    let attempts = probe_attempts(budget);
    match wait_for_child_or(&mut child, attempts, PROBE_INTERVAL, || {
        crate::mcp_http_cli::mcp_ping_headless(port, &token).then_some(())
    })
    .map_err(|e| CliError::Daemon(format!("wait for {} --mcp-http: {e}", binary.display())))?
    {
        WaitOutcome::Ready(()) => Ok(start_json(pid, port, Some(&document))),
        WaitOutcome::Exited(status) => {
            remove_manager_files();
            Err(CliError::Daemon(format!(
                "Design Editor MCP server exited before accepting connections: {status}"
            )))
        }
        WaitOutcome::TimedOut => {
            // Never leave a server running behind a failed start.
            let _ = child.kill();
            let _ = child.wait();
            remove_manager_files();
            Err(CliError::Daemon(format!(
                "Design Editor MCP server did not respond on 127.0.0.1:{port} within {budget}s, \
                 so it was stopped; pass --wait <seconds> to give a large document longer"
            )))
        }
    }
}

/// A running server answers a start only when the caller did not ask for
/// another port.
pub(super) fn check_requested_port(
    requested: StartPort,
    running_port: u16,
    running_pid: u32,
) -> Result<(), CliError> {
    if requested.explicit && requested.port != running_port {
        return Err(CliError::Daemon(format!(
            "a headless Design Editor MCP server is already running on port {running_port} \
             (pid {running_pid}); stop it with `de stop` or start without --port {}",
            requested.port
        )));
    }
    Ok(())
}

fn document_len(document: &Path) -> u64 {
    std::fs::metadata(document).map_or(0, |meta| meta.len())
}

/// Seconds to wait for a document of `len` bytes: the base budget plus
/// `WAIT_SECS_PER_MIB` per started MiB, capped.
fn sized_wait_secs(len: u64) -> u64 {
    let mib = len.div_ceil(1024 * 1024);
    (BASE_WAIT_SECS + mib * WAIT_SECS_PER_MIB).min(MAX_SIZED_WAIT_SECS)
}

fn probe_attempts(budget_secs: u64) -> usize {
    let per_second = (1000 / PROBE_INTERVAL.as_millis()) as u64;
    usize::try_from(budget_secs.saturating_mul(per_second)).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_grows_with_the_document_and_is_capped() {
        assert_eq!(sized_wait_secs(0), 10);
        assert_eq!(sized_wait_secs(200 * 1024), 20);
        // The 1.4 MB file from the report: two started MiB.
        assert_eq!(sized_wait_secs(1_400_000), 30);
        assert_eq!(sized_wait_secs(500 * 1024 * 1024), MAX_SIZED_WAIT_SECS);
    }

    #[test]
    fn probes_cover_the_whole_budget() {
        assert_eq!(probe_attempts(3), 30);
        assert_eq!(probe_attempts(120), 1200);
    }

    #[test]
    fn a_different_explicit_port_names_the_running_server() {
        let asked = StartPort {
            port: 3178,
            explicit: true,
        };
        let err = check_requested_port(asked, 3177, 42).expect_err("refused");
        let message = err.to_string();
        assert!(
            message.contains("port 3177") && message.contains("pid 42"),
            "{message}"
        );
    }

    #[test]
    fn the_running_server_answers_an_implicit_or_matching_port() {
        let implicit = StartPort {
            port: 3100,
            explicit: false,
        };
        assert!(check_requested_port(implicit, 3177, 42).is_ok());
        let same = StartPort {
            port: 3177,
            explicit: true,
        };
        assert!(check_requested_port(same, 3177, 42).is_ok());
    }
}
