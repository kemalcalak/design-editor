//! Headless design-agent run: the App's built-in design-agent loop against
//! a copy of a document, landed as one command.
//!
//! This is the one engine behind the MCP `run_design_agent` tool and the
//! `de agent` CLI, and the daemon-side equivalent of the desktop design
//! generation (`chat_session_launch_design.rs`): the same skills-assembled
//! system prompt (`de_ai_skills::design_agent_system_prompt_with_skills`),
//! the same tool-executing loop (`ConfiguredBuiltinProvider::send` →
//! `run_{anthropic,openai}_agent_loop`, `DESIGN_LOOP_MAX_TURNS`), the same
//! 15-tool surface including real offscreen `get_screenshot`, and the same
//! loop-end structural backstop (`finalize_on_exit`).
//!
//! ## How the result lands
//!
//! The loop runs against a CLONE of the document (the executor pattern
//! proven by de-smoke's headless loop mode), and the finished clone becomes
//! ONE `EditorCommand::Batch`: delete the original top-level frames, insert
//! the authored roots id-stably, and carry the variables/themes the loop set.
//! The caller applies it (the MCP host to its live document, the CLI to the
//! document it then saves). A pre-flight failure, timeout, cancellation or
//! empty result returns an error and no command, so the document stays
//! byte-identical.
//!
//! ## Blocking, timeout, cancellation
//!
//! [`run_design_agent`] BLOCKS until the loop drains, the budget elapses or
//! the caller's cancel flag is raised. The provider loop runs on the chat
//! runtime and is drained from a dedicated thread, so a deadline or a
//! cancel stops the stream (`send_cancellable`) without wedging the caller.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use de_ai::chat_provider::{
    ChatDelta, ChatProvider, ChatRequest, EffortLevel, FinalizeReport, StopReason, ThinkingMode,
};
use de_ai::chat_usage::{TokenUsage, UsageMeter};
use de_editor_core::{BuiltinAgentConfig, EditorCommand, EditorState, NodeId, PenNodeExt};

pub use crate::design_agent_run_error::DesignAgentRunError;
pub(crate) use crate::design_agent_run_executor::HeadlessDesignExecutor;

use crate::chat_builtin_http::{ConfiguredBuiltinProvider, DESIGN_LOOP_MAX_OUTPUT_TOKENS};
use crate::design_agent_tools::design_tool_defs;

/// Briefs above this size are refused before prompt assembly.
pub const MAX_BRIEF_BYTES: usize = 32 * 1024;
/// How often the drain re-checks the caller's cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// One headless run: the brief, its budget, and the provider to run it on.
#[derive(Debug, Clone)]
pub struct DesignAgentRun {
    pub brief: String,
    /// Wall-clock budget for the whole loop.
    pub timeout: Duration,
    /// Turn-cap override; `None` keeps `DESIGN_LOOP_MAX_TURNS`.
    pub max_turns: Option<usize>,
    /// A ready builtin provider configuration (saved, or built from flags).
    pub provider: BuiltinAgentConfig,
    /// One of the provider's saved models; `None` takes its first one.
    pub model: Option<String>,
}

/// Caller hooks for a run. `cancel` stops the loop as soon as it is raised;
/// `on_delta` sees every streamed delta on the calling thread (progress);
/// `usage`, when given, is the meter the run adds its token usage to, so the
/// caller can read what a failed or cancelled run spent too.
#[derive(Default)]
pub struct RunControl<'a> {
    pub cancel: Option<Arc<AtomicBool>>,
    pub on_delta: Option<&'a mut dyn FnMut(&ChatDelta)>,
    pub usage: Option<Arc<UsageMeter>>,
}

/// What a finished run did.
#[derive(Debug, Clone, Default)]
pub struct DesignAgentRunReport {
    /// The model the run used.
    pub model: String,
    pub tool_calls: usize,
    pub text_chars: usize,
    pub stop_reason: Option<StopReason>,
    /// Top-level frames the landing command inserts.
    pub landed_roots: usize,
    pub finalize: Option<FinalizeReport>,
    /// The last error delta the stream carried, when the loop still
    /// produced a design.
    pub loop_error: Option<String>,
    /// Tokens the provider reported across every request of the run.
    pub usage: TokenUsage,
    /// Provider requests the loop made.
    pub provider_requests: u64,
}

impl DesignAgentRunReport {
    /// The report as JSON (camelCase; the MCP tool's result shape).
    pub fn to_json(&self) -> serde_json::Value {
        let finalize = self.finalize.as_ref().map(|report| {
            serde_json::json!({
                "committedScreens": report.screens.committed,
                "unfilledScreens": report.screens.unfilled,
                "qualityChecks": report.quality.checks,
                "qualityRepairs": report
                    .quality
                    .repairs
                    .iter()
                    .map(|(key, count)| serde_json::json!({ "check": key, "repairs": count }))
                    .collect::<Vec<_>>(),
                "qualityNotes": report.quality.notes,
            })
        });
        serde_json::json!({
            "model": self.model,
            "toolCalls": self.tool_calls,
            "textChars": self.text_chars,
            "stopReason": self.stop_reason.map(|reason| format!("{reason:?}")),
            "landedRoots": self.landed_roots,
            "finalize": finalize,
            "loopError": self.loop_error,
            "usage": usage_json(&self.usage, self.provider_requests),
        })
    }
}

/// `{ inputTokens, outputTokens, requests }`; a count the provider never
/// reported is `null`.
pub fn usage_json(usage: &TokenUsage, requests: u64) -> serde_json::Value {
    serde_json::json!({
        "inputTokens": usage.input_tokens,
        "outputTokens": usage.output_tokens,
        "requests": requests,
    })
}

/// A finished run: the command that lands the design, and its report.
#[derive(Debug, Clone)]
pub struct DesignAgentRunOutcome {
    pub command: EditorCommand,
    pub report: DesignAgentRunReport,
}

/// Pick a saved builtin provider: `provider_id` by id, else the first ready
/// one. Returned only when it is ready (enabled, key, a saved model).
pub fn select_saved_provider<'a>(
    agents: &'a [BuiltinAgentConfig],
    provider_id: Option<&str>,
) -> Result<&'a BuiltinAgentConfig, DesignAgentRunError> {
    let config = match provider_id {
        Some(id) => agents
            .iter()
            .find(|agent| agent.id == id)
            .ok_or_else(|| DesignAgentRunError::UnknownProvider(id.to_string()))?,
        None => agents
            .iter()
            .find(|agent| agent.ready())
            .ok_or(DesignAgentRunError::NoConfiguredProvider)?,
    };
    if !config.ready() {
        return Err(DesignAgentRunError::ProviderNotReady(config.id.clone()));
    }
    Ok(config)
}

/// Check a brief the way every run does: trimmed, non-empty, bounded.
pub fn validate_brief(brief: &str) -> Result<String, DesignAgentRunError> {
    let brief = brief.trim();
    if brief.is_empty() {
        return Err(DesignAgentRunError::EmptyBrief);
    }
    if brief.len() > MAX_BRIEF_BYTES {
        return Err(DesignAgentRunError::BriefTooLarge {
            bytes: brief.len(),
            max: MAX_BRIEF_BYTES,
        });
    }
    Ok(brief.to_string())
}

/// Run the design agent on a copy of `state` and return the command that
/// lands its design. Blocks until the loop ends, the budget elapses or
/// `control.cancel` is raised.
pub fn run_design_agent(
    state: &EditorState,
    run: &DesignAgentRun,
    control: RunControl<'_>,
) -> Result<DesignAgentRunOutcome, DesignAgentRunError> {
    let brief = validate_brief(&run.brief)?;
    let provider = build_provider(&run.provider, run.model.as_deref())?;
    let model = provider.model().to_string();
    let meter = control.usage.clone().unwrap_or_default();
    let factory = |executor: Arc<HeadlessDesignExecutor>| -> Box<dyn DesignLoopDriver> {
        let provider = provider
            .clone()
            .with_canvas_tools(design_tool_defs(), executor)
            .with_loop_finalize()
            .with_usage_meter(meter.clone());
        let provider = match run.max_turns {
            Some(turns) => provider.with_max_turns(turns),
            None => provider,
        };
        Box::new(BuiltinLoopDriver { provider })
    };
    let mut outcome = run_with_driver(state, &brief, run.timeout, &factory, control)?;
    outcome.report.model = model;
    outcome.report.usage = meter.total();
    outcome.report.provider_requests = meter.requests();
    Ok(outcome)
}

fn build_provider(
    config: &BuiltinAgentConfig,
    model: Option<&str>,
) -> Result<ConfiguredBuiltinProvider, DesignAgentRunError> {
    if !config.ready() {
        return Err(DesignAgentRunError::ProviderNotReady(config.id.clone()));
    }
    match model {
        Some(model) => ConfiguredBuiltinProvider::from_builtin_agent_with_model(config, model)
            .ok_or_else(|| DesignAgentRunError::ModelNotSaved {
                provider: config.id.clone(),
                model: model.to_string(),
            }),
        None => ConfiguredBuiltinProvider::from_builtin_agent(config)
            .ok_or_else(|| DesignAgentRunError::ProviderNotReady(config.id.clone())),
    }
}

/// Transport seam so tests drive a run without a network provider: the
/// production driver wraps a wired [`ConfiguredBuiltinProvider`]; tests
/// script deltas and mutate the clone through the shared executor.
pub(crate) trait DesignLoopDriver: Send {
    fn run(
        &self,
        request: ChatRequest,
        cancel: Arc<AtomicBool>,
    ) -> Box<dyn Iterator<Item = ChatDelta> + Send>;
}

struct BuiltinLoopDriver {
    provider: ConfiguredBuiltinProvider,
}

impl DesignLoopDriver for BuiltinLoopDriver {
    fn run(
        &self,
        request: ChatRequest,
        cancel: Arc<AtomicBool>,
    ) -> Box<dyn Iterator<Item = ChatDelta> + Send> {
        self.provider.send_cancellable(request, cancel)
    }
}

/// Builds the driver AFTER the run wires the executor, so tests can capture
/// the executor while production attaches it to the provider's tool loop.
pub(crate) type DriverFactory<'f> =
    dyn Fn(Arc<HeadlessDesignExecutor>) -> Box<dyn DesignLoopDriver> + 'f;

/// The run with an injected driver: clone, loop, land.
pub(crate) fn run_with_driver(
    state: &EditorState,
    brief: &str,
    timeout: Duration,
    factory: &DriverFactory<'_>,
    control: RunControl<'_>,
) -> Result<DesignAgentRunOutcome, DesignAgentRunError> {
    let clone = Arc::new(Mutex::new(state.clone()));
    let executor = Arc::new(HeadlessDesignExecutor::new(clone.clone(), brief));
    let driver = factory(executor.clone());
    let request = ChatRequest {
        system_prompt: de_ai_skills::design_agent_system_prompt_with_skills(brief),
        user_message: brief.to_string(),
        history: Vec::new(),
        max_output_tokens: DESIGN_LOOP_MAX_OUTPUT_TOKENS,
        thinking: ThinkingMode::Adaptive,
        effort: EffortLevel::default(),
        attachments: Vec::new(),
        model: None,
    };
    let stats =
        drain(driver, request, Instant::now() + timeout, control).map_err(|error| match error {
            DesignAgentRunError::Timeout { .. } => DesignAgentRunError::Timeout {
                seconds: timeout.as_secs(),
            },
            other => other,
        })?;

    let finalize = executor.take_finalize_report();
    let finished = clone
        .lock()
        .expect("EditorState mutex poisoned after the loop drained")
        .clone();
    let Some(command) = landing_batch(state, &finished) else {
        return Err(match stats.last_error {
            Some(error) => DesignAgentRunError::Loop(error),
            None => DesignAgentRunError::EmptyResult,
        });
    };
    Ok(DesignAgentRunOutcome {
        command,
        report: DesignAgentRunReport {
            tool_calls: stats.tool_calls,
            text_chars: stats.text_chars,
            stop_reason: stats.stop_reason,
            landed_roots: finished.active_children().len(),
            finalize,
            loop_error: stats.last_error,
            ..DesignAgentRunReport::default()
        },
    })
}

struct DrainStats {
    tool_calls: usize,
    text_chars: usize,
    stop_reason: Option<StopReason>,
    last_error: Option<String>,
}

/// Drain the loop's delta stream on a dedicated thread so the deadline and
/// the caller's cancel flag can stop a stalled provider without wedging the
/// calling thread. The timeout's seconds are filled in by the caller.
fn drain(
    driver: Box<dyn DesignLoopDriver>,
    request: ChatRequest,
    deadline: Instant,
    mut control: RunControl<'_>,
) -> Result<DrainStats, DesignAgentRunError> {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<ChatDelta>();
    let thread_stop = stop.clone();
    std::thread::Builder::new()
        .name("de-design-agent-run".into())
        .spawn(move || {
            for delta in driver.run(request, thread_stop) {
                if tx.send(delta).is_err() {
                    break;
                }
            }
        })
        .expect("spawn de-design-agent-run drain thread");

    let cancelled = || {
        control
            .cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    };
    let mut stats = DrainStats {
        tool_calls: 0,
        text_chars: 0,
        stop_reason: None,
        last_error: None,
    };
    loop {
        if cancelled() {
            stop.store(true, Ordering::Release);
            return Err(DesignAgentRunError::Cancelled);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            stop.store(true, Ordering::Release);
            return Err(DesignAgentRunError::Timeout { seconds: 0 });
        }
        let delta = match rx.recv_timeout(remaining.min(CANCEL_POLL)) {
            Ok(delta) => delta,
            // Stream closed: the loop ended (Done normally precedes close).
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(stats),
            // Re-check the cancel flag and the deadline at the loop top.
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
        };
        if let Some(on_delta) = control.on_delta.as_mut() {
            on_delta(&delta);
        }
        match delta {
            ChatDelta::TextDelta(text) => stats.text_chars += text.len(),
            ChatDelta::Thinking(_) => {}
            ChatDelta::ToolUse { .. } => stats.tool_calls += 1,
            ChatDelta::Error(message) => stats.last_error = Some(message),
            ChatDelta::Done { stop_reason } => stats.stop_reason = Some(stop_reason),
        }
    }
}

/// Build the landing batch: replace the original top-level frames with the
/// clone's authored roots (id-stable), then carry variables/themes when the
/// loop changed them.
fn landing_batch(original: &EditorState, finished: &EditorState) -> Option<EditorCommand> {
    let authored = finished.active_children().to_vec();
    if authored.is_empty() {
        return None;
    }
    let mut commands = Vec::new();
    for root in original.active_children() {
        commands.push(EditorCommand::DeleteNode {
            node_id: NodeId::new(root.base().id.clone()),
            page_id: None,
        });
    }
    commands.push(EditorCommand::InsertAuthoredSubtreePreservingRoots {
        nodes: authored,
        parent_id: NodeId::NONE,
        page_id: None,
    });
    if finished.doc.variables != original.doc.variables {
        commands.push(EditorCommand::SetVariables {
            variables: finished.doc.variables.clone().unwrap_or_default(),
            replace: true,
        });
    }
    if finished.doc.themes != original.doc.themes {
        commands.push(EditorCommand::SetThemes {
            themes: finished.doc.themes.clone().unwrap_or_default(),
            replace: true,
        });
    }
    Some(EditorCommand::Batch { commands })
}

#[cfg(test)]
#[path = "design_agent_run_tests.rs"]
mod tests;
