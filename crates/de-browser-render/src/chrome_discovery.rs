//! Find an installed Chromium-based browser. `DE_BROWSER_PATH` wins when it
//! names a file; otherwise the usual install locations of Chrome, Chromium,
//! Edge and Brave are tried in order, per platform.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Environment variable naming the browser binary to use.
pub const BROWSER_PATH_ENV: &str = "DE_BROWSER_PATH";

/// The browser binary to launch, `None` when none is installed.
pub fn find_browser() -> Option<PathBuf> {
    find_browser_in(std::env::var_os(BROWSER_PATH_ENV), &candidates(), |path| {
        path.is_file()
    })
}

/// [`find_browser`] with the environment, candidate list and file check
/// passed in, so the order is testable on every platform.
pub(crate) fn find_browser_in(
    env_path: Option<OsString>,
    candidates: &[PathBuf],
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(path) = env_path.filter(|p| !p.is_empty()).map(PathBuf::from) {
        if exists(&path) {
            return Some(path);
        }
    }
    candidates.iter().find(|path| exists(path)).cloned()
}

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    let apps = [
        "Google Chrome.app/Contents/MacOS/Google Chrome",
        "Chromium.app/Contents/MacOS/Chromium",
        "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "Brave Browser.app/Contents/MacOS/Brave Browser",
    ];
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    roots
        .iter()
        .flat_map(|root| apps.iter().map(move |app| root.join(app)))
        .collect()
}

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    let installs = [
        r"Google\Chrome\Application\chrome.exe",
        r"Chromium\Application\chrome.exe",
        r"Microsoft\Edge\Application\msedge.exe",
        r"BraveSoftware\Brave-Browser\Application\brave.exe",
    ];
    ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(|var| std::env::var_os(var).map(PathBuf::from))
        .flat_map(|root| installs.iter().map(move |rel| root.join(rel)))
        .collect()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn candidates() -> Vec<PathBuf> {
    let names = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
        "brave-browser",
    ];
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    names
        .iter()
        .flat_map(|name| dirs.iter().map(move |dir| dir.join(name)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_path_wins_when_it_exists() {
        let candidates = [PathBuf::from("/a/chrome"), PathBuf::from("/b/chromium")];
        let all = |_: &Path| true;
        assert_eq!(
            find_browser_in(Some("/custom/browser".into()), &candidates, all),
            Some(PathBuf::from("/custom/browser"))
        );
        let only_b = |p: &Path| p == Path::new("/b/chromium");
        assert_eq!(
            find_browser_in(Some("/missing".into()), &candidates, only_b),
            Some(PathBuf::from("/b/chromium")),
            "a missing override falls back to the install list"
        );
        assert_eq!(
            find_browser_in(Some("".into()), &candidates, |_| false),
            None
        );
    }

    #[test]
    fn the_first_installed_candidate_is_used() {
        let candidates = [
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        let installed = |p: &Path| p != Path::new("/a");
        assert_eq!(
            find_browser_in(None, &candidates, installed),
            Some(PathBuf::from("/b"))
        );
    }
}
