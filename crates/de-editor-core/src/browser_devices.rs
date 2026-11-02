//! Device presets of the live browser node (#20) and the shared rules for
//! its `url` / `zoom` / scroll values. A preset is the CSS viewport the page
//! is laid out at; picking one also resizes the node to that box. An id the
//! table does not know (a pen.dev preset we have not mapped yet) is kept on
//! the node and the page is laid out at the node's own size.

/// One device preset: its `deviceId`, the i18n key of its menu label and
/// the CSS viewport the page renders at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrowserDevice {
    pub id: &'static str,
    pub label_key: &'static str,
    pub width: f64,
    pub height: f64,
    /// Mobile presets emulate a touch device (mobile user agent and
    /// viewport meta handling) when the page is rendered.
    pub mobile: bool,
}

/// Every preset, in menu order.
pub const BROWSER_DEVICES: [BrowserDevice; 3] = [
    BrowserDevice {
        id: "desktop",
        label_key: "browser.device.desktop",
        width: 1440.0,
        height: 900.0,
        mobile: false,
    },
    BrowserDevice {
        id: "tablet",
        label_key: "browser.device.tablet",
        width: 768.0,
        height: 1024.0,
        mobile: true,
    },
    BrowserDevice {
        id: "phone",
        label_key: "browser.device.phone",
        width: 390.0,
        height: 844.0,
        mobile: true,
    },
];

/// The preset a `deviceId` names, `None` for an unknown id.
pub fn browser_device(id: &str) -> Option<&'static BrowserDevice> {
    BROWSER_DEVICES.iter().find(|device| device.id == id)
}

/// Smallest and largest page zoom (1 = 100 %).
pub const BROWSER_ZOOM_MIN: f64 = 0.25;
pub const BROWSER_ZOOM_MAX: f64 = 4.0;

/// Clamp a zoom into the supported range; a non-finite value means 100 %.
pub fn clamp_browser_zoom(zoom: f64) -> f64 {
    if zoom.is_finite() {
        zoom.clamp(BROWSER_ZOOM_MIN, BROWSER_ZOOM_MAX)
    } else {
        1.0
    }
}

/// Clamp a scroll offset: pages scroll from 0 down / right only.
pub fn clamp_browser_scroll(offset: f64) -> f64 {
    if offset.is_finite() {
        offset.max(0.0)
    } else {
        0.0
    }
}

/// The address a typed `url` loads: trimmed, with a scheme added when none
/// is given, as pen.dev does: an absolute path (`/Users/me/a.html`) becomes
/// a `file://` address, `localhost` or an IP address (with any port) gets
/// `http://`, anything else `https://`. A document-relative path (`./a.html`,
/// `../a.html`, #283) stays as typed; it resolves against the document's
/// folder when the page loads (`browser_live::resolve_page_url`). `None` for
/// a scheme other than http, https or file (a `javascript:` or `data:`
/// address never reaches the page loader). An empty input normalizes to
/// `Some("")`, which clears the address.
pub fn normalize_browser_url(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return Some(String::new());
    }
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("file://")
    {
        return Some(url.to_string());
    }
    if is_document_relative(url) {
        return Some(url.to_string());
    }
    if url.starts_with('/') {
        return Some(format!("file://{url}"));
    }
    if is_local_host(&lower) {
        return Some(format!("http://{url}"));
    }
    if let Some(colon) = lower.find(':') {
        let scheme = &lower[..colon];
        // `localhost:3000` and `example.com:8080` carry a port, not a scheme.
        let is_port = lower[colon + 1..]
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
        if !is_port
            && scheme
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'+')
        {
            return None;
        }
    }
    Some(format!("https://{url}"))
}

/// `./…` or `../…`: a page addressed from the document's folder (#283).
pub fn is_document_relative(url: &str) -> bool {
    url.starts_with("./") || url.starts_with("../")
}

/// `localhost`, a dotted IPv4 address or a bracketed IPv6 address, with an
/// optional port, at the start of a schemeless `lower`-case address.
fn is_local_host(lower: &str) -> bool {
    let end = lower.find(['/', '?', '#']).unwrap_or(lower.len());
    let authority = &lower[..end];
    let host = if authority.starts_with('[') {
        match authority.find(']') {
            Some(close) => {
                let (host, rest) = authority.split_at(close + 1);
                if !rest.is_empty() && !is_port(rest) {
                    return false;
                }
                return host.len() > 2
                    && host[1..host.len() - 1]
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() || b == b':');
            }
            None => return false,
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) if is_port(&format!(":{port}")) => host,
            Some(_) => return false,
            None => authority,
        }
    };
    host == "localhost" || is_ipv4(host)
}

/// `:` followed by one or more digits.
fn is_port(rest: &str) -> bool {
    rest.strip_prefix(':')
        .is_some_and(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
#[path = "browser_devices_tests.rs"]
mod tests;
