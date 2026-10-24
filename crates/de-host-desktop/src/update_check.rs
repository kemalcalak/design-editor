//! Background auto-update check.
//!
//! A background probe of the GitHub releases API that compares the
//! running build to the latest published release and surfaces the
//! result.
//!
//! Flow: [`UpdateProbe::spawn`] runs one HTTPS request on a worker
//! thread (never the UI thread); the runner drains the outcome with
//! [`UpdateProbe::poll`] and writes it into
//! `EditorUiState::update_status` (rendered by the settings System
//! tab). When a newer release is found the runner opens the in-app
//! update modal; the download / verify / install job lives in
//! `update_install.rs` (#111).
//!
//! Every failure mode (offline, rate-limited, malformed JSON)
//! degrades to `UpdateStatus::Error` — the probe never panics and
//! never blocks shutdown.

use std::sync::mpsc::{self, Receiver, TryRecvError};

use de_editor_core::UpdateStatus;

/// GitHub publish target — mirrors the TS app's `constants.ts`
/// (`GITHUB_OWNER` / `GITHUB_REPO`), the same repo `electron-updater`
/// reads releases from.
const GITHUB_OWNER: &str = "kemalcalak";
const GITHUB_REPO: &str = "design-editor";

/// Releases page a found-update notice opens in the browser.
pub fn releases_url() -> String {
    format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases")
}

/// Releases list API endpoint. GitHub's `releases/latest` endpoint excludes
/// prereleases, so the desktop updater uses the list feed and picks the first
/// non-draft release itself.
fn latest_release_api() -> String {
    format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases?per_page=20")
}

/// Releases Atom feed — the fallback when the JSON API is unavailable.
///
/// The anonymous API allows 60 requests per hour *per source IP*, so every
/// user behind a shared egress (corporate NAT, a VPN, most China-region
/// proxies) can find it already exhausted by strangers and see the probe fail
/// with "check the network" while the network is perfectly fine. The Atom
/// feed is served by github.com rather than api.github.com and is not on that
/// quota, so it answers when the API will not. Drafts never appear in it,
/// which is the same rule `select_latest_release_tag` applies to the API
/// response.
fn latest_release_atom() -> String {
    format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases.atom")
}

/// Background release-API probe. One request per `spawn`; the
/// runner re-spawns to re-check (e.g. from the "Check for Updates"
/// menu item).
pub struct UpdateProbe {
    rx: Option<Receiver<UpdateStatus>>,
}

impl UpdateProbe {
    /// Disabled auto-check state. Manual checks still call
    /// [`UpdateProbe::spawn`] directly.
    pub fn idle() -> Self {
        Self { rx: None }
    }

    /// The launch-time probe: one release check when the user's
    /// "automatic updates" setting is on, otherwise idle.
    pub fn for_auto_check(enabled: bool) -> Self {
        if enabled {
            Self::spawn()
        } else {
            Self::idle()
        }
    }

    /// Spawn the probe worker. Returns immediately; the request
    /// runs on its own thread.
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(check_latest_release());
        });
        Self { rx: Some(rx) }
    }

    /// Whether the probe worker is still running (result not yet
    /// drained). The runner uses this to keep waking the event loop
    /// so a result that lands while the app is idle is still drained.
    pub fn is_pending(&self) -> bool {
        self.rx.is_some()
    }

    /// Drain the probe result if it has landed. Returns `Some` once
    /// (when the worker resolves), `None` while still in flight or
    /// after it has already been drained — so the runner can write
    /// the new status and act on a found update exactly once.
    pub fn poll(&mut self) -> Option<UpdateStatus> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok(status) => {
                self.rx = None;
                Some(status)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.rx = None;
                Some(UpdateStatus::Error)
            }
        }
    }
}

/// Query the releases list and classify the running build against the newest
/// non-draft release, including prereleases. Blocking — only ever called on the
/// probe worker thread.
fn check_latest_release() -> UpdateStatus {
    let Some(tag) = fetch_latest_tag() else {
        return UpdateStatus::Error;
    };
    let latest = tag.trim().trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");
    if is_newer(latest, current) {
        UpdateStatus::Available {
            version: latest.to_string(),
        }
    } else {
        UpdateStatus::UpToDate
    }
}

/// Run the HTTPS request. The worker bridges onto the shared async
/// runtime for the one call — a private per-call runtime aborts
/// with "Cannot start a runtime from within a runtime" whenever this
/// probe is reached from a tokio worker. `None` on any transport /
/// parse failure.
fn fetch_latest_tag() -> Option<String> {
    de_host_services::chat_runtime::block_on_anywhere(async {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(15))
            // GitHub's API rejects requests without a User-Agent.
            .user_agent(concat!("design-editor/", env!("CARGO_PKG_VERSION")))
            .build()
            .ok()?;
        // The API carries the richer response, so it stays the first choice;
        // the feed only has to cover the case where the API refuses to answer
        // at all. A rate-limited probe must not read as "you are offline".
        if let Some(tag) = fetch_tag_from_api(&client).await {
            return Some(tag);
        }
        fetch_tag_from_atom(&client).await
    })
}

async fn fetch_tag_from_api(client: &reqwest::Client) -> Option<String> {
    let resp = client
        .get(latest_release_api())
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    select_latest_release_tag(&json)
}

async fn fetch_tag_from_atom(client: &reqwest::Client) -> Option<String> {
    let resp = client
        .get(latest_release_atom())
        .header("Accept", "application/atom+xml")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    select_latest_release_tag_from_atom(&resp.text().await.ok()?)
}

/// Pull the newest release tag out of the releases Atom feed.
///
/// Entries are newest-first and each carries
/// `<id>tag:github.com,2008:Repository/<repo-id>/<tag></id>`, so the tag is
/// the last path segment of the first entry id. Only that one field is read —
/// this is deliberately a targeted extraction rather than an XML parse, so a
/// feed that grows unrelated markup cannot change the result.
fn select_latest_release_tag_from_atom(feed: &str) -> Option<String> {
    // The feed itself opens with an `<id>` for the repository, which has no
    // `Repository/<id>/<tag>` shape; requiring the entry marker skips it.
    const ENTRY_ID_MARKER: &str = "tag:github.com,2008:Repository/";
    let rest = feed.split_once(ENTRY_ID_MARKER)?.1;
    let id_body = rest.split_once("</id>")?.0;
    let tag = id_body.rsplit_once('/')?.1.trim();
    (!tag.is_empty() && !tag.contains('<')).then(|| tag.to_string())
}

/// Select the newest published release tag from GitHub's releases list.
/// GitHub returns newest releases first. Drafts are not visible to users and
/// must not drive update prompts; prereleases are visible and are intentionally
/// allowed for v0.8.0.
fn select_latest_release_tag(json: &serde_json::Value) -> Option<String> {
    json.as_array()?
        .iter()
        .filter(|release| {
            !release
                .get("draft")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .find_map(|release| release.get("tag_name")?.as_str().map(str::to_string))
}

/// The release-asset filename rust-release.yml publishes for this
/// platform, or `None` on an unsupported os/arch pair.
pub(crate) fn installer_asset_name(version: &str) -> Option<String> {
    asset_name_for(std::env::consts::OS, std::env::consts::ARCH, version)
}

/// Pure name table — split from [`installer_asset_name`] so tests
/// cover every platform from any host.
/// Published: macOS arm64 (DMG), Windows x64 (NSIS installer) and Linux
/// x86_64 (a tarball holding the bare binary, #51). The Linux name carries no
/// version; the download base already names the release.
fn asset_name_for(os: &str, arch: &str, version: &str) -> Option<String> {
    match (os, arch) {
        ("macos", "aarch64") => Some(format!("Design-Editor-{version}-arm64-mac.dmg")),
        ("windows", "x86_64") => Some(format!("Design-Editor-{version}-x64-win-setup.exe")),
        ("linux", "x86_64") => Some("design-editor-linux-x86_64.tar.gz".to_string()),
        _ => None,
    }
}

/// Download URL prefix for the assets of release `v<version>`.
pub(crate) fn release_download_base(version: &str) -> String {
    format!("https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases/download/v{version}")
}

/// Args for `cmd /C start "" "<target>"` with the target double-quoted
/// so cmd doesn't split it at `&` (OAuth / query-string URLs) or at
/// spaces (installer paths). Inside double quotes cmd keeps its
/// metacharacters literal; `"` itself is illegal in URLs and Windows
/// paths but is stripped defensively. `%VAR%` expansion remains
/// possible in pathological inputs — accepted, matches the `open`
/// crate's behaviour.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn windows_start_args(target: &str) -> String {
    format!("/C start \"\" \"{}\"", target.replace('"', "%22"))
}

/// Spawn `cmd /C start "" "<target>"` without flashing a console.
#[cfg(target_os = "windows")]
fn windows_start(target: &str) -> std::io::Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut c = std::process::Command::new("cmd");
    c.raw_arg(windows_start_args(target));
    c.creation_flags(CREATE_NO_WINDOW);
    c.spawn()
}

/// Whether `latest` is a strictly newer version than `current`.
/// Both are dotted numeric strings (`0.8.0`); a pre-release suffix
/// (`-beta.1`) is dropped before the numeric compare so a stable
/// release of the same core version doesn't read as an update.
fn is_newer(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split('-')
            .next()
            .unwrap_or(v)
            .split('.')
            .map(|n| n.trim().parse::<u64>().unwrap_or(0))
            .collect()
    }
    let l = parts(latest);
    let c = parts(current);
    let len = l.len().max(c.len());
    for i in 0..len {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv != cv {
            return lv > cv;
        }
    }
    false
}

/// Open `url` in the user's default browser. Best-effort: a missing
/// opener tool just logs. Platform openers — `open` (macOS),
/// `xdg-open` (Linux), `cmd /c start` (Windows).
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = windows_start(url);
    if let Err(e) = result {
        eprintln!("design-editor: open_url({url}) failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like the live feed: a repository-level `<id>` comes first and
    /// must not be mistaken for a release.
    const ATOM_FEED: &str = concat!(
        "<feed><id>tag:github.com,2008:https://github.com/kemalcalak/design-editor/releases</id>",
        "<entry><id>tag:github.com,2008:Repository/1159938129/v0.8.3</id></entry>",
        "<entry><id>tag:github.com,2008:Repository/1159938129/v0.8.2</id></entry></feed>",
    );

    #[test]
    fn atom_fallback_reads_the_newest_release_tag() {
        assert_eq!(
            select_latest_release_tag_from_atom(ATOM_FEED).as_deref(),
            Some("v0.8.3")
        );
    }

    #[test]
    fn atom_fallback_yields_nothing_without_a_release_entry() {
        // A repo with no releases still serves a feed, and its only `<id>` is
        // the repository one. Returning its trailing path segment there would
        // invent a version out of the URL.
        for feed in [
            "<feed><id>tag:github.com,2008:https://github.com/o/r/releases</id></feed>",
            "<feed></feed>",
            "",
            // Truncated mid-entry: no closing tag to bound the id.
            "<feed><entry><id>tag:github.com,2008:Repository/1/v0.8.3",
            // Present but empty tag segment.
            "<feed><entry><id>tag:github.com,2008:Repository/1/</id></entry></feed>",
        ] {
            assert_eq!(
                select_latest_release_tag_from_atom(feed),
                None,
                "{feed:?} must not yield a version"
            );
        }
    }

    #[test]
    fn windows_start_args_quotes_urls_with_query_params() {
        assert_eq!(
            windows_start_args("https://a.example/auth?b=1&c=2"),
            "/C start \"\" \"https://a.example/auth?b=1&c=2\""
        );
        // Double quotes are illegal in URLs/paths — stripped defensively.
        assert_eq!(windows_start_args("x\"y"), "/C start \"\" \"x%22y\"");
    }

    #[test]
    fn is_newer_compares_dotted_numbers() {
        assert!(is_newer("0.9.0", "0.8.0"));
        assert!(is_newer("0.8.1", "0.8.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.8.0", "0.8.0"));
        assert!(!is_newer("0.7.9", "0.8.0"));
    }

    #[test]
    fn is_newer_handles_uneven_segment_counts() {
        // A shorter version pads missing trailing segments with 0.
        assert!(is_newer("0.8.0.1", "0.8.0"));
        assert!(!is_newer("0.8", "0.8.0"));
        assert!(is_newer("0.9", "0.8.5"));
    }

    #[test]
    fn is_newer_drops_a_prerelease_suffix() {
        // A stable release equal to the running core version is not
        // "newer"; the `-beta` suffix must not skew the compare.
        assert!(!is_newer("0.8.0-beta.1", "0.8.0"));
        assert!(is_newer("0.9.0-rc.1", "0.8.0"));
    }

    #[test]
    fn releases_api_uses_list_endpoint_so_prereleases_are_visible() {
        assert_eq!(
            latest_release_api(),
            "https://api.github.com/repos/kemalcalak/design-editor/releases?per_page=20"
        );
    }

    #[test]
    fn release_list_selection_keeps_prerelease_and_skips_drafts() {
        let feed = serde_json::json!([
            {
                "tag_name": "v0.9.0",
                "draft": true,
                "prerelease": false
            },
            {
                "tag_name": "v0.8.0",
                "draft": false,
                "prerelease": true
            },
            {
                "tag_name": "v0.7.5",
                "draft": false,
                "prerelease": false
            }
        ]);

        assert_eq!(select_latest_release_tag(&feed).as_deref(), Some("v0.8.0"));
    }

    #[test]
    fn releases_url_points_at_the_publish_repo() {
        assert_eq!(
            releases_url(),
            "https://github.com/kemalcalak/design-editor/releases"
        );
    }

    #[test]
    fn idle_probe_is_not_pending() {
        let probe = UpdateProbe::idle();

        assert!(!probe.is_pending());
    }

    #[test]
    fn disabled_auto_check_creates_idle_probe() {
        assert!(!UpdateProbe::for_auto_check(false).is_pending());
    }

    #[test]
    fn disconnected_worker_resolves_as_error_instead_of_staying_checking() {
        let (tx, rx) = mpsc::channel();
        drop(tx);
        let mut probe = UpdateProbe { rx: Some(rx) };

        assert_eq!(probe.poll(), Some(UpdateStatus::Error));
        assert!(!probe.is_pending());
    }
}

#[cfg(test)]
mod installer_asset_tests {
    use super::{asset_name_for, release_download_base};

    #[test]
    fn download_base_points_at_the_tagged_release() {
        assert_eq!(
            release_download_base("0.2.1"),
            "https://github.com/kemalcalak/design-editor/releases/download/v0.2.1"
        );
    }

    #[test]
    fn asset_names_cover_the_release_matrix() {
        assert_eq!(
            asset_name_for("macos", "aarch64", "0.9.0").as_deref(),
            Some("Design-Editor-0.9.0-arm64-mac.dmg")
        );
        assert_eq!(
            asset_name_for("windows", "x86_64", "0.9.0").as_deref(),
            Some("Design-Editor-0.9.0-x64-win-setup.exe")
        );
        assert_eq!(
            asset_name_for("linux", "x86_64", "0.9.0").as_deref(),
            Some("design-editor-linux-x86_64.tar.gz")
        );
        // Unpublished platforms fall back to the releases page.
        assert_eq!(asset_name_for("linux", "aarch64", "0.9.0"), None);
        assert_eq!(asset_name_for("macos", "x86_64", "0.9.0"), None);
        assert_eq!(asset_name_for("windows", "aarch64", "0.9.0"), None);
        assert_eq!(asset_name_for("freebsd", "x86_64", "0.9.0"), None);
    }
}
