//! Which addresses this process may load in its headless browser (#20).
//!
//! The desktop app and the stdio / file MCP servers run on the user's own
//! machine and load anything the user or their agent names. A serve-web
//! daemon may run on a server, where a page address is a request the server
//! makes: once it starts ([`screen_for_daemon`]) every load, the web
//! editor's snapshots and the MCP `browser` tool alike, goes through the URL
//! screen `import_html_url` uses (public http / https hosts plus the
//! `DESIGN_EDITOR_WEB_AI_ENDPOINT_ALLOWLIST` entries). A daemon listening on
//! loopback serves its own user's machine, so it also loads loopback and
//! file addresses (local dev servers). The screen covers the page address;
//! the page's own subresources load as Chrome loads them.

use std::net::IpAddr;
use std::sync::atomic::{AtomicU8, Ordering};

const OPEN: u8 = 0;
const SCREENED: u8 = 1;
const SCREENED_LOOPBACK_OK: u8 = 2;

static POLICY: AtomicU8 = AtomicU8::new(OPEN);

/// The serve-web daemon listens on `ip`: screen every page address.
pub(crate) fn screen_for_daemon(ip: IpAddr) {
    let policy = if ip.is_loopback() {
        SCREENED_LOOPBACK_OK
    } else {
        SCREENED
    };
    POLICY.store(policy, Ordering::Relaxed);
}

/// Whether this process may load `url`.
pub(crate) fn url_permitted(url: &str) -> bool {
    let allowlist = std::env::var(crate::import_html_url::ALLOWLIST_ENV).ok();
    match POLICY.load(Ordering::Relaxed) {
        OPEN => true,
        policy => url_allowed(url, policy == SCREENED_LOOPBACK_OK, allowlist.as_deref()),
    }
}

/// The screen: the import URL screen, and with `loopback_ok` also loopback
/// hosts and file addresses.
pub(crate) fn url_allowed(url: &str, loopback_ok: bool, allowlist: Option<&str>) -> bool {
    if crate::import_html_url::screen_import_url_with_allowlist(url, allowlist).is_ok() {
        return true;
    }
    if !loopback_ok {
        return false;
    }
    if url.to_ascii_lowercase().starts_with("file://") {
        return true;
    }
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str().is_some_and(|host| {
            let host = host.trim_start_matches('[').trim_end_matches(']');
            host.eq_ignore_ascii_case("localhost")
                || host.ends_with(".localhost")
                || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_daemon_loads_public_addresses_only() {
        assert!(url_allowed("https://example.com/page", false, None));
        for url in [
            "http://127.0.0.1:3000/",
            "http://localhost:5173",
            "http://169.254.169.254/latest/meta-data",
            "http://10.0.0.5/admin",
            "file:///etc/passwd",
            "http://intranet/",
        ] {
            assert!(!url_allowed(url, false, None), "{url}");
        }
        assert!(
            url_allowed(
                "https://staging.corp.example/",
                false,
                Some("https://staging.corp.example")
            ),
            "the allowlist opens more"
        );
    }

    #[test]
    fn a_loopback_daemon_also_loads_its_own_machine() {
        for url in [
            "http://127.0.0.1:3000/",
            "http://localhost:5173/x",
            "http://app.localhost/",
            "http://[::1]:8080/",
            "file:///Users/me/site/index.html",
        ] {
            assert!(url_allowed(url, true, None), "{url}");
        }
        assert!(
            !url_allowed("http://10.0.0.5/", true, None),
            "the LAN stays out"
        );
        assert!(!url_allowed("http://169.254.169.254/", true, None));
    }
}
