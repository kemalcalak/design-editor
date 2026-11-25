//! TS-parity whole-document sync (`POST /api/mcp/document`) and the shared
//! JSON-RPC response writer. Split out of `mcp_live.rs` to keep the spine
//! under the 800-line cap.

use super::*;

/// Monotonic counter for the whole-document sync `version` reported back to a
/// TS whole-doc-sync client (parity with `setSyncDocument`'s monotonic
/// version). Process-global so concurrent connections still get distinct,
/// increasing versions; the exact starting value is not part of the contract.
pub(super) static LIVE_SYNC_VERSION: AtomicU64 = AtomicU64::new(0);

/// Handle a TS-style whole-document sync (`POST /api/mcp/document`): validate
/// the body, swap the document into the live `EditorState` on the UI thread,
/// and reply `{ok:true,version}` (or HTTP 400 with `{ok:false,error}`).
pub(super) fn serve_document_sync<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    req_tx: &Sender<UiRequest>,
    wake_ui: &UiWake,
    shared: &LiveShared,
    body: &str,
) -> Result<(), McpLiveError> {
    let request = match crate::mcp_serve::parse_document_sync_request(body) {
        Ok(request) => request,
        // A refused envelope is a client fault → 400, matching the TS
        // validation 400s in `document.post.ts`. `Display` is transparent,
        // so the body text is unchanged.
        Err(e) => {
            return write_http(
                stream,
                "400 Bad Request",
                &crate::mcp_serve::rest_error_body(&e.to_string()),
            );
        }
    };
    let editor_meta = request.resolved_editor_meta(request.embedded_editor_meta.clone());
    if request.metadata_only {
        let result = shared
            .lock_state("document metadata sync")
            .and_then(|_guard| request_update_editor_meta(req_tx, wake_ui, editor_meta));
        return match result {
            Ok(()) => write_http(
                stream,
                "200 OK",
                &crate::mcp_serve::document_sync_ok(LIVE_SYNC_VERSION.load(Ordering::Relaxed)),
            ),
            Err(e) => write_sync_failure(stream, &e),
        };
    }
    // Load OFF the UI thread (parse only — no skia; layout runs at paint) so a
    // large document never pins the UI thread or holds the lock during the
    // parse. A load failure is a client fault → 400, like the TS validation
    // 400s in `document.post.ts`.
    let loaded = match de_pen_loader::load_canonical(request.document_json) {
        Ok(loaded) => loaded,
        Err(e) => {
            return write_http(
                stream,
                "400 Bad Request",
                &crate::mcp_serve::rest_error_body(&e.to_string()),
            );
        }
    };
    for w in &loaded.warnings {
        eprintln!("design-editor mcp: document-sync schema warning: {w:?}");
    }
    // Serialize the swap against concurrent live writes (same lock the JSON-RPC
    // apply path takes). The lock is now held only for the fast in-memory
    // replace, not the parse — so a huge document can't time out mid-load while
    // still eventually mutating the doc.
    // The version is minted under the lock so it follows the apply order;
    // the reply is written after the lock is released.
    let result = shared.lock_state("document sync").and_then(|_guard| {
        request_replace(req_tx, wake_ui, loaded.value, editor_meta)
            .map(|()| LIVE_SYNC_VERSION.fetch_add(1, Ordering::Relaxed) + 1)
    });
    match result {
        Ok(version) => write_http(
            stream,
            "200 OK",
            &crate::mcp_serve::document_sync_ok(version),
        ),
        Err(e) => write_sync_failure(stream, &e),
    }
}

/// A sync the editor could not take: busy (#406) is 503, telling the
/// client to send it again; a UI thread that is gone or did not ack is a
/// server fault, 500 (TS throws → 500 for server-side failures).
fn write_sync_failure<S: std::io::Write>(
    stream: &mut S,
    error: &McpLiveError,
) -> Result<(), McpLiveError> {
    let status = match error {
        McpLiveError::Busy(_) => "503 Service Unavailable",
        _ => "500 Internal Server Error",
    };
    write_http(
        stream,
        status,
        &crate::mcp_serve::rest_error_body(&error.to_string()),
    )
}

pub(super) fn write_json_rpc_response<S: std::io::Write>(
    stream: &mut S,
    response: &str,
) -> Result<(), McpLiveError> {
    if response.is_empty() {
        write_http(stream, "202 Accepted", "")
    } else {
        write_http(stream, "200 OK", response)
    }
}

/// `mcp_serve::write_mcp_http_response` lifted into this module's error type.
/// Every handler here answers by `return`ing a write, so without the lift each
/// one would spell out `Ok(mcp_serve::write_mcp_http_response(..)?)` — this
/// keeps them one-liners and puts the `McpServeError` → [`McpLiveError`] hop
/// in exactly one place.
pub(super) fn write_http<S: std::io::Write>(
    stream: &mut S,
    status: &str,
    body: &str,
) -> Result<(), McpLiveError> {
    Ok(crate::mcp_serve::write_mcp_http_response(
        stream, status, body,
    )?)
}

/// Same lift for the origin-scoped writer. `cors_origin` is echoed verbatim
/// as `Access-Control-Allow-Origin` when `Some`, and the header is omitted
/// entirely when `None`. Callers on this endpoint pass the ONE origin the
/// admission boundary accepted (`admission::cors_origin_for`) — the
/// permissive `*` that [`write_http`] emits is not a value any browser-facing
/// route here should answer with.
pub(super) fn write_http_with_origin<S: std::io::Write>(
    stream: &mut S,
    status: &str,
    body: &str,
    cors_origin: Option<&str>,
) -> Result<(), McpLiveError> {
    Ok(crate::mcp_serve::write_mcp_http_response_with_origin(
        stream,
        status,
        body,
        cors_origin,
    )?)
}
