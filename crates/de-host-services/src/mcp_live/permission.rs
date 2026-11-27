//! "Ask before destructive agent actions" for agents on the live MCP server
//! (#398): CLI and ACP agents drive the editor through this endpoint, so a
//! destructive `tools/call` waits for the same Allow / Deny prompt the
//! in-app loops use (`EditorState::gate_agent_tool_with`), shown on the
//! active chat tab.
//!
//! The connection thread classifies the call and, when it is destructive,
//! asks the UI thread BEFORE taking the state lock (a call waiting on the
//! user never blocks other clients' reads) and blocks until the answer, with
//! no timeout: the UI answers at once while the setting is off, and a server
//! stop drops the question, which reads as declined.

use std::sync::mpsc::{self, Sender, SyncSender};

use de_editor_core::agent_permission::{DestructiveAction, ToolGate};
use de_editor_core::EditorState;
use de_mcp::{RequestId, ToolErrorCode, ToolResponse};

use super::{McpLiveServer, McpPumpOutcome, UiRequest, UiWake};

/// One question waiting for the user.
pub(super) struct PendingPermission {
    pub(super) tool: String,
    pub(super) args: String,
    pub(super) action: DestructiveAction,
    pub(super) ack: SyncSender<bool>,
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

/// Ask the UI thread whether the destructive call may run. False when the
/// user declined, or the server stopped before an answer.
pub(super) fn request_permission(
    req_tx: &Sender<UiRequest>,
    wake_ui: &UiWake,
    tool: String,
    args: String,
    action: DestructiveAction,
) -> bool {
    let (ack_tx, ack_rx) = mpsc::sync_channel(1);
    let request = UiRequest::Permission(PendingPermission {
        tool,
        args,
        action,
        ack: ack_tx,
    });
    if req_tx.send(request).is_err() {
        return false;
    }
    wake_ui();
    ack_rx.recv().unwrap_or(false)
}

/// The reply to a declined call: the tool-level error the in-app loops give
/// their agent.
pub(super) fn declined_response(id: &RequestId) -> String {
    de_mcp::json_serializer::tool_response_to_json(&ToolResponse::Err {
        id: id.clone(),
        code: ToolErrorCode::InvalidArgument,
        message: de_editor_core::agent_permission::DECLINED_RESULT.to_string(),
    })
}

impl McpLiveServer {
    /// Settle the questions waiting on the user: run, decline, or keep
    /// waiting (a new prompt repaints).
    pub(super) fn pump_permissions(
        &mut self,
        state: &mut EditorState,
        outcome: &mut McpPumpOutcome,
    ) {
        self.pending_permissions.retain(|pending| {
            let action = pending.action.clone();
            match state.gate_agent_tool_with(None, &pending.tool, &pending.args, || Some(action)) {
                ToolGate::Run => {
                    let _ = pending.ack.send(true);
                    false
                }
                ToolGate::Declined => {
                    let _ = pending.ack.send(false);
                    false
                }
                ToolGate::Asked => {
                    outcome.repaint = true;
                    true
                }
                ToolGate::Wait => true,
            }
        });
    }

    /// Whether an agent's call is waiting for the user's answer.
    pub fn has_pending_permission(&self) -> bool {
        !self.pending_permissions.is_empty()
    }
}

#[cfg(test)]
#[path = "permission_tests.rs"]
mod tests;
