//! Stdio MCP server mode for the desktop binary.
//!
//! `design-editor --mcp <path>` runs JSON-RPC stdio against a
//! `.de` file. External CLIs can spawn this mode to drive the Rust
//! editor the way they drive TS pen-mcp today.
//! The server backs the `.de` file with an `de_editor_core::
//! EditorState` (the canonical `jian_ops_schema::PenDocument`), not
//! the old shell-core `Document`. Loading a `.de` into a `PenDocument`
//! is plain `jian-ops-schema` deserialization — no `pen_doc_adapter`
//! needed for this path. Write tools apply through
//! `EditorState::apply(EditorCommand)`; on every successful write the
//! `PenDocument` is serialized straight back to disk.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::import_html_url::import_html_url_snapshot;
use de_editor_core::{EditorCommand, EditorState};
use de_mcp::import_html_tool::import_html_snapshot;
use de_mcp::import_pen_dev_tool::import_pen_dev_snapshot;
use de_mcp::import_snapshot_tool::import_web_snapshot_tool;
use de_mcp::{
    add_node_effect_snapshot, add_page_snapshot, align_selected_snapshot,
    apply_design_system_snapshot, batch_design_snapshot, batch_get_snapshot,
    clear_selection_snapshot, codegen_assemble_snapshot, codegen_clean_snapshot,
    codegen_export_snapshot, codegen_plan_snapshot, codegen_submit_chunk_snapshot,
    conversion_status_snapshot, copy_node_snapshot, copy_selected_snapshot, count_nodes_snapshot,
    create_component_snapshot, create_variable_snapshot, cut_selected_snapshot,
    cycle_active_axis_value_snapshot, debug_tools_enabled, delete_component_snapshot,
    delete_node_snapshot, delete_page_snapshot, delete_selected_snapshot, delete_variable_snapshot,
    design_content_snapshot, design_refine_snapshot, design_skeleton_snapshot,
    document_info_snapshot, duplicate_page_snapshot, duplicate_selected_snapshot,
    export_design_md_snapshot, find_empty_space_snapshot, find_node_by_name_snapshot,
    get_active_theme_snapshot, get_canvas_bounds_snapshot, get_component_snapshot,
    get_design_agent_prompt_snapshot, get_design_md_snapshot, get_design_prompt_snapshot,
    get_editor_state_snapshot, get_guidelines_snapshot, get_history_depth_snapshot,
    get_node_children_snapshot, get_node_parent_snapshot, get_node_snapshot,
    get_selection_set_snapshot, get_style_guide_snapshot, get_style_guide_tags_snapshot,
    get_variables_snapshot, get_viewport_snapshot, group_selected_snapshot, import_svg_snapshot,
    insert_node_snapshot, instantiate_component_snapshot, lint_document_snapshot,
    list_components_snapshot, list_node_kinds_snapshot, list_pages_snapshot,
    list_style_guides_snapshot, list_theme_presets_snapshot, list_ui_kits_snapshot,
    list_variables_snapshot, load_theme_preset_snapshot, move_node_snapshot,
    nudge_selected_snapshot, open_document_snapshot, paste_clipboard_snapshot, read_nodes_snapshot,
    redo_snapshot, remove_node_effect_snapshot, remove_page_snapshot, rename_component_snapshot,
    rename_page_snapshot, rename_variable_snapshot, reorder_page_snapshot,
    reorder_selected_snapshot, replace_all_matching_properties_snapshot, replace_node_snapshot,
    run_stdio_with_applier, save_document_snapshot, save_theme_preset_snapshot,
    search_all_unique_properties_snapshot, selection_snapshot, set_active_axis_value_snapshot,
    set_active_page_snapshot, set_active_tool_snapshot, set_design_md_snapshot,
    set_ellipse_arc_snapshot, set_node_collapsed_snapshot, set_node_corner_radius_snapshot,
    set_node_fill_hex_snapshot, set_node_flip_snapshot, set_node_font_size_snapshot,
    set_node_font_weight_snapshot, set_node_hidden_snapshot, set_node_locked_snapshot,
    set_node_name_snapshot, set_node_rotation_snapshot, set_node_stroke_hex_snapshot,
    set_node_stroke_side_width_snapshot, set_node_stroke_style_snapshot,
    set_node_stroke_width_snapshot, set_node_text_snapshot, set_selection_set_snapshot,
    set_selection_snapshot, set_shader_source_snapshot, set_shader_uniform_snapshot,
    set_themes_snapshot, set_variable_boolean_snapshot, set_variable_color_snapshot,
    set_variable_number_snapshot, set_variable_string_snapshot, set_variables_snapshot,
    set_viewport_snapshot, snapshot_layout_snapshot, spawn_agents_snapshot,
    toggle_node_selection_snapshot, tool_search_snapshot, undo_snapshot, ungroup_selected_snapshot,
    update_node_snapshot, upsert_component_snapshot, upsert_screen_snapshot,
    upsert_variables_snapshot, McpTool, ToolRegistry,
};
#[cfg(feature = "mcp-debug-tools")]
use de_mcp::{
    debug_logs_tail_snapshot, debug_screenshot_snapshot, debug_validation_report_snapshot,
};

pub(crate) mod auto_finalize;
pub mod error;
pub mod file_path;
mod preview_hook;
mod sniff;
mod write_repairs;

pub use error::McpServeError;
pub(crate) use preview_hook::saved_document_path;
use sniff::{sniff_id_raw, sniff_method};
#[cfg(test)]
use write_repairs::writes_verbatim;
pub(crate) use write_repairs::{baseline_for_tool, run_write_repairs_after_apply, WriteScope};

/// Unwrap an MCP `tools/call` reply to the inner tool-result JSON text,
/// so tests assert on the flat result fields directly. Strips an HTTP
/// envelope first (live-server tests pass the raw HTTP response).
/// Mirrors the CLI's `unwrap_mcp_reply`. Public (not `#[cfg(test)]`) so
/// de-host-desktop's tests reach it across the crate boundary.
#[doc(hidden)]
pub fn tool_text(response: &str) -> String {
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or(response)
        .trim();
    let value: serde_json::Value = serde_json::from_str(body).expect("json-rpc reply");
    value["result"]["content"][0]["text"]
        .as_str()
        .expect("tools/call content text")
        .to_string()
}

/// Load a `.de` file into an `EditorState` via the schema compat layer.
pub fn load_editor_state(path: &Path) -> Result<EditorState, McpServeError> {
    crate::doc_io::load_editor_state(path, de_editor_core::Locale::EnUs)
        .map_err(|error| McpServeError::Document(format!("load {}: {error}", path.display())))
}

/// Serialize through the same streaming, atomic path as desktop Save, then
/// refresh the file's Home preview.
fn save_editor_state(state: &EditorState, path: &Path) -> Result<(), McpServeError> {
    crate::doc_io::save_to_path(state, path)
        .map_err(|error| McpServeError::Document(format!("save {}: {error}", path.display())))?;
    preview_hook::after_save(path);
    Ok(())
}

/// Process one JSON-RPC message line against the editor state.
#[cfg(test)]
fn process_message(
    state: &mut EditorState,
    path: &Path,
    line: &str,
) -> Result<Option<String>, McpServeError> {
    process_message_with_auto_finalize(state, path, line, None)
}

fn process_message_with_auto_finalize(
    state: &mut EditorState,
    path: &Path,
    line: &str,
    mut auto_finalize: Option<&mut auto_finalize::AutoFinalize>,
) -> Result<Option<String>, McpServeError> {
    if let Some(response) = file_path::process_message_for_file_path_arg(Some(path), line)? {
        return Ok(Some(response));
    }
    generate_tool::land_generations_and_save(state, path);
    let explicit_finalize =
        de_mcp::parse_tool_call(line.trim()).is_some_and(|call| call.tool == "finalize_design");
    let mut applier_failed: Option<String> = None;
    let mut apply_rejected = false;
    // File-backed mode has no live canvas for any tool call to animate; the
    // tool name only decides whether the write lands verbatim.
    let response = process_message_with_applier(state, line, |tool_name, state, cmd| {
        let baseline = baseline_for_tool(tool_name, state);
        // `EditorState::apply` runs the pre-validate-then-mutate
        // discipline; `false` means the command rejected and the
        // document was NOT changed.
        if !state.apply(cmd.clone()) {
            apply_rejected = true;
            return false;
        }
        let scope = baseline.map(|baseline| baseline.scope_after(state));
        if let Some(scope) = &scope {
            run_write_repairs_after_apply(state, scope);
        }
        if let Err(e) = save_editor_state(state, path) {
            applier_failed = Some(format!("save failed: {e}"));
            return false;
        }
        // A verbatim write does not arm auto-finalize: finalizing would run
        // the same repairs over it at session end.
        if let (Some(auto_finalize), Some(scope)) = (auto_finalize.as_deref_mut(), &scope) {
            auto_finalize.note_write(Instant::now(), scope);
        }
        true
    })?;
    if let Some(ref msg) = applier_failed {
        eprintln!("design-editor mcp: {msg}");
    }
    if explicit_finalize
        && !apply_rejected
        && applier_failed.is_none()
        && response.as_deref().is_some_and(mcp_response_succeeded)
    {
        if let Some(auto_finalize) = auto_finalize {
            auto_finalize.note_finalize(state.document_revision());
        }
    }
    Ok(response)
}

fn mcp_response_succeeded(response: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(response)
        .ok()
        .and_then(|value| value.get("result").cloned())
        .and_then(|result| result.get("isError").cloned())
        .and_then(|is_error| is_error.as_bool())
        != Some(true)
}

/// One MCP write as the host should apply it.
pub struct McpWrite<'a> {
    /// The command to apply: the tool's own, or its root-placed form (#37).
    pub command: &'a EditorCommand,
    /// The tool's own command when `command` is the placed form. Placement
    /// names the new roots by the ids the insert will mint on the state it
    /// was computed against, so a host whose document may have moved on
    /// since (the live UI thread) applies this instead when the revision
    /// differs: an unplaced insert, never a move of somebody else's node.
    pub unplaced: Option<&'a EditorCommand>,
    /// `document_revision()` of the state placement was computed against.
    pub revision: u64,
}

/// [`process_message_with_write_applier`] for hosts that apply against the
/// same state the call was dispatched on, so the placed command is always
/// the right one.
pub fn process_message_with_applier<F>(
    state: &mut EditorState,
    line: &str,
    mut apply: F,
) -> Result<Option<String>, McpServeError>
where
    F: FnMut(&str, &mut EditorState, &EditorCommand) -> bool,
{
    process_message_with_write_applier(state, line, |tool_name, state, write| {
        apply(tool_name, state, write.command)
    })
}

pub fn process_message_with_write_applier<F>(
    state: &mut EditorState,
    line: &str,
    mut apply: F,
) -> Result<Option<String>, McpServeError>
where
    F: FnMut(&str, &mut EditorState, &McpWrite) -> bool,
{
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    // MCP handshake / discovery methods short-circuit the tool dispatcher.
    match sniff_method(trimmed).as_deref() {
        Some("initialize") => {
            return Ok(sniff_id_raw(trimmed).map(|id| initialize_response(&id)));
        }
        Some("tools/list") => {
            return Ok(sniff_id_raw(trimmed)
                .map(|id| tools_list_response(&id, state, debug_tools_enabled())));
        }
        Some("notifications/initialized") | Some("initialized") => {
            return Ok(None); // notification — no response required
        }
        Some("ping") => {
            return Ok(sniff_id_raw(trimmed)
                .map(|id| ping_response(&id, headless_token_from_env().as_deref())));
        }
        _ => {}
    }
    // Fall through: tools/call or legacy direct dispatch. The
    // registry snapshots `state` at build time, so it no longer
    // borrows it once the applier closure mutates it.
    let call = de_mcp::parse_tool_call(trimmed);
    let requested_tool = call.as_ref().map(|call| call.tool.clone());
    let registry = rebuild_registry(state, requested_tool.as_deref());
    let response = process_tool_message_with_registry(&registry, line, |tool_name, cmd| {
        // New page roots land in free canvas space, in front (#37). Placing
        // here, before the host applier runs, gives every transport the
        // same command to apply.
        let revision = state.document_revision();
        let placed = call.as_ref().and_then(|call| {
            de_mcp::root_placement::place_new_roots(state, tool_name, &call.arguments, cmd)
        });
        let write = McpWrite {
            command: placed.as_ref().unwrap_or(cmd),
            unplaced: placed.is_some().then_some(cmd),
            revision,
        };
        apply(tool_name, state, &write)
    })?;
    if requested_tool.as_deref() == Some("save_document") {
        preview_hook::after_save_document(response.as_deref());
    }
    Ok(response)
}

/// Dispatch one already-classified tool call through a caller-provided
/// registry. Live hosts use this seam for tools whose complete snapshot is
/// much smaller than an [`EditorState`] (for example `list_pages`) or that do
/// not need state at all (`set_active_page`). The parser, command-application
/// contract, and wire serializer remain the same as the general path above.
///
/// Reports [`McpServeError::Dispatch`] — the `dispatch: ` prefix that used
/// to be baked into a `String` here is now part of that variant's `Display`
/// input, so the message is unchanged.
pub(crate) fn process_tool_message_with_registry<F>(
    registry: &ToolRegistry,
    line: &str,
    mut apply: F,
) -> Result<Option<String>, McpServeError>
where
    F: FnMut(&str, &EditorCommand) -> bool,
{
    let mut out: Vec<u8> = Vec::new();
    {
        let mut input = std::io::Cursor::new(line.as_bytes());
        run_stdio_with_applier(registry, &mut input, &mut out, |tool_name, cmd| {
            apply(tool_name, cmd)
        })
        .map_err(|e| McpServeError::Dispatch(format!("dispatch: {e}")))?;
    }
    let resp = String::from_utf8_lossy(&out).trim().to_string();
    Ok((!resp.is_empty()).then_some(resp))
}

pub fn run(path: PathBuf) -> Result<(), McpServeError> {
    auto_finalize::run(path)
}

pub fn run_http(path: PathBuf, port: u16) -> Result<(), McpServeError> {
    auto_finalize::run_http(path, port)
}

#[cfg(test)]
fn serve_http_connection<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    state: &mut EditorState,
    path: &Path,
) -> Result<bool, McpServeError> {
    auto_finalize::serve_http_connection(stream, state, path, None)
}

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: String,
    pub host: Option<String>,
    pub origin: Option<String>,
    /// Legacy `X-Design-Editor-Token` header value, when present. The local and
    /// managed web-canvas daemons deliberately ignore it; managed lifecycle
    /// authentication, when used, stays in the `design-editor/shutdown` body.
    pub token: Option<String>,
    /// `Content-Type` header value, when present. Browser-facing JSON routes
    /// require `application/json` so cross-origin "simple requests" (which
    /// skip the CORS preflight) cannot reach them.
    pub content_type: Option<String>,
    /// The raw query string (no leading `?`), when the target had one.
    ///
    /// `path` keeps its query stripped so exact-path routing is unaffected;
    /// this is captured alongside so a route that accepts no parameters
    /// (e.g. the live design.md route) can refuse one explicitly.
    pub query: Option<String>,
}

/// Parse a capped HTTP header and then read exactly its declared body length.
/// Query strings are stripped from the path before routing.
pub fn read_http_request<S: std::io::Read>(stream: &mut S) -> Result<HttpRequest, McpServeError> {
    const MAX_HEADER: usize = 64 * 1024;
    // Whole-document sync can carry embedded images, so retain the 64 MiB
    // ceiling while reading incrementally to avoid allocating from a claimed
    // Content-Length alone.
    const MAX_BODY: usize = 64 * 1024 * 1024;
    let mut head: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream
            .read(&mut byte)
            .map_err(|e| McpServeError::Io(format!("http read: {e}")))?;
        if n == 0 {
            return Err(McpServeError::Protocol(
                "connection closed before headers completed".into(),
            ));
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        if head.len() > MAX_HEADER {
            return Err(McpServeError::Protocol(
                "request headers exceed 64 KiB".into(),
            ));
        }
    }
    let headers = String::from_utf8_lossy(&head);
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| McpServeError::Protocol("request line missing".into()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| McpServeError::Protocol("request method missing".into()))?
        .to_ascii_uppercase();
    // Strip any `?query` from the request target so exact-path routing
    // (`/api/mcp/document`, `/mcp`, …) isn't defeated by `/api/mcp/document?x=1`.
    let target = request_parts
        .next()
        .ok_or_else(|| McpServeError::Protocol("request path missing".into()))?;
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (
            path.to_string(),
            (!query.is_empty()).then(|| query.to_string()),
        ),
        None => (target.to_string(), None),
    };
    // Parse `Content-Length` via `split_once(':')` — byte-slicing a `&str`
    // (e.g. `l[..15]`) would panic if a crafted header puts a multibyte UTF-8
    // boundary mid-slice; that panic would also bypass the live server's
    // connection-count decrement. A malformed length falls back to 0 (empty
    // body) rather than erroring.
    let content_length_values: Vec<&str> = headers
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.trim().split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .collect();
    let declared_length = content_length_values
        .iter()
        .find_map(|value| value.parse::<usize>().ok());
    let content_length = declared_length.unwrap_or(0);
    // Browser-extension snapshot ingress is the large body-carrying scoped
    // route, so it caps its body far below the endpoint-wide `MAX_BODY` — and
    // does it here, before a single body byte is read, so an untokened caller
    // cannot make this process buffer 64 MiB. The separate design-evidence
    // route receives its own smaller cap below. See
    // `mcp_live::snapshot_ingest::MAX_SNAPSHOT_BODY`.
    if method == "POST" && path == crate::mcp_live::snapshot_ingest::SNAPSHOT_INGEST_PATH {
        let limit = crate::mcp_live::snapshot_ingest::MAX_SNAPSHOT_BODY;
        match declared_length {
            // The extension always sends `Content-Length` (it POSTs a
            // string body through `fetch`), so a missing one is not a
            // client this route has to serve — and serving it would mean
            // reading an unbounded body to find out how big it is.
            None => {
                return Err(McpServeError::Framing {
                    status: "411 Length Required",
                    message: "web snapshot ingress requires a Content-Length header".into(),
                })
            }
            Some(declared) if declared > limit => {
                return Err(McpServeError::Framing {
                    status: "413 Payload Too Large",
                    message: format!("web snapshot body exceeds {} MiB", limit / (1024 * 1024)),
                })
            }
            Some(_) => {}
        }
    }
    // Intelligent design extraction is extension-reachable and must never
    // inherit the endpoint-wide 64 MiB body budget. Require one canonical
    // decimal Content-Length and reject above 256 KiB before reading bytes.
    if crate::mcp_live::design_md_route::is_design_md_path(&path) {
        let limit = crate::design_md_evidence::MAX_DESIGN_MD_EVIDENCE_BYTES;
        if content_length > limit {
            return Err(McpServeError::Framing {
                status: "413 Payload Too Large",
                message: "design.md evidence body exceeds 256 KiB".into(),
            });
        }
        let starts_job =
            method == "POST" && path == crate::mcp_live::design_md_route::DESIGN_MD_PATH;
        if !starts_job && content_length > 0 {
            return Err(McpServeError::Framing {
                status: "400 Bad Request",
                message: "non-POST design.md requests must not carry a body".into(),
            });
        }
    }
    if method == "POST" && path == crate::mcp_live::design_md_route::DESIGN_MD_PATH {
        let valid_single_length = content_length_values.len() == 1
            && !content_length_values[0].is_empty()
            && content_length_values[0]
                .bytes()
                .all(|byte| byte.is_ascii_digit())
            && declared_length.is_some();
        if !valid_single_length {
            return Err(McpServeError::Framing {
                status: if content_length_values.is_empty() {
                    "411 Length Required"
                } else {
                    "400 Bad Request"
                },
                message: "design.md evidence requires one valid Content-Length header".into(),
            });
        }
    }
    let credential_body_label = match path.as_str() {
        "/api/settings/credentials" => Some("credential settings"),
        "/api/ai/models/discover" => Some("model discovery"),
        _ => None,
    };
    if let Some(label) = credential_body_label
        .filter(|_| content_length > crate::web_credentials::MAX_CREDENTIAL_BODY_BYTES)
    {
        return Err(McpServeError::Protocol(format!(
            "{label} body exceeds 256 KiB"
        )));
    }
    let header_value = |wanted: &str| {
        headers.lines().skip(1).find_map(|line| {
            let (name, value) = line.trim().split_once(':')?;
            name.eq_ignore_ascii_case(wanted)
                .then(|| value.trim().to_string())
        })
    };
    let host = header_value("host");
    let origin = header_value("origin");
    let token = header_value("x-design-editor-token");
    let content_type = header_value("content-type");
    if method == "POST" && path == crate::mcp_live::design_md_route::DESIGN_MD_PATH {
        let content_type_count = headers
            .lines()
            .skip(1)
            .filter(|line| {
                line.trim()
                    .split_once(':')
                    .is_some_and(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            })
            .count();
        if content_type_count > 1 {
            return Err(McpServeError::Framing {
                status: "400 Bad Request",
                message: "design.md evidence accepts only one Content-Type header".into(),
            });
        }
    }
    if content_length > MAX_BODY {
        return Err(McpServeError::Protocol(format!(
            "request body exceeds {} MiB",
            MAX_BODY / (1024 * 1024)
        )));
    }
    // Grow the body from bytes actually received, not the declared length;
    // socket timeouts bound a stalled peer.
    let mut body = Vec::with_capacity(content_length.min(64 * 1024));
    let mut remaining = content_length;
    let mut chunk = [0u8; 64 * 1024];
    while remaining > 0 {
        let want = remaining.min(chunk.len());
        let n = stream
            .read(&mut chunk[..want])
            .map_err(|e| McpServeError::Io(format!("http body read: {e}")))?;
        if n == 0 {
            return Err(McpServeError::Protocol(
                "connection closed before body completed".into(),
            ));
        }
        body.extend_from_slice(&chunk[..n]);
        remaining -= n;
    }
    let body = if method == "POST" && path == crate::mcp_live::design_md_route::DESIGN_MD_PATH {
        String::from_utf8(body).map_err(|_| McpServeError::Framing {
            status: "400 Bad Request",
            message: "design.md evidence body must be valid UTF-8".into(),
        })?
    } else {
        String::from_utf8_lossy(&body).into_owned()
    };
    Ok(HttpRequest {
        method,
        path,
        body,
        host,
        origin,
        token,
        content_type,
        query,
    })
}

/// Compatibility wrapper for older tests/callers that only care about
/// the JSON-RPC body.
#[cfg(test)]
pub fn read_http_request_body<S: std::io::Read>(stream: &mut S) -> Result<String, McpServeError> {
    read_http_request(stream).map(|req| req.body)
}

/// Permissive-CORS (`*`) response writer — the convenience wrapper over
/// [`write_mcp_http_response_with_origin`] for every caller that does not
/// compute a per-request `Access-Control-Allow-Origin`.
pub fn write_mcp_http_response<S: std::io::Write>(
    stream: &mut S,
    status: &str,
    body: &str,
) -> Result<(), McpServeError> {
    write_mcp_http_response_with_origin(stream, status, body, Some("*"))
}

/// Like [`write_mcp_http_response`], but lets the caller supply the exact
/// `Access-Control-Allow-Origin` value to emit: `Some(origin)` echoes that
/// literal value, `None` omits the header entirely. Used by the managed
/// web-canvas daemon (`web_canvas_server::serve_one`), which computes the
/// right value per-request via `cors_origin_for` against its
/// `--allow-origin` allowlist; every other caller (the `--mcp-http`
/// server, the legacy `--serve-web` accept-loop overflow reply) keeps the
/// permissive `*` via `write_mcp_http_response` above, unchanged.
pub(crate) fn write_mcp_http_response_with_origin<S: std::io::Write>(
    stream: &mut S,
    status: &str,
    body: &str,
    cors_origin: Option<&str>,
) -> Result<(), McpServeError> {
    let cors_line = cors_origin
        .map(|origin| format!("Access-Control-Allow-Origin: {origin}\r\n"))
        .unwrap_or_default();
    let http = format!(
        "HTTP/1.1 {status}\r\n\
         {cors_line}\
         Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, mcp-session-id, X-Design-Editor-Token, Authorization\r\n\
         Access-Control-Expose-Headers: mcp-session-id\r\n\
         mcp-session-id: design-editor\r\n\
         Cache-Control: no-store\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(http.as_bytes())
        .map_err(|e| McpServeError::Io(format!("http write: {e}")))?;
    stream
        .flush()
        .map_err(|e| McpServeError::Io(format!("http flush: {e}")))
}

mod registry;
use registry::rebuild_registry;

mod wire;
pub use wire::*;

mod doc_sync;
pub use doc_sync::*;

pub(crate) mod schemas;
mod tools_list;
#[cfg(not(feature = "mcp-debug-tools"))]
pub use schemas::TOOL_SCHEMAS;
#[cfg(feature = "mcp-debug-tools")]
pub use schemas::{DEBUG_TOOL_SCHEMAS, TOOL_SCHEMAS};
use tools_list::tools_list_response;

mod capture_time;
pub(crate) mod screenshot_tool;
use screenshot_tool::get_screenshot_snapshot;
pub(crate) mod capture_preview_tool;
use capture_preview_tool::capture_preview_snapshot;

pub(crate) mod export_tool;
use export_tool::export_nodes_snapshot;
pub(crate) mod execute_tool;
use execute_tool::execute_snapshot;

pub(crate) mod export_item_tool;
use export_item_tool::export_item_snapshot;
pub(crate) mod export_deck_tool;
use export_deck_tool::export_deck_snapshot;
pub(crate) mod export_frames_tool;
use export_frames_tool::{export_frames_snapshot, get_deck_boards_snapshot};
pub(crate) mod scene_template_tools;
use scene_template_tools::{list_scene_templates_snapshot, use_scene_template_snapshot};

pub(crate) mod finalize_tool;
use finalize_tool::finalize_design_snapshot;
pub(crate) mod design_quality_tool;
use design_quality_tool::get_design_quality_snapshot;
pub(crate) mod boolean_op_tool;
use boolean_op_tool::boolean_op_snapshot;
pub(crate) mod font_tools;
use font_tools::load_fonts_snapshot;
pub(crate) mod enrich_images_generate;
pub(crate) mod enrich_images_tool;
use enrich_images_tool::enrich_images_snapshot;
pub(crate) mod generate_tool;
use generate_tool::{generate_snapshot, generation_status_snapshot};
pub(crate) mod design_agent_run_error;
pub(crate) mod design_agent_run_tool;
use design_agent_run_tool::run_design_agent_snapshot;

#[cfg(test)]
#[path = "mcp_serve/auto_finalize_stdio_tests.rs"]
mod auto_finalize_stdio_tests;
#[cfg(test)]
mod codegen_wire_tests;
#[cfg(test)]
mod conversion_flow_tests;
#[cfg(test)]
mod design_agent_run_tool_tests;
#[cfg(test)]
mod enrich_images_tool_tests;
#[cfg(test)]
mod finalize_tool_advisory_tests;
#[cfg(test)]
mod finalize_tool_tests;
#[cfg(test)]
mod generate_tool_tests;
#[cfg(test)]
mod import_pen_dev_verbatim_tests;
#[cfg(test)]
mod kit_insert_tools_tests;
#[cfg(test)]
mod root_placement_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_tests;
