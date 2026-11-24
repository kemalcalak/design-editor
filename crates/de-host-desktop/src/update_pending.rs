//! The release a restart was meant to install (#362).
//!
//! Before the app hands a staged update to its helper and quits, it writes
//! the target version here. The next launch reads and removes it: a running
//! version that differs means the install never happened (the helper failed,
//! hung or was stopped), and the update modal says so instead of silently
//! running the old build. This holds whatever went wrong in the helper,
//! because only the outcome is checked.

use std::path::{Path, PathBuf};

const MARKER_FILE: &str = "design-editor-update-pending.txt";

/// Where the marker lives: the user's temp folder, which survives the
/// restart and needs no settings to be loaded first.
pub(crate) fn marker_path() -> PathBuf {
    std::env::temp_dir().join(MARKER_FILE)
}

/// Record that the coming restart should start `version`.
pub(crate) fn record(path: &Path, version: &str) -> std::io::Result<()> {
    std::fs::write(path, version)
}

/// Forget a recorded restart (the helper could not be started).
pub(crate) fn clear(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Read and remove the marker. `Some(version)` when the last restart was
/// meant to install `version` but `running` is another build.
pub(crate) fn take_failed(path: &Path, running: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    clear(path);
    let wanted = text.trim();
    (!wanted.is_empty() && wanted != running).then(|| wanted.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_marker(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "de-update-pending-test-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(MARKER_FILE)
    }

    #[test]
    fn a_restart_that_did_not_install_is_reported_once() {
        let path = temp_marker("failed");
        record(&path, "0.6.2").unwrap();
        assert_eq!(take_failed(&path, "0.6.1").as_deref(), Some("0.6.2"));
        assert!(!path.exists(), "the marker is consumed");
        assert_eq!(take_failed(&path, "0.6.1"), None);
    }

    #[test]
    fn a_restart_that_installed_is_silent() {
        let path = temp_marker("installed");
        record(&path, "0.6.2\n").unwrap();
        assert_eq!(take_failed(&path, "0.6.2"), None);
        assert!(!path.exists());
    }

    #[test]
    fn no_marker_means_nothing_to_report() {
        let path = temp_marker("absent");
        clear(&path);
        assert_eq!(take_failed(&path, "0.6.1"), None);
    }
}
