//! Rename layers (#383) — the agent half on the desktop. Opening the
//! preview raises `ai_requested`; this drains it, asks the chat model the
//! user selected for names on a worker thread, and merges the reply into
//! the rows the user has not edited. With no model configured (or on any
//! failure) the preview keeps the offline rules' names.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;

use de_ai::chat_provider::{ChatDelta, ChatProvider, ChatRequest, EffortLevel, ThinkingMode};
use de_editor_core::rename_layers::ai::{
    parse_rename_layers_reply, rename_layer_briefs, rename_layers_prompt,
};
use de_editor_core::rename_layers::{LayerNameProposal, RenameLayersSource};
use de_editor_core::EditorState;

use crate::chat_session::{provider_for_selected_model, selected_cli_model_id};
use crate::rename_layers_error::RenameLayersAiError;
use crate::DesktopApp;

type Outcome = Result<BTreeMap<String, String>, RenameLayersAiError>;

/// One agent turn. Dropping it (the preview closed, or a new preview
/// asked again) cancels the turn: a provider with an abortable transport
/// stops at once, any other one is left to finish and its reply dropped.
pub(crate) struct RenameLayersSession {
    rx: Receiver<Outcome>,
    cancel: Arc<AtomicBool>,
}

impl Drop for RenameLayersSession {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl RenameLayersSession {
    fn start(provider: Box<dyn ChatProvider>, request: ChatRequest) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_tx = tx.clone();
        let worker_cancel = Arc::clone(&cancel);
        let spawned = thread::Builder::new()
            .name("de-rename-layers".into())
            .spawn(move || {
                let outcome = run_provider_blocking(provider.as_ref(), request, &worker_cancel);
                let _ = worker_tx.send(outcome);
            });
        if let Err(err) = spawned {
            let _ = tx.send(Err(RenameLayersAiError::WorkerSpawn(err.to_string())));
        }
        Self { rx, cancel }
    }

    fn try_take(&self) -> Option<Outcome> {
        match self.rx.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(RenameLayersAiError::WorkerVanished)),
        }
    }
}

/// The agent request for the open preview, or `None` when it has no rows.
fn build_request(state: &EditorState, model: Option<String>) -> Option<ChatRequest> {
    let dialog = state.editor_ui.rename_layers.as_ref()?;
    if dialog.rows.is_empty() {
        return None;
    }
    let proposals: Vec<LayerNameProposal> = dialog
        .rows
        .iter()
        .map(|row| LayerNameProposal {
            node_id: row.node_id.as_str().to_string(),
            current: (!row.current.is_empty()).then(|| row.current.clone()),
            proposed: row.proposed.clone(),
        })
        .collect();
    let briefs = rename_layer_briefs(state.active_children(), &proposals);
    Some(ChatRequest {
        // CLI providers have no system slot: the instructions ride in the
        // user message, like the design.md turn.
        system_prompt: String::new(),
        user_message: rename_layers_prompt(&briefs, dialog.locale),
        history: Vec::new(),
        max_output_tokens: 8192,
        thinking: ThinkingMode::Disabled,
        effort: EffortLevel::Low,
        attachments: vec![],
        model,
    })
}

fn run_provider_blocking(
    provider: &dyn ChatProvider,
    request: ChatRequest,
    cancel: &Arc<AtomicBool>,
) -> Outcome {
    let mut reply = String::new();
    for delta in provider.send_cancellable(request, Arc::clone(cancel)) {
        if cancel.load(Ordering::Acquire) {
            return Err(RenameLayersAiError::Cancelled);
        }
        match delta {
            ChatDelta::TextDelta(text) => reply.push_str(&text),
            ChatDelta::Done { .. } => break,
            ChatDelta::Error(message) => return Err(RenameLayersAiError::Provider(message)),
            _ => {}
        }
    }
    Ok(parse_rename_layers_reply(&reply))
}

/// Land an agent turn in the open preview (a closed preview drops it).
fn land(state: &mut EditorState, outcome: Outcome) {
    match outcome {
        Ok(names) => state.merge_rename_layers_ai(&names),
        Err(err) => {
            eprintln!("design-editor: rename layers agent failed: {err}");
            state.fail_rename_layers_ai();
        }
    }
}

impl DesktopApp {
    /// Ask the selected model for names when a preview just opened.
    pub(crate) fn drain_rename_layers_ai(&mut self) -> bool {
        let requested = self
            .host
            .editor_state_mut()
            .editor_ui
            .rename_layers
            .as_mut()
            .is_some_and(|dialog| std::mem::take(&mut dialog.ai_requested));
        if !requested {
            return false;
        }
        // A new preview supersedes a turn still running for an older one.
        self.current_rename_layers = None;
        // No model configured: the offline rules are the answer.
        let Some(provider) = provider_for_selected_model(&self.host) else {
            return false;
        };
        let model = selected_cli_model_id(&self.host);
        let Some(request) = build_request(self.host.editor_state(), model) else {
            return false;
        };
        self.current_rename_layers = Some(RenameLayersSession::start(provider, request));
        if let Some(dialog) = self
            .host
            .editor_state_mut()
            .editor_ui
            .rename_layers
            .as_mut()
        {
            dialog.source = RenameLayersSource::AiRunning;
        }
        self.host.mark_editor_state_dirty();
        true
    }

    /// Merge a finished agent turn into the preview.
    pub(crate) fn poll_rename_layers_ai(&mut self) -> bool {
        // The preview closed: cancel the turn instead of waiting it out.
        if self.current_rename_layers.is_some()
            && self.host.editor_state().editor_ui.rename_layers.is_none()
        {
            self.current_rename_layers = None;
            return false;
        }
        let Some(outcome) = self
            .current_rename_layers
            .as_ref()
            .and_then(RenameLayersSession::try_take)
        else {
            return false;
        };
        self.current_rename_layers = None;
        land(self.host.editor_state_mut(), outcome);
        self.host.mark_editor_state_dirty();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_ai::chat_provider::{EchoProvider, StopReason};
    use de_editor_core::NodeId;

    fn state() -> EditorState {
        let doc = serde_json::json!({"version": "1.0.0", "children": [
            {"type": "frame", "id": "screen", "name": "Frame 1", "width": 800, "height": 600,
             "children": [
                {"type": "frame", "id": "cta", "name": "Rectangle 3", "width": 160, "height": 48,
                 "fill": [{"type": "solid", "color": "#2563EB"}],
                 "children": [{"type": "text", "id": "label", "content": "Start"}]}
             ]}
        ]});
        let mut state = EditorState::from_document(
            jian_ops_schema::load_str(&doc.to_string())
                .expect("fixture")
                .value,
        );
        state.set_single_selection(NodeId::new("cta"));
        assert!(state.open_rename_layers());
        state
    }

    fn wait(session: &RenameLayersSession) -> Outcome {
        loop {
            if let Some(outcome) = session.try_take() {
                return outcome;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn the_agent_reply_renames_the_untouched_rows() {
        let mut state = state();
        let request = build_request(&state, Some("model-a".into())).expect("rows");
        assert!(request.user_message.contains("\"cta\""));
        assert!(request.user_message.contains("English"));
        assert_eq!(request.model.as_deref(), Some("model-a"));
        let provider = EchoProvider {
            script: vec![
                ChatDelta::TextDelta("```json\n{\"cta\": \"Start trial button\"}\n```".into()),
                ChatDelta::Done {
                    stop_reason: StopReason::EndTurn,
                },
            ],
        };
        let session = RenameLayersSession::start(Box::new(provider), request);
        land(&mut state, wait(&session));
        let dialog = state.editor_ui.rename_layers.as_ref().expect("open");
        assert_eq!(dialog.source, RenameLayersSource::Ai);
        assert_eq!(dialog.rows[0].proposed, "Start trial button");
    }

    #[test]
    fn a_cancelled_turn_lands_nothing() {
        let state = state();
        let request = build_request(&state, None).expect("rows");
        let provider = EchoProvider {
            script: vec![ChatDelta::TextDelta("{\"cta\": \"Go\"}".into())],
        };
        let cancel = Arc::new(AtomicBool::new(true));
        assert_eq!(
            run_provider_blocking(&provider, request, &cancel),
            Err(RenameLayersAiError::Cancelled)
        );
    }

    #[test]
    fn a_provider_error_keeps_the_rules() {
        let mut state = state();
        let request = build_request(&state, None).expect("rows");
        let provider = EchoProvider {
            script: vec![ChatDelta::Error("offline".into())],
        };
        let session = RenameLayersSession::start(Box::new(provider), request);
        land(&mut state, wait(&session));
        let dialog = state.editor_ui.rename_layers.as_ref().expect("open");
        assert_eq!(dialog.source, RenameLayersSource::AiFailed);
        assert_eq!(dialog.rows[0].proposed, "Primary button");
    }
}
