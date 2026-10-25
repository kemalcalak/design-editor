//! Headless web-canvas daemon — serves the document to the Rust WASM web shell
//! (`de-host-web`, which runs in a browser and can't bind a socket) and to
//! external MCP/CLI clients. It is the Rust analog of the TS web app's
//! `apps/web/server/api/mcp/*` Nitro routes + `setSyncDocument`: it owns the
//! canonical document in memory and answers the same whole-document REST sync
//! shape, so a JSON-RPC/REST client (e.g. the `de` CLI or any MCP client) can
//! drive the Rust *web* canvas the same way it drives the desktop canvas.
//!
//! This module ships the request-handling CORE (`handle_web_canvas_request`,
//! fully unit-testable without a socket) plus a runnable loop
//! (`run_web_canvas`, behind
//! `design-editor --serve-web <port> [doc] [--host <addr>]`).
//! Layered on top: an SSE endpoint that streams `version` bumps to connected
//! shells, static serving of the host page + WASM bundle (`crate::web_static`
//! — `GET /` and `GET /pkg/*`), and a token-authenticated `design-editor/shutdown`
//! (same contract as `--mcp-http`) so `de stop` works against this daemon.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use de_editor_core::agent_settings::{AcpAgentConnectOutcome, ProviderConnectOutcome};
use de_editor_core::{AgentSettings, EditorState};

use crate::web_canvas_server_error::WebCanvasError;
use crate::web_credential_policy::WebCredentialPersistence;

/// Every fallible step of this module fails with [`WebCanvasError`].
type Result<T> = std::result::Result<T, WebCanvasError>;

/// Slow/stalled-peer bound — bodies can be large (whole documents with embedded
/// images), so a connection that opens and dribbles must not pin a thread.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// Max concurrent connections (SSE streams are long-lived, so bound them).
const MAX_CONNS: usize = 64;

/// SSE keep-alive cadence — also how quickly a disconnected SSE client is
/// detected (the heartbeat write fails once the socket is gone).
const SSE_HEARTBEAT: Duration = Duration::from_secs(15);

/// One SSE payload: the sequence number a client polls on.
///
/// `version` changes only when document *content* changes, so it is what makes
/// a client refetch the document. Serialized as `{"version":N}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SseTick {
    pub version: u64,
}

/// RAII decrement for the connection counter — `Drop` runs on normal exit AND
/// panic unwind, so a panicking connection can't leak its `MAX_CONNS` slot.
struct ConnGuard(Arc<AtomicUsize>);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// In-memory document authority for the web-canvas daemon — the Rust mirror of
/// the TS `mcp-sync-state` (document + monotonic version). The browser shell
/// mirrors this (over the SSE endpoint, layered on later); external MCP clients
/// read/replace it over `/api/mcp/document`.
pub struct WebCanvasState {
    pub(crate) editor: EditorState,
    /// Immutable deployment policy for browser-supplied credentials. Public
    /// demo deployments fail closed to browser-only persistence.
    pub(crate) credential_persistence: WebCredentialPersistence,
    /// Local file backing this daemon document, when `--serve-web` was launched
    /// with a path or the user opened a recent local file through the daemon.
    pub(crate) current_path: Option<PathBuf>,
    /// Monotonic sync version, bumped on every document mutation — the key the
    /// browser shell uses to detect that the live document changed.
    pub(crate) version: u64,
    /// The bound port, reported by `GET /api/mcp/server` (TS `server.get.ts`
    /// parity).
    pub(crate) port: u16,
    /// Managed-mode per-instance lifecycle token (see
    /// `ServeWebOptions::managed` / `run_web_canvas`). `None` outside managed
    /// mode. Ordinary requests are tokenless; `serve_one` consults this only
    /// for the optional body-authenticated graceful-shutdown compatibility
    /// path.
    pub(crate) managed_token: Option<String>,
    /// Managed-mode `--allow-origin` allowlist. Empty outside managed mode.
    /// Read by `serve_one` (via `cors_origin_for`) to decide which `Origin`
    /// is echoed back in `Access-Control-Allow-Origin`.
    pub(crate) allow_origins: Vec<String>,
    /// Idempotence guard for `POST /api/mcp/sync-reset`: set once the FIRST
    /// successful reset in this process's lifetime completes. A failed reset
    /// does not set it (retryable); once set, further sync-reset calls are a
    /// no-op `{"ok":true,"skipped":true,"version":<current>}` reply instead
    /// of touching the document again.
    pub(crate) reset_consumed: bool,
    /// How this daemon is deployed (`Local` or `Managed`); reported to the
    /// browser as `serveMode`. See `serve_mode.rs`.
    pub(crate) mode: ServeMode,
}

impl WebCanvasState {
    #[cfg(test)]
    pub(crate) fn new(editor: EditorState, port: u16) -> Self {
        Self::new_with_path(editor, port, None)
    }

    #[cfg(test)]
    pub(crate) fn new_with_policy(
        editor: EditorState,
        port: u16,
        credential_persistence: WebCredentialPersistence,
    ) -> Self {
        Self::new_with_path_and_policy(editor, port, None, credential_persistence)
    }

    #[cfg(test)]
    pub(crate) fn new_with_path(
        editor: EditorState,
        port: u16,
        current_path: Option<PathBuf>,
    ) -> Self {
        Self::new_with_path_and_policy(
            editor,
            port,
            current_path,
            WebCredentialPersistence::BrowserOnly,
        )
    }

    fn new_with_path_and_policy(
        mut editor: EditorState,
        port: u16,
        current_path: Option<PathBuf>,
        credential_persistence: WebCredentialPersistence,
    ) -> Self {
        if !credential_persistence.server_persistence() {
            let _ = crate::web_credentials::remove_browser_owned_credentials(&mut editor);
        }
        Self {
            editor,
            credential_persistence,
            current_path,
            version: 0,
            port,
            managed_token: None,
            allow_origins: Vec::new(),
            reset_consumed: false,
            mode: ServeMode::Local,
        }
    }

    /// Replace the whole document (an already-loaded `POST /api/mcp/document`
    /// body), bump and return the new version.
    pub(crate) fn replace_document(&mut self, doc: jian_ops_schema::PenDocument) -> u64 {
        self.editor.replace_document(doc);
        self.version += 1;
        self.version
    }

    /// Clear the transient web-sync document back to the same starter document a
    /// fresh browser shell paints before the daemon applies updates.
    pub(crate) fn reset_document(&mut self) -> Result<u64> {
        if let Some(path) = self.current_path.clone() {
            let mut next = crate::mcp_serve::load_editor_state(&path)?;
            preserve_web_canvas_preferences(&self.editor, &mut next);
            set_file_name_display(&mut next, &path);
            self.editor = next;
        } else {
            self.editor.replace_document(EditorState::starter().doc);
        }
        self.version += 1;
        Ok(self.version)
    }

    /// Idempotent wrapper around [`Self::reset_document`] for
    /// `POST /api/mcp/sync-reset`: the first SUCCESSFUL reset in this
    /// process's lifetime consumes `reset_consumed`; every call after that
    /// is a no-op (`skipped: true`) that neither reloads nor bumps the
    /// version. A failed reset does NOT consume the guard — it stays
    /// retryable, and the document/version are left exactly as
    /// `reset_document`'s own `?`-early-return already guarantees (nothing
    /// touched before the fallible load succeeds).
    pub(crate) fn reset_document_guarded(&mut self) -> Result<ResetOutcome> {
        if self.reset_consumed {
            return Ok(ResetOutcome { skipped: true });
        }
        self.reset_document()?;
        self.reset_consumed = true;
        Ok(ResetOutcome { skipped: false })
    }

    /// Push a whole-document replacement (`POST /api/mcp/document` body).
    /// Consolidates the route's former inline parse-then-replace logic into
    /// one testable entry point. `base_version_override` is test-only — the
    /// real route always passes `None`, in which case an optional top-level
    /// `baseVersion` field inside `body` itself is consulted (extracted via
    /// serde, not a hand-rolled scan). When a base version is present and
    /// does not match the current version, the write is rejected WITHOUT
    /// mutating anything: that's a version verdict conveyed through
    /// `PushOutcome::applied == false`, not an `Err`. `Err` is reserved for
    /// the existing malformed-JSON / schema-validation failures, which stay
    /// client-fault → HTTP 400 at the route exactly as before.
    pub(crate) fn apply_document_push(
        &mut self,
        body: &str,
        base_version_override: Option<u64>,
    ) -> Result<PushOutcome> {
        let push = PendingDocumentPush::parse(body)?;
        self.apply_prepared_document_push(push, base_version_override)
    }

    /// Apply an already-parsed push. Runs entirely under the state lock.
    ///
    /// Everything fallible and expensive — JSON parse, schema load, structural
    /// validation — happened in [`PendingDocumentPush::parse`] before the lock
    /// was taken. What remains is the part that genuinely needs exclusivity:
    /// re-checking `baseVersion` against the live counter, and installing.
    ///
    /// The `baseVersion` check MUST be here rather than at parse time: the
    /// version can move between parse and install, and checking it outside the
    /// lock would be a race that silently clobbers a concurrent write.
    pub(crate) fn apply_prepared_document_push(
        &mut self,
        push: PendingDocumentPush,
        base_version_override: Option<u64>,
    ) -> Result<PushOutcome> {
        let base_version = base_version_override.or(push.base_version);
        if let Some(expected) = base_version {
            if expected != self.version {
                return Ok(PushOutcome {
                    applied: false,
                    current_version: self.version,
                });
            }
        }
        let mut push = push;
        // Taken rather than moved: taking `prepared` is what tells the `Drop`
        // this push reached the install.
        let editor_meta = push.take_editor_meta();
        let Some(prepared) = push.prepared.take() else {
            // Active-page changes do not mutate canonical document content.
            // Apply the scalar pair without replacing the identical document,
            // bumping its generation, or publishing a version that another
            // browser would install as a full undoable remote replacement.
            de_pen_loader::apply_editor_meta(&mut self.editor, editor_meta);
            return Ok(PushOutcome {
                applied: true,
                current_version: self.version,
            });
        };
        let version = self.replace_document(prepared.into_document());
        de_pen_loader::apply_editor_meta(&mut self.editor, editor_meta);
        Ok(PushOutcome {
            applied: true,
            current_version: version,
        })
    }

    /// The sequence number a client polls on.
    pub(crate) const fn sse_tick(&self) -> SseTick {
        SseTick {
            version: self.version,
        }
    }

    #[cfg(test)]
    pub(crate) fn document_version_for_test(&self) -> u64 {
        self.version
    }
}

/// Outcome of [`WebCanvasState::reset_document_guarded`].
pub(crate) struct ResetOutcome {
    pub skipped: bool,
}

/// Outcome of [`WebCanvasState::apply_document_push`] — expresses only the
/// version verdict (parse/schema failures are `Err`, handled separately).
pub(crate) struct PushOutcome {
    pub applied: bool,
    pub current_version: u64,
}

/// A handled reply: HTTP status line + JSON body, ready for
/// `write_mcp_http_response`.
pub struct WebReply {
    pub(crate) status: &'static str,
    pub(crate) body: String,
}

fn persist_api_settings<F>(
    method: &str,
    path: &str,
    state: &mut WebCanvasState,
    settings_before: crate::settings_io::Fingerprint,
    credential_settings_before: Option<AgentSettings>,
    reply: WebReply,
    save_credentials: F,
) -> WebReply
where
    // `settings_io::save_checked` reports its own typed
    // `SettingsIoError` now; only its success/failure is consulted here, but
    // naming the type keeps the bound honest instead of erasing it to a
    // `String` the caller would have to re-render.
    F: FnOnce(&EditorState) -> std::result::Result<(), crate::settings_io::SettingsIoError>,
{
    if method == "POST" && path == "/api/settings/credentials" && reply.status == "200 OK" {
        if save_credentials(&state.editor).is_err() {
            if let Some(settings) = credential_settings_before {
                state.editor.editor_ui.agent_settings = settings;
                state.editor.rebuild_chat_models();
            }
            return WebReply {
                status: "500 Internal Server Error",
                body: crate::mcp_serve::rest_error_body("failed to persist server credentials"),
            };
        }
    } else {
        crate::settings_io::save_if_changed(&state.editor, settings_before);
    }
    reply
}

/// Handle one parsed web-canvas REST request against the in-memory state. Pure
/// w.r.t. IO — fully unit-testable without a socket. Mirrors the TS Nitro
/// routes:
/// - `GET  /api/mcp/server`   → health `{ok:true,…}` (like `server.get.ts`)
/// - `GET  /api/mcp/document` → `{document:<doc>,version}` (like `document.get.ts`)
/// - `POST /api/mcp/document` → whole-doc replace → `{ok:true,version}` (like
///   `document.post.ts`); an optional top-level `baseVersion` makes the write
///   conditional — a stale value 409s with `{ok:false,error:"version-conflict",
///   version}` and leaves the document untouched instead of clobbering it
/// - `GET  /api/mcp/version`  → `{version}` — Rust-only cheap change probe; the
///   TS stack pushes documents over SSE instead, so it never needs one. The
///   browser shell polls this and fetches the full document only on a bump.
/// - `GET  /api/mcp/selection` → `{selectedIds,activePageId}` (like `selection.get.ts`)
/// - `POST /api/mcp/selection` → renderer selection push (like `selection.post.ts`)
/// - `POST /api/file/save` → write the browser document to this daemon's
///   backing local path, including embedded active-page metadata
///   (and legacy `.opmeta` fallback compatibility)
/// - `POST /api/file/open-recent` → local-daemon recent-file open, used by the
///   browser shell because only the daemon can read local paths.
/// - anything else → 404 (the JSON-RPC `/mcp` path + SSE are handled by the
///   caller's connection loop, not here).
pub fn handle_web_canvas_request(
    method: &str,
    path: &str,
    body: &str,
    state: &mut WebCanvasState,
) -> WebReply {
    match (method, path) {
        ("GET", "/api/mcp/server") => WebReply {
            status: "200 OK",
            // `{running,port,localIp}` matches TS `server.get.ts`; the daemon
            // binds 127.0.0.1 (localhost-only) so localIp is loopback. Extra
            // `server`/`mode` fields are additive diagnostics.
            // `serveMode` is an additive diagnostic naming how the daemon
            // was launched (`local` / `managed`).
            body: format!(
                r#"{{"running":true,"port":{},"localIp":"127.0.0.1","server":"design-editor-mcp","mode":"web-canvas","serveMode":"{}"}}"#,
                state.port,
                state.mode.wire_name()
            ),
        },
        ("POST", "/api/mcp/server") => update_mcp_server_settings(body, state),
        ("GET", "/api/mcp/document") => match serde_json::to_string(&state.editor.doc) {
            Ok(doc_json) => WebReply {
                status: "200 OK",
                body: format!(
                    r#"{{"document":{doc_json},"version":{},"activePageIndex":{},"preserveAuthoredGeometry":{},"scenario":{}}}"#,
                    state.version,
                    state.editor.ui.active_page_index,
                    state.editor.editor_ui.preserve_authored_geometry,
                    // The scene tag rides the same wire as the rest of the
                    // editor meta: a Slides deck must reach the browser as a
                    // deck or web preview cannot enter its presentation.
                    match state.editor.editor_ui.scenario {
                        Some(scene) => format!("\"{}\"", scene.as_str()),
                        None => "null".to_string(),
                    }
                ),
            },
            Err(e) => WebReply {
                status: "500 Internal Server Error",
                body: crate::mcp_serve::rest_error_body(&e.to_string()),
            },
        },
        ("POST", "/api/mcp/document") => match state.apply_document_push(body, None) {
            Ok(outcome) if outcome.applied => WebReply {
                status: "200 OK",
                body: crate::mcp_serve::document_sync_ok(outcome.current_version),
            },
            Ok(outcome) => WebReply {
                // Stale baseVersion: reject without writing, TS-style error
                // envelope plus the current version so the caller can decide
                // whether to refetch and retry.
                status: "409 Conflict",
                body: serde_json::json!({
                    "ok": false,
                    "error": "version-conflict",
                    "version": outcome.current_version,
                })
                .to_string(),
            },
            Err(error) => error_reply(&error),
        },
        ("GET", "/api/mcp/version") => WebReply {
            status: "200 OK",
            body: format!(r#"{{"version":{}}}"#, state.version),
        },
        ("GET", "/api/mcp/indicators") => WebReply {
            // Agent-indicator relay: design runs execute inside this
            // daemon, so the process-global registry the canvas paints
            // from lives HERE — the browser polls this and mirrors it
            // into its own registry (agent_indicators::apply_remote) so
            // agent borders / badges / reveal animations show on web.
            status: "200 OK",
            body: de_editor_core::agent_indicators::relay_json(),
        },
        // The wasm shell posts a sync-reset on every mount: "the browser
        // just booted, drop the transient document".
        ("POST", "/api/mcp/sync-reset") => match state.reset_document_guarded() {
            Ok(outcome) if outcome.skipped => WebReply {
                status: "200 OK",
                body: format!(
                    r#"{{"ok":true,"skipped":true,"version":{}}}"#,
                    state.version
                ),
            },
            Ok(_) => WebReply {
                status: "200 OK",
                body: crate::mcp_serve::document_sync_ok(state.version),
            },
            Err(e) => WebReply {
                status: e.http_status(),
                body: crate::mcp_serve::rest_error_body(&format!("sync reset failed: {e}")),
            },
        },
        ("GET", "/api/mcp/selection") => {
            // TS `selection.get.ts` → `getSyncSelection()` shape:
            // `{selectedIds, activePageId}`. Read straight off the live
            // editor selection so MCP clients and the REST route agree.
            let ids: Vec<&str> = state
                .editor
                .selection
                .set
                .iter()
                .map(|id| id.as_str())
                .collect();
            let active_page_id = state
                .editor
                .doc
                .pages
                .as_ref()
                .and_then(|pages| pages.get(state.editor.ui.active_page_index))
                .map(|page| page.id.clone());
            let body = serde_json::json!({
                "selectedIds": ids,
                "activePageId": active_page_id,
            });
            WebReply {
                status: "200 OK",
                body: serde_json::to_string(&body)
                    .unwrap_or_else(|_| r#"{"selectedIds":[],"activePageId":null}"#.to_string()),
            }
        }
        ("POST", "/api/mcp/selection") => apply_selection_sync(body, state),
        ("POST", "/api/file/save") => save_current_file(body, state),
        ("POST", "/api/file/open-recent") => open_recent_file(body, state),
        ("POST", "/api/export/pdf") => export_pdf_download(body, state),
        ("POST", "/api/export/raster") => export_raster_download(body, state),
        ("GET", "/api/ai/models") => WebReply {
            // JSON array of model ids the AI proxy can serve (the
            // configured built-in agents). The web bundle queries this
            // to populate its model picker without bundling a static
            // list or holding API keys. `POST /api/ai/stream` is a
            // streaming route handled in the connection loop, not here.
            status: "200 OK",
            body: crate::ai_proxy::models_json(&state.editor),
        },
        ("GET", "/api/settings/credential-policy") => WebReply {
            status: "200 OK",
            body: serde_json::json!({
                "serverPersistence": state.credential_persistence.server_persistence(),
            })
            .to_string(),
        },
        ("POST", "/api/settings/credentials") => {
            if !state.credential_persistence.server_persistence() {
                WebReply {
                    status: "403 Forbidden",
                    body: crate::mcp_serve::rest_error_body(
                        "server credential persistence is disabled",
                    ),
                }
            } else {
                match crate::web_credentials::apply_json(&mut state.editor, body) {
                    Ok(()) => WebReply {
                        status: "200 OK",
                        body: r#"{"ok":true}"#.into(),
                    },
                    // The oversize verdict now rides on the error itself
                    // (`is_payload_too_large`) instead of this route
                    // re-measuring the body against the cap — same 413/400
                    // split, one owner for the threshold.
                    Err(error) => WebReply {
                        status: if error.is_payload_too_large() {
                            "413 Payload Too Large"
                        } else {
                            "400 Bad Request"
                        },
                        body: crate::mcp_serve::rest_error_body(&error.to_string()),
                    },
                }
            }
        }
        _ => not_found_reply(),
    }
}

/// The daemon's canonical unknown-route reply.
fn not_found_reply() -> WebReply {
    WebReply {
        status: "404 Not Found",
        body: r#"{"ok":false,"error":"Not found. Use /api/mcp/document, /api/mcp/sync-reset, /api/mcp/server, /api/file/save, /api/export/raster, /api/export/pdf, or /mcp."}"#
            .to_string(),
    }
}

mod document_push;
pub(crate) use document_push::{document_push_reply, PendingDocumentPush};
// Spine-local: the shared error renderer for the document routes.
use document_push::error_reply;
mod sse_hub;
pub use sse_hub::SseHub;
pub(crate) use sse_hub::SseSlot;
mod connect_routes;
mod connection;
mod connection_ai_routes;
mod doc_routes;
mod export_routes;
mod origin_guard;
mod run_loop;
mod script_routes;
pub mod serve_mode;
mod serve_options;

pub use connect_routes::*;
use connection::*;
use doc_routes::*;
use export_routes::*;
use origin_guard::*;
pub use run_loop::*;
pub use serve_mode::ServeMode;
pub use serve_options::*;

#[cfg(test)]
#[path = "web_canvas_server_tests.rs"]
mod tests;
