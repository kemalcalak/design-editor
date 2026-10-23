//! One-connection handling: route dispatch across static assets, SSE, REST
//! and JSON-RPC (`serve_one`), plus the SSE writer it hands long-lived
//! subscribers to. Split out of `web_canvas_server.rs` to keep the spine
//! under the 800-line cap; the `/api/ai/*` branch lives in the sibling
//! `connection_ai_routes.rs` for the same reason.

use super::*;

/// Everything one request is served against: the daemon's one document
/// authority, its SSE hub, and the mode it was launched in.
pub(super) struct ConnCtx<'a> {
    pub(super) state: &'a Mutex<WebCanvasState>,
    pub(super) hub: &'a SseHub,
    pub(super) mode: ServeMode,
}

/// Handle one connection against the single-user document authority.
///
/// The `Local`-mode entry point in the two-argument shape the connection
/// tests were written against, so those tests keep proving that the
/// parameterisation below did not change local behaviour. Production callers
/// name their mode via [`serve_one_in_mode`].
#[cfg(test)]
pub(super) fn serve_one<S: Read + Write>(
    stream: &mut S,
    state: &Mutex<WebCanvasState>,
    hub: &SseHub,
) -> Result<bool> {
    serve_one_in_mode(stream, state, hub, ServeMode::Local)
}

/// Handle one connection. Routes: static host page + wasm bundle (`GET /`,
/// `GET /pkg/*` via `crate::web_static`); SSE live-update stream (`GET
/// /api/mcp/events`); REST whole-doc sync / health (`/api/*` via
/// [`handle_web_canvas_request`]); else JSON-RPC `/mcp` tool dispatch. A
/// mutation (REST POST or a mutating tool call) bumps the version and is
/// broadcast to SSE subscribers. The state `Mutex` is held only across the
/// in-memory operation, never across the (long-lived) SSE wait.
///
/// Managed mode is a single-document, single-operator daemon. Its supervisor
/// already owns the process through the stdin lease, so ordinary HTTP requests
/// do not carry a per-request token. Browser requests are still constrained by
/// the explicit supervisor origin allowlist plus the daemon's own exact
/// loopback origin: an unrelated `Origin` is rejected before route dispatch,
/// while native clients without an `Origin` remain usable. The existing
/// sensitive-POST Host/Origin and JSON-content checks below remain in force in
/// every single-user mode.
///
/// Returns `Ok(true)` when the client requested a token-authenticated graceful
/// shutdown (same `design-editor/shutdown` body contract as `--mcp-http`) — the
/// caller then stops the accept loop so `de stop` never signals a pid. In
/// managed mode only this lifecycle request, not ordinary traffic, uses the
/// token emitted in the startup handshake.
pub(super) fn serve_one_in_mode<S: Read + Write>(
    stream: &mut S,
    state: &Mutex<WebCanvasState>,
    hub: &SseHub,
    mode: ServeMode,
) -> Result<bool> {
    let req = crate::mcp_serve::read_http_request(stream)?;
    dispatch(stream, &req, &ConnCtx { state, hub, mode })
}

/// Route one already-parsed request against `ctx`.
pub(super) fn dispatch<S: Read + Write>(
    stream: &mut S,
    req: &crate::mcp_serve::HttpRequest,
    ctx: &ConnCtx<'_>,
) -> Result<bool> {
    let state = ctx.state;
    let hub = ctx.hub;
    let (managed_lifecycle_token, allow_origins) = {
        let guard = state.lock().unwrap_or_else(|p| p.into_inner());
        (guard.managed_token.clone(), guard.allow_origins.clone())
    };
    // Managed mode keeps its own allowlist echo, and the local daemon keeps
    // the permissive value it has always used.
    let cors_origin: Option<String> = if matches!(ctx.mode, ServeMode::Managed) {
        cors_origin_for(&allow_origins, req.origin.as_deref(), req.host.as_deref())
    } else {
        Some("*".to_string())
    };
    let cors_origin = cors_origin.as_deref();
    if matches!(ctx.mode, ServeMode::Managed)
        && !managed_request_origin_allowed(
            &allow_origins,
            req.origin.as_deref(),
            req.host.as_deref(),
        )
    {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "403 Forbidden",
            &crate::mcp_serve::rest_error_body("request origin is not allowed"),
            cors_origin,
        )?;
        return Ok(false);
    }
    if req.method == "OPTIONS" {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "204 No Content",
            "",
            cors_origin,
        )?;
        return Ok(false);
    }
    if is_sensitive_browser_post(req) && !credential_request_origin_allowed(req) {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "403 Forbidden",
            &crate::mcp_serve::rest_error_body("cross-origin sensitive request is forbidden"),
            cors_origin,
        )?;
        return Ok(false);
    }
    // Sensitive JSON routes refuse CORS "simple request" content types
    // (text/plain, form-encoded, or none): a drive-by page can fire those
    // without a preflight, and unmanaged daemons have no token gate.
    if is_sensitive_browser_post(req) && !content_type_is_json(req.content_type.as_deref()) {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "415 Unsupported Media Type",
            &crate::mcp_serve::rest_error_body(
                "this route requires Content-Type: application/json",
            ),
            cors_origin,
        )?;
        return Ok(false);
    }
    // Static serving: the host page (`/`) and the wasm-bindgen bundle
    // (`/pkg/*`). Owns only those paths — everything else falls through.
    if req.method == "GET" {
        let bundle_dir = crate::web_static::resolve_bundle_dir();
        if let Some(reply) =
            crate::web_static::handle_static_request(&req.path, bundle_dir.as_deref())
        {
            return crate::web_static::write_static_response(stream, &reply, cors_origin)
                .map(|()| false);
        }
    }
    // There is deliberately no managed-mode request-token gate here. Managed
    // is a single-operator child process whose authority comes from the local
    // supervisor's stdin lease and the browser Origin boundary above.
    // SSE live-update stream: the browser shell subscribes and re-syncs whenever
    // the document version advances. Subscribe BEFORE reading the current
    // version so no broadcast is missed (a duplicate is harmless — versions are
    // monotonic). The state lock is released before the long SSE wait.
    if req.method == "GET" && req.path == "/api/mcp/events" {
        let slot = hub.subscribe();
        let current = state.lock().unwrap_or_else(|p| p.into_inner()).sse_tick();
        return serve_sse(stream, &slot, current, cors_origin).map(|()| false);
    }
    // The AI / image routes, which parse under the lock and then run long
    // network on this connection thread. See `connection_ai_routes.rs`.
    if let Some(done) = connection_ai_routes::serve_ai_route(stream, req, ctx, cors_origin)? {
        return Ok(done);
    }
    // Offline `.fig` -> `.op` convert for the VS Code plugin: it can't parse
    // fig-kiwi itself, so it POSTs the raw bytes here and boots the returned
    // document JSON through its normal open-document push. Conversion is
    // pure (no network, no state) so — unlike the image routes above — it
    // runs on this connection's own thread without ever touching the state
    // lock; only large/slow parsing needs to stay off it.
    if req.method == "POST" && req.path == "/api/figma/convert" {
        let (status, body) = match crate::figma_convert::convert_fig_json(&req.body) {
            Ok(body) => ("200 OK", body),
            Err(error) => (
                "400 Bad Request",
                serde_json::json!({ "ok": false, "error": error.to_string() }).to_string(),
            ),
        };
        crate::mcp_serve::write_mcp_http_response_with_origin(stream, status, &body, cors_origin)?;
        return Ok(false);
    }
    // All `/api/mcp/*` REST paths go to the REST handler — including ones this
    // daemon doesn't implement yet, which it answers with 404 rather than
    // mis-routing them into the JSON-RPC dispatch below.
    if req.path.starts_with("/api/") {
        // Parse the whole-document push BEFORE taking the state lock. A push
        // can carry megabytes of embedded images, and parsing it under the
        // lock stalled every other request to this daemon — including the
        // version probe that tells the browser anything changed. Nothing here
        // reads live state; `baseVersion` is deliberately re-checked inside
        // the lock, where it is not a race.
        let pending_push = (req.method == "POST" && req.path == "/api/mcp/document")
            .then(|| PendingDocumentPush::parse(&req.body));
        let reply = {
            let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
            let before = guard.version;
            let settings_before = crate::settings_io::fingerprint(&guard.editor);
            let credential_settings_before = (req.method == "POST"
                && req.path == "/api/settings/credentials")
                .then(|| guard.editor.editor_ui.agent_settings.clone());
            let reply = match pending_push {
                // The pre-parsed push installs directly; the generic handler
                // never sees this route's body twice.
                Some(parsed) => document_push_reply(parsed, &mut guard),
                None => handle_web_canvas_request(&req.method, &req.path, &req.body, &mut guard),
            };
            let reply = persist_api_settings(
                &req.method,
                &req.path,
                &mut guard,
                settings_before,
                credential_settings_before,
                reply,
                crate::settings_io::save_checked,
            );
            // Broadcast INSIDE the state lock so the version bump and its
            // broadcast are atomic — otherwise two concurrent mutations could
            // broadcast their versions out of order (SSE clients seeing N then
            // N-1). `broadcast` only sends to unbounded channels (non-blocking),
            // so the lock is held briefly. Lock order is always state→hub.
            if guard.version != before {
                hub.broadcast(guard.sse_tick());
            }
            reply
        };
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            reply.status,
            &reply.body,
            cors_origin,
        )?;
        return Ok(false);
    }
    // JSON-RPC tool dispatch is served ONLY as a POST to `/` or `/mcp`. An
    // unknown path is 404; a known path with the wrong method (e.g. `GET /mcp`)
    // is 405 — never silently dispatched as a tool call.
    if req.path != "/mcp" && req.path != "/" {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "404 Not Found",
            r#"{"ok":false,"error":"Not found. Use /, /pkg/*, /api/mcp/document, /api/mcp/sync-reset, /api/mcp/server, /api/mcp/events, /api/file/save, /api/export/raster, /api/export/pdf, or /mcp."}"#,
            cors_origin,
        )?;
        return Ok(false);
    }
    if req.method != "POST" {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "405 Method Not Allowed",
            r#"{"ok":false,"error":"Method not allowed. POST a JSON-RPC message to /mcp."}"#,
            cors_origin,
        )?;
        return Ok(false);
    }
    // Token-authenticated graceful shutdown (`de stop`): same body-carried
    // token contract as the `--mcp-http` server. Managed mode accepts the
    // startup-handshake token here for lifecycle compatibility, but never as
    // an ordinary request header; local operator shutdown continues to use
    // only DESIGN_EDITOR_MCP_TOKEN. A stale file, recycled pid, or random
    // client therefore cannot shut the daemon down.
    let operator_token = crate::mcp_serve::headless_token_from_env();
    let managed_shutdown = matches!(ctx.mode, ServeMode::Managed)
        .then_some(managed_lifecycle_token.as_deref())
        .flatten()
        .and_then(|token| crate::mcp_serve::shutdown_request_id(&req.body, token));
    let operator_shutdown = operator_token
        .as_deref()
        .and_then(|token| crate::mcp_serve::shutdown_request_id(&req.body, token));
    if let Some(id) = managed_shutdown.or(operator_shutdown) {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "200 OK",
            &crate::mcp_serve::shutdown_ok_response(&id),
            cors_origin,
        )?;
        return Ok(true);
    }
    // `debug_screenshot` for `--serve-web`: the browser shell mirrors this
    // daemon's document, so the daemon can satisfy the live screenshot tool from
    // the same raster export path desktop live MCP uses. Keep this ahead of the
    // generic dispatch, whose headless debug tool can only report no live
    // canvas.
    #[cfg(feature = "mcp-debug-tools")]
    if let Some(response) = {
        let guard = state.lock().unwrap_or_else(|p| p.into_inner());
        crate::mcp_live::screenshot::maybe_serve(
            &req.body,
            de_mcp::debug_tools_enabled(),
            |shot_req| {
                let spec = crate::mcp_live::screenshot::capture_spec(&shot_req);
                crate::export::screenshot::capture(&guard.editor, &spec)
            },
        )
    } {
        crate::mcp_serve::write_mcp_http_response_with_origin(
            stream,
            "200 OK",
            &response,
            cors_origin,
        )?;
        return Ok(false);
    }
    // JSON-RPC `/mcp` dispatch against the in-memory document. A mutating apply
    // bumps the sync version, broadcast to SSE subscribers so the browser shell
    // sees JSON-RPC-driven changes too.
    let response = {
        let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
        let before = guard.version;
        let mut applied_any = false;
        // Mechanical passthrough only — this daemon (`--serve-web`/`de
        // start`) is a SEPARATE request loop from `mcp_live.rs`'s
        // `McpLiveServer` (desktop `--live-mcp`), not the same struct;
        // wiring canvas-generation indicators here (so a `batch_design`
        // call against a headless `de start` daemon also relays the
        // radar-scan to the browser shell) is tracked as follow-up
        // scope, not part of this pass.
        let response = crate::mcp_serve::process_message_with_applier(
            &mut guard.editor,
            &req.body,
            |tool_name, editor, cmd| {
                let baseline = crate::mcp_serve::WriteBaseline::for_tool(tool_name, editor);
                let ok = editor.apply(cmd.clone());
                if let Some(baseline) = baseline.filter(|_| ok) {
                    let scope = baseline.scope_after(editor);
                    crate::mcp_serve::run_write_repairs_after_apply(editor, &scope);
                }
                applied_any |= ok;
                ok
            },
        )?
        .unwrap_or_default();
        if applied_any {
            guard.version += 1;
        }
        // Atomic bump+broadcast under the state lock (see the REST path) so SSE
        // version events stay monotonic across concurrent mutations.
        if guard.version != before {
            hub.broadcast(guard.sse_tick());
        }
        response
    };
    let status = if response.is_empty() {
        "202 Accepted"
    } else {
        "200 OK"
    };
    crate::mcp_serve::write_mcp_http_response_with_origin(stream, status, &response, cors_origin)?;
    Ok(false)
}

/// Stream Server-Sent Events to a subscribed client: write the SSE headers,
/// emit the current tick immediately (initial sync), then forward each
/// bump from `rx` as a `data: {"version":N}` event. A periodic
/// heartbeat comment keeps the connection alive AND detects a disconnected
/// client (the write fails once the socket is gone). Returns when the client
/// disconnects (write error) or the hub is dropped.
pub(super) fn serve_sse<S: Write>(
    stream: &mut S,
    slot: &SseSlot,
    current: SseTick,
    cors_origin: Option<&str>,
) -> Result<()> {
    let cors_line = cors_origin
        .map(|origin| format!("Access-Control-Allow-Origin: {origin}\r\n"))
        .unwrap_or_default();
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/event-stream\r\n\
         Cache-Control: no-cache\r\n\
         Connection: keep-alive\r\n\
         {cors_line}\r\n"
    );
    stream
        .write_all(headers.as_bytes())
        .map_err(|e| WebCanvasError::Transport(format!("sse headers: {e}")))?;
    write_sse_event(stream, current)?;
    loop {
        // The slot holds only the newest tick, so coalescing is structural:
        // a burst of mutations behind a slow client collapses to one event
        // rather than queueing one entry per mutation.
        match slot.take_latest(SSE_HEARTBEAT) {
            Some(tick) => write_sse_event(stream, tick)?,
            None => {
                // SSE comment heartbeat — no-op for the client, but a failed
                // write here is how we notice it disconnected, which is also
                // what ends this loop and drops the slot.
                stream
                    .write_all(b": ping\n\n")
                    .map_err(|e| WebCanvasError::Transport(format!("sse heartbeat: {e}")))?;
                stream
                    .flush()
                    .map_err(|e| WebCanvasError::Transport(format!("sse flush: {e}")))?;
            }
        }
    }
}

/// Format + write one SSE `data:` event carrying the sequence number.
pub(super) fn write_sse_event<S: Write>(stream: &mut S, tick: SseTick) -> Result<()> {
    let event = format!("data: {{\"version\":{}}}\n\n", tick.version);
    stream
        .write_all(event.as_bytes())
        .map_err(|e| WebCanvasError::Transport(format!("sse write: {e}")))?;
    stream
        .flush()
        .map_err(|e| WebCanvasError::Transport(format!("sse flush: {e}")))
}
