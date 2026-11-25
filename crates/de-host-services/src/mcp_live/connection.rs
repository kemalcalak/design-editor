//! Live-MCP accept loop + per-connection dispatch: the TCP `server_loop`,
//! the JSON-RPC/REST `serve_connection` router, and the lightweight tool
//! fast path that needs no full `EditorState` snapshot. Split out of
//! `mcp_live.rs` to keep the spine under the 800-line cap.

use super::*;

/// RAII decrement for the live-connection counter — see its use in the
/// per-connection thread. `Drop` runs during normal exit AND panic unwind, so a
/// panicking connection can't leak its reserved slot.
pub(super) struct ConnGuard(Arc<AtomicUsize>);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn server_loop(
    listener: TcpListener,
    req_tx: Sender<UiRequest>,
    stop_rx: Receiver<()>,
    admission: Arc<LiveAdmission>,
    quit_flag: Arc<AtomicBool>,
    wake_ui: UiWake,
    client_identity: Arc<Mutex<Option<(String, String)>>>,
    shared: Arc<LiveShared>,
) {
    // `shared` holds the lock that serializes only *stateful* requests, so a
    // concurrent multi-apply batch can't interleave. Stateless probes
    // (`ping`/`initialize`/`tools/list`) bypass it, so `de`'s liveness probe
    // and a client loading its tools are never blocked behind a call.
    // Bounds the number of in-flight connection threads.
    let conn_count = Arc::new(AtomicUsize::new(0));
    loop {
        match stop_rx.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }
        // Name a request that holds the editor too long (#406).
        shared.watchdog();
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                if let Err(e) = stream.set_nonblocking(false) {
                    eprintln!("design-editor mcp: accepted stream blocking mode: {e}");
                    continue;
                }
                if conn_count.load(Ordering::Acquire) >= MAX_LIVE_CONNS {
                    // Shed load rather than spawn unbounded threads.
                    let _ = stream.set_write_timeout(Some(ACCEPT_IDLE_SLEEP));
                    let _ = crate::mcp_serve::write_mcp_http_response(
                        &mut stream,
                        "503 Service Unavailable",
                        r#"{"jsonrpc":"2.0","error":{"code":-32000,"message":"server busy"},"id":null}"#,
                    );
                    continue;
                }
                // Handle each connection on its own thread so a long write
                // (which waits on the UI thread) doesn't stall concurrent
                // pings. Threads are short-lived and detached.
                conn_count.fetch_add(1, Ordering::AcqRel);
                let req_tx = req_tx.clone();
                let admission = Arc::clone(&admission);
                let shared = Arc::clone(&shared);
                let quit = Arc::clone(&quit_flag);
                let conns = Arc::clone(&conn_count);
                let wake = Arc::clone(&wake_ui);
                let identity = Arc::clone(&client_identity);
                let spawned = thread::Builder::new()
                    .name("de-mcp-live-conn".into())
                    .stack_size(LIVE_CONN_STACK_SIZE)
                    .spawn(move || {
                        // RAII: decrement the live-connection count even if
                        // `serve_connection` panics. Without this, a panic would
                        // leak the slot, and after MAX_LIVE_CONNS panics the
                        // server would wedge into permanent load-shedding.
                        let _conn_guard = ConnGuard(conns);
                        let mut stream = stream;
                        let _ = stream.set_read_timeout(Some(UI_ACK_TIMEOUT));
                        let _ = stream.set_write_timeout(Some(UI_ACK_TIMEOUT));
                        if let Err(e) = serve_connection(
                            &mut stream,
                            &req_tx,
                            &admission,
                            &shared,
                            &quit,
                            &wake,
                            &identity,
                        ) {
                            eprintln!("design-editor mcp: {e}");
                            // Busy on a REST route is not a server fault.
                            let status = match e {
                                McpLiveError::Busy(_) => "503 Service Unavailable",
                                _ => "500 Internal Server Error",
                            };
                            let _ = crate::mcp_serve::write_mcp_http_response(
                                &mut stream,
                                status,
                                &error_json(&e.to_string()),
                            );
                        }
                    });
                if spawned.is_err() {
                    // The closure (and its `conns` clone) was dropped without
                    // running — undo the count we reserved above.
                    conn_count.fetch_sub(1, Ordering::AcqRel);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_IDLE_SLEEP);
            }
            Err(e) => {
                eprintln!("design-editor mcp: accept: {e}");
                thread::sleep(ACCEPT_IDLE_SLEEP);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn serve_connection<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    req_tx: &Sender<UiRequest>,
    admission: &LiveAdmission,
    shared: &LiveShared,
    quit_flag: &AtomicBool,
    wake_ui: &UiWake,
    client_identity: &Mutex<Option<(String, String)>>,
) -> Result<(), McpLiveError> {
    // A refused body FRAMING (over a route's declared cap, or missing the
    // `Content-Length` a route requires) is a client fault detected before a
    // single body byte was read — answer it with its own status instead of
    // letting it bubble out as a 500. See `mcp_serve::read_http_request`.
    let req = match crate::mcp_serve::read_http_request(stream) {
        Ok(req) => req,
        Err(crate::mcp_serve::McpServeError::Framing { status, message }) => {
            return write_http_with_origin(
                stream,
                status,
                &crate::mcp_serve::rest_error_body(&message),
                None,
            );
        }
        Err(e) => return Err(e.into()),
    };
    let token = admission.token();
    // The one origin this request may be answered to (never `*`) — computed
    // once and threaded through every reply below, refusals included.
    let cors_origin = admission::cors_origin_for(&req, admission);
    // Gate 1 (see `admission.rs`): browser screening, ahead of ALL routing —
    // including the preflight and the stateless probes, so a foreign page
    // cannot even fingerprint this endpoint. `Host`/`Origin` are not
    // page-forgeable, which is what closes DNS rebinding.
    if let Err(denial) = admission::check_boundary(&req, admission) {
        return write_http_with_origin(
            stream,
            denial.http_status(),
            &admission::denial_json_rpc(&req.body, denial),
            cors_origin,
        );
    }
    if req.method == "OPTIONS" {
        if design_md_route::is_design_md_path(&req.path) {
            return design_md_route::write_preflight(stream, cors_origin);
        }
        // The preflight is scoped exactly like the request it precedes: a
        // browser only proceeds when the reply names ITS origin, so this is
        // what stops a non-accepted extension from reaching the ingress.
        return write_http_with_origin(stream, "204 No Content", "", cors_origin);
    }
    // TS live-canvas whole-document sync (REST `POST /api/mcp/document`),
    // distinct from the JSON-RPC `/mcp` path below. Lets a TS whole-doc-sync
    // client (`setSyncDocument` → POST `{document}`) drive THIS editor's
    // on-screen canvas, mirroring `apps/web/server/api/mcp/document.post.ts`.
    if crate::mcp_serve::is_document_sync_route(&req.method, &req.path) {
        // Whole-document replacement of the live (possibly SHARED) document.
        // The local desktop trusts every
        // caller that clears the `Host`/`Origin` boundary above, so no
        // per-instance token is demanded here. The
        // REST route answers `{ok,error}`, not JSON-RPC.
        return serve_document_sync(stream, req_tx, wake_ui, shared, &req.body);
    }
    // Content-free design-token evidence from the Chrome extension. This
    // route queues an asynchronous LLM request only; it never snapshots or
    // mutates the live document and therefore does not take the state lock.
    if design_md_route::is_design_md_path(&req.path) {
        return design_md_route::serve_design_md(stream, req_tx, wake_ui, &req, cors_origin);
    }
    // Insert-only browser-extension ingress (`POST /api/import/web-snapshot`).
    // Deliberately before the general `/mcp` router — see `snapshot_ingest`
    // for why this one route accepts an extension origin and what that does
    // (and does not) grant.
    if snapshot_ingest::is_snapshot_ingest_route(&req.method, &req.path) {
        return snapshot_ingest::serve_snapshot_ingest(
            stream,
            req_tx,
            wake_ui,
            shared,
            &req,
            cors_origin,
        );
    }
    if req.path != "/mcp" && req.path != "/" {
        return write_http(stream, "404 Not Found", r#"{"error":"Not found"}"#);
    }
    if req.method != "POST" {
        return write_http(
            stream,
            "400 Bad Request",
            r#"{"jsonrpc":"2.0","error":{"code":-32000,"message":"Invalid or missing session ID"},"id":null}"#,
        );
    }
    // Token-authed graceful shutdown: ack, then flag the UI thread to exit
    // the event loop. No pid-kill ⇒ no signal-the-wrong-process race.
    if let Some(id) = crate::mcp_serve::shutdown_request_id(&req.body, token) {
        write_json_rpc_response(stream, &crate::mcp_serve::shutdown_ok_response(&id))?;
        quit_flag.store(true, Ordering::Release);
        wake_ui();
        return Ok(());
    }
    // Stateless handshake/liveness methods must NOT block on the UI thread or
    // take the stateful lock, so `de`'s `ping` probe stays fast and never
    // false-negatives a busy editor. The live `ping` reply carries our
    // identity token so the CLI can confirm THIS server published the file.
    match crate::mcp_serve::classify_stateless(&req.body) {
        crate::mcp_serve::Stateless::Respond(resp) => {
            // Capture the client's declared identity off the SAME
            // `initialize` request `classify_stateless` just answered —
            // the ONLY message this wire protocol carries a name in.
            // Always overwrite (not "first wins"): a later `initialize`
            // means a different tool connected, and the badge should
            // say who is ACTUALLY driving now.
            if let Some(name) = crate::mcp_serve::parse_client_info_name(&req.body) {
                if let Ok(mut identity) = client_identity.lock() {
                    *identity = Some((name, MCP_CLIENT_COLOR.to_string()));
                }
                // An idle window must repaint to name the new client.
                wake_ui();
            }
            return write_json_rpc_response(stream, &resp);
        }
        crate::mcp_serve::Stateless::Swallow => {
            return write_json_rpc_response(stream, "");
        }
        crate::mcp_serve::Stateless::Ping(id) => {
            return write_json_rpc_response(stream, &live_ping_response(&id, token));
        }
        // The catalog is not the document: answered from the kit catalog the
        // UI thread last published, with no lock and no snapshot (#406).
        crate::mcp_serve::Stateless::ToolsList(id) => {
            let response = crate::mcp_serve::tools_list_response_for(
                &id,
                &shared.kit_catalog(),
                de_mcp::debug_tools_enabled(),
            );
            return write_json_rpc_response(stream, &response);
        }
        crate::mcp_serve::Stateless::NeedsState => {}
    }
    match serve_stateful(&req.body, req_tx, shared, wake_ui) {
        Ok(response) => write_json_rpc_response(stream, &response),
        // Loading, painting or behind another request: tell the client to
        // send it again rather than hold it until its own timeout (#406).
        Err(McpLiveError::Busy(busy)) => {
            eprintln!("design-editor mcp: {busy}");
            let response = crate::mcp_serve::busy_error_response(
                &req.body,
                &busy.to_string(),
                busy.reason(),
                BUSY_RETRY_AFTER.as_millis() as u64,
            );
            write_json_rpc_response(stream, &response.unwrap_or_default())
        }
        Err(e) => Err(e),
    }
}

/// A JSON-RPC request that reads or writes the live document, answered
/// under the state lock. Returns the response body; the caller writes it
/// once the lock is released, so a slow or dead client never holds the
/// editor while its socket drains.
fn serve_stateful(
    body: &str,
    req_tx: &Sender<UiRequest>,
    shared: &LiveShared,
    wake_ui: &UiWake,
) -> Result<String, McpLiveError> {
    // Gate 2 (see `admission.rs`): everything from here down reads or writes
    // the live document — every `tools/call`, not just the write ones. The
    // `Host`/`Origin` boundary above is the only gate the local desktop
    // applies.
    // Everything below observes or mutates shared state — the live
    // `EditorState` OR a `--file` document on disk (a read-modify-write).
    // Serialize it under one lock so snapshots stay coherent, concurrent live
    // applies cannot interleave, and file-backed writes cannot race. The wait
    // is bounded (`STATE_LOCK_WAIT`, then busy naming the holder), and a
    // poisoned lock (a panicked sibling connection) is tolerated.
    let _guard = shared.lock_state(&request_label(body))?;
    // pen.dev's pathless `save` saves the open document to its file (#392).
    use crate::mcp_serve::pen_dev_save::{bind_save, SaveDispatch};
    let body = match bind_save(body, shared.document_path().as_deref()) {
        SaveDispatch::Line(body) => body,
        SaveDispatch::Reply(reply) => return Ok(reply),
    };
    let body = body.as_ref();
    // File-backed path (`--file` arg): handle the whole read-modify-write
    // while holding the lock.
    if let Some(response) =
        crate::mcp_serve::file_path::process_message_for_file_path_arg(None, body)?
    {
        // A file-backed save copies another file, not the live document.
        report_saved_document(body, &response, None, req_tx, wake_ui);
        return Ok(response);
    }
    // `debug_screenshot` against the LIVE canvas: route through the
    // raster export pipeline on the UI thread instead of the generic
    // dispatch (whose headless tool can only report no-live-canvas).
    // Gate-closed (`DESIGN_EDITOR_DEBUG_TOOLS` unset) falls through to the
    // generic dispatch, which reports UnknownTool exactly like the
    // headless registry that never registered the tool.
    #[cfg(feature = "mcp-debug-tools")]
    if let Some(response) =
        screenshot::maybe_serve(body, de_mcp::debug_tools_enabled(), |shot_req| {
            request_screenshot(req_tx, wake_ui, shot_req)
        })
    {
        return Ok(response);
    }
    // These tools need either no editor snapshot (`set_active_page`) or only
    // page metadata (`list_pages`). Keep them on the normal MCP
    // parser/registry/serializer path, but do not deep-clone the live document.
    if let Some(response) = process_lightweight_live_tool(body, req_tx, wake_ui, shared)? {
        return Ok(response);
    }
    let (mut state, document_epoch) = request_snapshot_with_epoch(req_tx, wake_ui, shared)?;
    let response = crate::mcp_serve::process_message_with_write_applier(
        &mut state,
        body,
        |tool_name, local_state, write| match request_apply(
            req_tx,
            wake_ui,
            tool_name.to_string(),
            write.command.clone(),
            unplaced_write(write),
        ) {
            Ok(ack) => {
                if ack.applied {
                    let _ = local_state.apply(write.command.clone());
                }
                ack.applied
            }
            Err(e) => {
                eprintln!("design-editor mcp: apply failed: {e}");
                false
            }
        },
    )?
    .unwrap_or_default();
    // Only a `save_document` reply is reported, and that tool applies no
    // write, so this names the snapshot its file was written from.
    let saved_from = McpSavedRevision {
        document_epoch,
        generation: state.document_generation(),
        revision: state.document_revision(),
    };
    report_saved_document(body, &response, Some(saved_from), req_tx, wake_ui);
    Ok(response)
}

/// What the watchdog and busy answers call a request: its tool, and the
/// file it targets when it is not the live document (a `filePath` call
/// loads and saves that file under the lock). Reads only those fields:
/// serde skips the rest of a multi-MB body without copying it.
pub(super) fn request_label(body: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Request<'a> {
        #[serde(borrow)]
        params: Option<Params<'a>>,
    }
    #[derive(serde::Deserialize)]
    struct Params<'a> {
        #[serde(borrow)]
        name: Option<std::borrow::Cow<'a, str>>,
        #[serde(borrow)]
        arguments: Option<FileArgs<'a>>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FileArgs<'a> {
        #[serde(borrow)]
        file_path: Option<std::borrow::Cow<'a, str>>,
        #[serde(borrow)]
        source_file_path: Option<std::borrow::Cow<'a, str>>,
    }
    let params = serde_json::from_str::<Request>(body)
        .ok()
        .and_then(|request| request.params);
    let Some(tool) = params.as_ref().and_then(|params| params.name.as_deref()) else {
        return "MCP request".to_string();
    };
    let file = params
        .as_ref()
        .and_then(|params| params.arguments.as_ref())
        .and_then(|args| {
            args.file_path
                .as_deref()
                .or(args.source_file_path.as_deref())
        })
        .map(str::trim)
        .filter(|path| !path.is_empty() && !path.starts_with("live://"));
    match file {
        Some(path) => format!("{tool} on {path}"),
        None => tool.to_string(),
    }
}

/// Tell the UI about a file a client just wrote with `save_document`, so
/// the host can list it on Home (its preview is already queued) and, when
/// `saved_from` names the live document, bind the file like Save As (#403).
pub(super) fn report_saved_document(
    line: &str,
    response: &str,
    saved_from: Option<McpSavedRevision>,
    req_tx: &Sender<UiRequest>,
    wake_ui: &UiWake,
) {
    if let Some(path) = crate::mcp_serve::saved_document_path(line, response) {
        let saved = McpSavedDocument { path, saved_from };
        if req_tx.send(UiRequest::DocumentSaved(saved)).is_ok() {
            wake_ui();
        }
    }
}

/// Dispatch live tools whose registry snapshot is independent of the full
/// [`EditorState`]. Returning `None` delegates to the general snapshot path.
pub(super) fn process_lightweight_live_tool(
    line: &str,
    req_tx: &Sender<UiRequest>,
    wake_ui: &UiWake,
    shared: &LiveShared,
) -> Result<Option<String>, McpLiveError> {
    let Some(call) = de_mcp::parse_tool_call(line) else {
        return Ok(None);
    };
    let is_write = match call.tool.as_str() {
        "set_active_page" => true,
        "list_pages" => false,
        _ => return Ok(None),
    };

    // A write cannot be withdrawn once queued, so it waits the full apply
    // ack; while the UI is known to be stuck it is not queued at all.
    if is_write && shared.ui_stalled() {
        wake_ui();
        return Err(McpLiveError::Busy(EditorBusy::UiStalled));
    }
    let mut registry = de_mcp::ToolRegistry::default();
    if is_write {
        registry.register(Box::new(de_mcp::set_active_page_snapshot()));
    } else {
        registry.register(Box::new(request_list_pages(req_tx, wake_ui, shared)?));
    }
    Ok(crate::mcp_serve::process_tool_message_with_registry(
        &registry,
        line,
        |tool_name, cmd| {
            if !is_write {
                return false;
            }
            match request_apply(req_tx, wake_ui, tool_name.to_string(), cmd.clone(), None) {
                Ok(ack) => ack.applied,
                Err(e) => {
                    eprintln!("design-editor mcp: apply failed: {e}");
                    false
                }
            }
        },
    )?)
}
