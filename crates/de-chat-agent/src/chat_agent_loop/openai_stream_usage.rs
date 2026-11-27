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
//! matching on the text would miss exactly the endpoints this exists for.
//!
//! The retry is only offered until a request carrying the field has
//! succeeded in the run: after that the endpoint has accepted it, so a later
//! 400 (a context overflow, say) is reported at once without an extra
//! request. When the retry also fails, the ORIGINAL refusal is reported, not
//! the retry's error (a timeout or 429 there would hide the real cause). A
//! retry that succeeds marks the endpoint + model for the process, so later
//! requests (this run and later runs) skip the field instead of paying the
//! refused request again. The key includes the model because a gateway may
//! refuse the field for one upstream model and accept it for the others.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::chat_builtin_http::BuiltinHttpError;

/// (url, model) pairs that refused a request carrying `stream_options` and
/// then accepted the same request without it.
fn refusing_endpoints() -> &'static Mutex<HashSet<(String, String)>> {
    static SET: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Whether requests for `model` at `url` should ask for the usage chunk.
pub(super) fn wants_stream_usage(url: &str, model: &str) -> bool {
    refusing_endpoints()
        .lock()
        .map(|set| !set.contains(&(url.to_string(), model.to_string())))
        .unwrap_or(true)
}

/// Remember that `model` at `url` only works without `stream_options`.
pub(super) fn remember_refusal(url: &str, model: &str) {
    if let Ok(mut set) = refusing_endpoints().lock() {
        set.insert((url.to_string(), model.to_string()));
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
