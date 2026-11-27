//! "Ask before destructive agent actions" for agents on the live MCP server
//! (#398): CLI and ACP agents drive the editor through this endpoint, so a
//! destructive `tools/call` waits for the same Allow / Deny prompt the
//! in-app loops use (`EditorState::gate_agent_tool_with`).
//!
//! The connection thread first reads whether the setting is on (published by
//! the pump on `LiveShared`, so with it off a call costs nothing more), then
//! classifies the call and, when it is destructive, asks the UI thread
//! BEFORE taking the state lock (a call waiting on the user never blocks
//! other clients). The prompt is pinned to the chat tab on screen when it
//! opens and to this call's own id. The connection waits at most
//! [`PERMISSION_DEADLINE`], under the clients' own tool timeouts: past it
//! the call is refused, its prompt closed, and a late Allow can no longer
//! run it (the call runs on the connection thread, which has returned). A
//! server stop drops the question, which reads as declined.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender, SyncSender};
use std::sync::Arc;
use std::time::Duration;

use de_editor_core::agent_permission::{DestructiveAction, ToolGate, WAITING_TOAST};
use de_editor_core::editor_toast::EditorToastLevel;
use de_editor_core::EditorState;
use de_mcp::{RequestId, ToolErrorCode, ToolResponse};

use super::{McpLiveServer, McpPumpOutcome, UiRequest, UiWake};

/// How long a connection waits for the user's answer: under the ~60 s
/// clients such as Codex give a tool call.
pub(super) const PERMISSION_DEADLINE: Duration = Duration::from_secs(50);

/// MCP question ids, apart from the in-app loops' request ids.
static NEXT_ID: AtomicU64 = AtomicU64::new(1 << 62);

/// One question waiting for the user.
pub(super) struct PendingPermission {
    pub(super) id: u64,
    pub(super) tool: String,
    pub(super) args: String,
    pub(super) action: DestructiveAction,
    pub(super) ack: SyncSender<bool>,
    /// Set by the connection when its wait ran out: the pump closes the
    /// prompt and drops the question.
    pub(super) expired: Arc<AtomicBool>,
    /// The chat tab (`ChatState::tab_id`) the prompt opened on, once it did.
    pub(super) tab: Option<u64>,
}

/// How a question ended for the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PermissionAnswer {
    Allowed,
    Declined,
    /// No answer within [`PERMISSION_DEADLINE`].
    TimedOut,
}

/// The destructive `tools/call` in `body`, if any: its id, tool, arguments
/// (as JSON) and action.
pub(super) fn destructive_call(
    body: &str,
) -> Option<(RequestId, String, String, DestructiveAction)> {
    let call = de_mcp::parse_tool_call(body.trim())?;
    let args = serde_json::to_string(&call.arguments).ok()?;
    let action = crate::agent_permission_classify::classify_agent_tool(&call.tool, &args)?;
    Some((call.id, call.tool, args, action))
}

/// Ask the UI thread whether the destructive call may run, waiting at most
/// `deadline`.
pub(super) fn request_permission(
    req_tx: &Sender<UiRequest>,
    wake_ui: &UiWake,
    tool: String,
    args: String,
    action: DestructiveAction,
    deadline: Duration,
) -> PermissionAnswer {
    let (ack_tx, ack_rx) = mpsc::sync_channel(1);
    let expired = Arc::new(AtomicBool::new(false));
    let request = UiRequest::Permission(PendingPermission {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        tool,
        args,
        action,
        ack: ack_tx,
        expired: Arc::clone(&expired),
        tab: None,
    });
    if req_tx.send(request).is_err() {
        return PermissionAnswer::Declined;
    }
    wake_ui();
    match ack_rx.recv_timeout(deadline) {
        Ok(true) => PermissionAnswer::Allowed,
        Ok(false) | Err(RecvTimeoutError::Disconnected) => PermissionAnswer::Declined,
        Err(RecvTimeoutError::Timeout) => {
            expired.store(true, Ordering::SeqCst);
            wake_ui();
            PermissionAnswer::TimedOut
        }
    }
}

/// The reply to a call that was not allowed: the tool-level error the
/// in-app loops give their agent, or that the user did not answer in time.
pub(super) fn refused_response(id: &RequestId, answer: PermissionAnswer) -> String {
    let message = match answer {
        PermissionAnswer::TimedOut => {
            r#"{"success":false,"error":"The user did not answer the permission prompt in time; the action was not run."}"#
        }
        _ => de_editor_core::agent_permission::DECLINED_RESULT,
    };
    de_mcp::json_serializer::tool_response_to_json(&ToolResponse::Err {
        id: id.clone(),
        code: ToolErrorCode::InvalidArgument,
        message: message.to_string(),
    })
}

impl McpLiveServer {
    /// Settle the questions waiting on the user: run, decline, drop an
    /// expired one, or keep waiting (a new prompt repaints, and raises the
    /// waiting notice when the chat is out of sight).
    pub(super) fn pump_permissions(
        &mut self,
        state: &mut EditorState,
        outcome: &mut McpPumpOutcome,
    ) {
        let now_ms = self.now_ms;
        self.pending_permissions.retain_mut(|pending| {
            if pending.expired.load(Ordering::SeqCst) {
                state.close_agent_permission(pending.id);
                outcome.repaint = true;
                return false;
            }
            // The tab it was asked on; the one on screen for a new question.
            let tab_index = match pending.tab {
                Some(tab_id) => {
                    let Some(index) = state.chat.tabs().iter().position(|t| t.tab_id == tab_id)
                    else {
                        // Its tab was closed, and the prompt with it.
                        let _ = pending.ack.send(false);
                        return false;
                    };
                    index
                }
                None => state.chat.active_index(),
            };
            let action = pending.action.clone();
            let gate = state.gate_agent_tool_with(
                Some(tab_index),
                pending.id,
                &pending.tool,
                &pending.args,
                || Some(action),
            );
            match gate {
                ToolGate::Run | ToolGate::Declined => {
                    let _ = pending.ack.send(gate == ToolGate::Run);
                    false
                }
                ToolGate::Asked => {
                    pending.tab = Some(state.chat.tabs()[tab_index].tab_id);
                    if state.permission_prompt_hidden(Some(tab_index)) {
                        state.editor_ui.show_toast(
                            WAITING_TOAST,
                            Vec::new(),
                            EditorToastLevel::Info,
                            now_ms,
                        );
                    }
                    outcome.repaint = true;
                    true
                }
                ToolGate::Wait => true,
            }
        });
        self.ui_side.publish_permission_setting(
            state
                .editor_ui
                .agent_settings
                .confirm_destructive_agent_actions,
        );
    }

    /// Whether an agent's call is waiting for the user's answer.
    pub fn has_pending_permission(&self) -> bool {
        !self.pending_permissions.is_empty()
    }
}

#[cfg(test)]
#[path = "permission_tests.rs"]
mod tests;
