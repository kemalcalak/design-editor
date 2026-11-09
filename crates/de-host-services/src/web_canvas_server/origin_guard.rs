//! Browser-origin screening for the daemon's sensitive POST routes
//! (credentials / AI / figma): same-origin by default, widened only
//! by the explicit `DESIGN_EDITOR_WEB_ALLOWED_ORIGINS` allowlist. Split out of
//! `web_canvas_server.rs` to keep the spine under the 800-line cap.

pub(crate) const WEB_ALLOWED_ORIGINS_ENV: &str = "DESIGN_EDITOR_WEB_ALLOWED_ORIGINS";

pub(super) fn is_sensitive_browser_post(request: &crate::mcp_serve::HttpRequest) -> bool {
    request.method == "POST"
        && (request.path == "/api/settings/credentials"
            || request.path.starts_with("/api/ai/")
            || request.path.starts_with("/api/figma/")
            || request.path == "/api/browser/snapshot"
            || request.path == de_editor_core::browser_live_wire::BROWSER_LIVE_PATH
            // A page on another origin must not run programs here (#219).
            || request.path.starts_with("/api/scripts/"))
}

/// Whether the request may proceed: false when the browser says another
/// site made it (`Sec-Fetch-Site: cross-site`). Only `cross-site` is refused: `same-origin`,
/// `same-site` and `none` (typed into the address bar) pass, and the Origin
/// check still runs after. A request without the header is not from a
/// current browser; like a request without `Origin`, it passes, but only on
/// a loopback daemon (`Host`), so a remote daemon does not open slots to
/// anything that leaves the header out. Guards routes a page on another
/// site could otherwise use up, such as the live browser channel's client
/// slots, which a cross-site `<img>` or `<script>` GET (no `Origin`) fills.
pub(super) fn fetch_site_allowed(request: &crate::mcp_serve::HttpRequest) -> bool {
    match request.sec_fetch_site.as_deref() {
        Some(site) => !site.trim().eq_ignore_ascii_case("cross-site"),
        None => request
            .host
            .as_deref()
            .and_then(|host| reqwest::Url::parse(&format!("http://{host}/")).ok())
            .is_some_and(|url| url.host_str().is_some_and(is_loopback_web_host)),
    }
}

/// `application/json` (optionally with parameters, e.g. `; charset=utf-8`).
pub(super) fn content_type_is_json(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/json")
    })
}

pub(super) fn credential_request_origin_allowed(request: &crate::mcp_serve::HttpRequest) -> bool {
    let allowed_origins = std::env::var(WEB_ALLOWED_ORIGINS_ENV).ok();
    credential_request_origin_allowed_with_config(request, allowed_origins.as_deref())
}

pub(super) fn credential_request_origin_allowed_with_config(
    request: &crate::mcp_serve::HttpRequest,
    allowed_origins: Option<&str>,
) -> bool {
    let Some(origin) = request.origin.as_deref() else {
        // Non-browser clients do not normally send Origin. Server persistence
        // is an opt-in private-deployment feature, so those clients remain
        // usable while browser requests are constrained by the unforgeable
        // Origin header.
        return true;
    };
    let Some(host) = request.host.as_deref() else {
        return false;
    };
    let Some(origin) = parse_http_origin(origin) else {
        return false;
    };
    let Ok(host) = reqwest::Url::parse(&format!("http://{host}/")) else {
        return false;
    };
    let same_request_authority =
        origin
            .host_str()
            .zip(host.host_str())
            .is_some_and(|(origin_host, request_host)| {
                let request_port = host.port().or_else(|| match origin.scheme() {
                    "http" => Some(80),
                    "https" => Some(443),
                    _ => None,
                });
                origin_host.eq_ignore_ascii_case(request_host)
                    && origin.port_or_known_default() == request_port
            });
    if !same_request_authority {
        return false;
    }
    if origin.host_str().is_some_and(is_loopback_web_host) {
        return true;
    }
    allowed_origins
        .into_iter()
        .flat_map(|origins| origins.split(','))
        .filter_map(|configured| parse_http_origin(configured.trim()))
        .any(|configured| same_url_origin(&origin, &configured))
}

pub(crate) fn parse_http_origin(value: &str) -> Option<reqwest::Url> {
    let origin = reqwest::Url::parse(value).ok()?;
    (matches!(origin.scheme(), "http" | "https")
        && origin.username().is_empty()
        && origin.password().is_none()
        && origin.host_str().is_some()
        && origin.path() == "/"
        && origin.query().is_none()
        && origin.fragment().is_none())
    .then_some(origin)
}

pub(super) fn same_url_origin(left: &reqwest::Url, right: &reqwest::Url) -> bool {
    left.scheme() == right.scheme()
        && left
            .host_str()
            .zip(right.host_str())
            .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
        && left.port_or_known_default() == right.port_or_known_default()
}

pub(super) fn is_loopback_web_host(host: &str) -> bool {
    let ip_literal = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.eq_ignore_ascii_case("localhost")
        || host
            .to_ascii_lowercase()
            .strip_suffix(".localhost")
            .is_some_and(|prefix| !prefix.is_empty())
        || ip_literal
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}
