//! `stream_options.include_usage` for the OpenAI-compatible agent loop
//! (#446).
//!
//! OpenAI's own streaming API sends the trailing `usage` chunk only when the
//! request asks for it, so without this field a run on OpenAI reports null
//! tokens. The field is part of OpenAI's documented request schema and every
//! built-in OpenAI-compatible preset either documents it or ignores it, but a
//! Custom endpoint (a strict proxy, a validating self-hosted server) may
//! refuse an unknown body field with a 400 (or 422 from a pydantic
//! `extra="forbid"` model).
//!
//! The loop therefore always asks, and when a request carrying the field is
//! refused with one of those statuses it retries ONCE without it. The trigger
//! is the status, not the error body: `send_with_backoff` never reads a
//! provider's body (it is untrusted and may echo credentials), and endpoints
//! word the refusal differently — some do not name the field at all, so
//! matching on the text would miss exactly the endpoints this exists for. A
//! 400 with another cause costs one extra refused request and then reports
//! the original error. A retry that succeeds marks the endpoint, so later
//! requests (this run and later runs in this process) skip the field instead
//! of paying the refused request again.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::chat_builtin_http::BuiltinHttpError;

/// Endpoints that refused a request carrying `stream_options` and then
/// accepted the same request without it.
fn refusing_endpoints() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Whether requests to `url` should ask for the trailing usage chunk.
pub(super) fn wants_stream_usage(url: &str) -> bool {
    refusing_endpoints()
        .lock()
        .map(|set| !set.contains(url))
        .unwrap_or(true)
}

/// Remember that `url` only works without `stream_options`.
pub(super) fn remember_refusal(url: &str) {
    if let Ok(mut set) = refusing_endpoints().lock() {
        set.insert(url.to_string());
    }
}

/// Ask for the trailing usage chunk, or take the request back out.
pub(super) fn set_stream_usage(body: &mut Value, include: bool) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    if include {
        obj.insert("stream_options".into(), json!({ "include_usage": true }));
    } else {
        obj.remove("stream_options");
    }
}

/// A refusal that may be the endpoint rejecting the `stream_options` field.
pub(super) fn may_be_field_refusal(err: &BuiltinHttpError) -> bool {
    matches!(
        err,
        BuiltinHttpError::HttpStatus { status, .. }
            if *status == reqwest::StatusCode::BAD_REQUEST
                || *status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
    )
}
