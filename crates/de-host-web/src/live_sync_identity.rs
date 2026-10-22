//! What happens when the tab stops speaking for the account it is showing.
//!
//! Split out of `live_sync_glue.rs` at the 800-line cap: the latch that stops
//! a tab pushing once the daemon has refused its credential.

use super::AUTH_INVALID;

/// Whether the daemon has refused this tab's credential.
pub(crate) fn auth_is_invalid() -> bool {
    AUTH_INVALID.with(std::cell::Cell::get)
}

/// Latch the tab out of pushing after a 401/403.
pub(crate) fn note_auth_invalid() {
    AUTH_INVALID.with(|flag| flag.set(true));
}
