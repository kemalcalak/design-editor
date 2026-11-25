//! Typed verdicts of the provider-endpoint screen (`web_credentials.rs`).
//!
//! Style follows `de_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebCredentialError {
    /// The endpoint is not a parseable URL.
    EndpointInvalid,
    /// The endpoint parsed but is refused: a non-HTTP(S) scheme, embedded
    /// credentials, a query/fragment, or — absent an explicit allowlist entry
    /// — a private, loopback, link-local, or cloud-metadata target.
    EndpointNotAllowed,
    /// A plain-HTTP endpoint that is not explicitly allowlisted.
    EndpointRequiresHttps,
}

impl fmt::Display for WebCredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebCredentialError::EndpointInvalid => {
                f.write_str("browser provider endpoint is invalid")
            }
            WebCredentialError::EndpointNotAllowed => {
                f.write_str("browser provider endpoint is not allowed")
            }
            WebCredentialError::EndpointRequiresHttps => {
                f.write_str("browser provider endpoint must use HTTPS")
            }
        }
    }
}

impl std::error::Error for WebCredentialError {}
