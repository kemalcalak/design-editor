//! Provider-endpoint screening shared by the built-in chat, model
//! discovery, image generation and HTML URL import: which base URLs may be
//! dialed, and the `DESIGN_EDITOR_WEB_AI_ENDPOINT_ALLOWLIST` opt-in for
//! private origins. (The browser credential sync that used to live here went
//! with the web editor, #401.)

use std::net::IpAddr;

pub use crate::web_credentials_error::WebCredentialError;

type Result<T> = std::result::Result<T, WebCredentialError>;

pub const WEB_AI_ENDPOINT_ALLOWLIST_ENV: &str = "DESIGN_EDITOR_WEB_AI_ENDPOINT_ALLOWLIST";

/// Validate a provider endpoint before it is dialed. Any public HTTPS origin
/// passes; private / loopback / metadata targets need an explicit
/// `DESIGN_EDITOR_WEB_AI_ENDPOINT_ALLOWLIST` entry. Connect-time DNS pinning
/// (`chat_builtin_http`) closes the rebinding gap this hostname check leaves.
pub(crate) fn validate_web_provider_base_url(base_url: &str) -> Result<reqwest::Url> {
    let allowlist = std::env::var(WEB_AI_ENDPOINT_ALLOWLIST_ENV).ok();
    validate_web_provider_base_url_with_allowlist(base_url, allowlist.as_deref())
}

fn validate_web_provider_base_url_with_allowlist(
    base_url: &str,
    allowlist: Option<&str>,
) -> Result<reqwest::Url> {
    let url =
        reqwest::Url::parse(base_url.trim()).map_err(|_| WebCredentialError::EndpointInvalid)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(WebCredentialError::EndpointNotAllowed);
    }
    if endpoint_is_explicitly_allowlisted(&url, allowlist) {
        return Ok(url);
    }
    if url.scheme() != "https" {
        return Err(WebCredentialError::EndpointRequiresHttps);
    }

    let host = url
        .host_str()
        .expect("host presence checked above")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.parse::<IpAddr>().is_ok_and(is_restricted_ip) || is_restricted_hostname(&host) {
        return Err(WebCredentialError::EndpointNotAllowed);
    }
    Ok(url)
}

/// Whether a raw base URL is an exact scheme/host/port match for an
/// `DESIGN_EDITOR_WEB_AI_ENDPOINT_ALLOWLIST` entry. Unparseable URLs are not.
pub(crate) fn base_url_is_explicitly_allowlisted(base_url: &str, allowlist: Option<&str>) -> bool {
    reqwest::Url::parse(base_url.trim())
        .is_ok_and(|url| endpoint_is_explicitly_allowlisted(&url, allowlist))
}

fn endpoint_is_explicitly_allowlisted(url: &reqwest::Url, allowlist: Option<&str>) -> bool {
    allowlist
        .into_iter()
        .flat_map(|value| value.split(','))
        .any(|entry| {
            let Ok(allowed) = reqwest::Url::parse(entry.trim()) else {
                return false;
            };
            matches!(allowed.scheme(), "http" | "https")
                && allowed.host_str().is_some()
                && allowed.username().is_empty()
                && allowed.password().is_none()
                && allowed.query().is_none()
                && allowed.fragment().is_none()
                && url.scheme() == allowed.scheme()
                && url
                    .host_str()
                    .zip(allowed.host_str())
                    .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
                && url.port_or_known_default() == allowed.port_or_known_default()
        })
}

fn is_restricted_hostname(host: &str) -> bool {
    host.is_empty()
        || !host.contains('.')
        || [
            "localhost",
            ".localhost",
            ".local",
            ".internal",
            ".home",
            ".lan",
            ".test",
            ".invalid",
        ]
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(suffix))
        || matches!(
            host,
            "metadata.google.internal"
                | "metadata.google"
                | "instance-data"
                | "instance-data.ec2.internal"
        )
}

// The reserved-address classifiers moved to `de_chat_agent::ip_screen`
// (pure code motion) with the connect-time dial guard; re-imported so every
// `web_credentials::is_restricted_ip` path stays valid.
pub(crate) use de_chat_agent::ip_screen::is_restricted_ip;
#[allow(unused_imports)]
pub(crate) use de_chat_agent::ip_screen::{is_restricted_ipv4, is_restricted_ipv6};

#[cfg(test)]
#[path = "web_credentials_tests.rs"]
mod tests;
