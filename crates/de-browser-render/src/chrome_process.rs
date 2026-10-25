//! One headless browser process: started with a throwaway profile and
//! `--remote-debugging-port=0`, its DevTools websocket address read from
//! the "DevTools listening on ws://…" line it prints on stderr. Dropping
//! the handle ends the process tree and deletes the profile.
//!
//! A static pool is never dropped when the app is killed (SIGTERM, a
//! crash), so on Unix a small watchdog (`sh`) polls the app's pid and ends
//! the browser's process group within seconds once the app is gone. Windows
//! relies on the app's own shutdown path.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::BrowserRenderError;

/// How long the browser may take to open its DevTools endpoint.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const DEVTOOLS_PREFIX: &str = "DevTools listening on ";

pub(crate) struct ChromeProcess {
    child: Child,
    /// Ends the browser if this process dies without dropping it (Unix).
    watchdog: Option<Child>,
    profile: PathBuf,
    ws_url: String,
}

impl ChromeProcess {
    pub(crate) fn spawn(binary: &Path) -> Result<Self, BrowserRenderError> {
        let profile = std::env::temp_dir().join(format!(
            "de-browser-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let mut command = Command::new(binary);
        command
            .args(launch_args(&profile))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        // Its own process group, so ending the tree reaches the renderer
        // and GPU helpers too (`de_process_io` checks the group first).
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
        let mut child = command.spawn().map_err(BrowserRenderError::Launch)?;
        let Some(stderr) = child.stderr.take() else {
            let _ = de_process_io::kill_process_tree(&mut child);
            return Err(BrowserRenderError::Startup);
        };
        let (tx, rx) = mpsc::channel();
        // Reads the endpoint line, then keeps draining stderr so the
        // browser never blocks on a full pipe. Ends with the process.
        std::thread::spawn(move || {
            let mut sent = false;
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !sent {
                    if let Some(url) = devtools_url(&line) {
                        let _ = tx.send(url);
                        sent = true;
                    }
                }
            }
        });
        match rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(ws_url) => {
                let watchdog = spawn_watchdog(child.id());
                Ok(Self {
                    child,
                    watchdog,
                    profile,
                    ws_url,
                })
            }
            Err(_) => {
                let _ = de_process_io::kill_process_tree(&mut child);
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(&profile);
                Err(BrowserRenderError::Startup)
            }
        }
    }

    /// The browser-level DevTools websocket address.
    pub(crate) fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// Whether the process is still running.
    pub(crate) fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for ChromeProcess {
    fn drop(&mut self) {
        if let Some(mut watchdog) = self.watchdog.take() {
            let _ = watchdog.kill();
            let _ = watchdog.wait();
        }
        let _ = de_process_io::terminate_process_tree(&mut self.child, Duration::from_secs(2));
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

/// Start the watchdog for the browser `browser_pid` (its own process group).
#[cfg(unix)]
fn spawn_watchdog(browser_pid: u32) -> Option<Child> {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(watchdog_script(std::process::id(), browser_pid))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

#[cfg(not(unix))]
fn spawn_watchdog(_browser_pid: u32) -> Option<Child> {
    None
}

/// Poll the app every 2 s; once it is gone, end the browser's group
/// (politely, then for good).
#[cfg_attr(not(unix), allow(dead_code))]
fn watchdog_script(app_pid: u32, browser_pid: u32) -> String {
    format!(
        "while kill -0 {app_pid} 2>/dev/null; do sleep 2; done; \
         kill -TERM -{browser_pid} 2>/dev/null; sleep 2; kill -KILL -{browser_pid} 2>/dev/null"
    )
}

fn launch_args(profile: &Path) -> Vec<String> {
    vec![
        "--headless=new".into(),
        "--remote-debugging-port=0".into(),
        "--remote-debugging-address=127.0.0.1".into(),
        format!("--user-data-dir={}", profile.display()),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-extensions".into(),
        "--disable-background-networking".into(),
        "--disable-sync".into(),
        "--mute-audio".into(),
        "--hide-scrollbars".into(),
        "about:blank".into(),
    ]
}

/// The websocket address on a "DevTools listening on ws://…" line.
fn devtools_url(line: &str) -> Option<String> {
    let url = line.trim().strip_prefix(DEVTOOLS_PREFIX)?.trim();
    url.starts_with("ws://").then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_devtools_line_yields_the_websocket_address() {
        assert_eq!(
            devtools_url("DevTools listening on ws://127.0.0.1:53111/devtools/browser/ab-12\n"),
            Some("ws://127.0.0.1:53111/devtools/browser/ab-12".into())
        );
        assert_eq!(devtools_url("[0101/000000.0:INFO] something else"), None);
        assert_eq!(devtools_url("DevTools listening on http://x"), None);
    }

    #[test]
    fn the_watchdog_waits_for_the_app_then_ends_the_browser_group() {
        let script = watchdog_script(41, 4242);
        assert!(script.starts_with("while kill -0 41 2>/dev/null; do sleep 2; done;"));
        assert!(script.contains("kill -TERM -4242"));
        assert!(script.contains("kill -KILL -4242"));
    }

    #[test]
    fn the_browser_runs_headless_on_a_loopback_port_with_a_fresh_profile() {
        let args = launch_args(Path::new("/tmp/de-browser-x"));
        assert!(args.contains(&"--headless=new".to_string()));
        assert!(args.contains(&"--remote-debugging-port=0".to_string()));
        assert!(args.contains(&"--remote-debugging-address=127.0.0.1".to_string()));
        assert!(args.contains(&"--user-data-dir=/tmp/de-browser-x".to_string()));
    }
}
