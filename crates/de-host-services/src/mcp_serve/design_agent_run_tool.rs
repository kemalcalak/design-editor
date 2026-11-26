//! MCP tool `run_design_agent` — run the App's built-in design-agent loop
//! headlessly against the active document and land the finished design.
//!
//! The engine is [`crate::design_agent_run`], shared with the `de agent`
//! CLI; this tool parses the MCP arguments, picks the provider from the
//! daemon's persisted agent settings
//! (`EditorState.editor_ui.agent_settings.builtin_agents`, never from the
//! request), and returns the landing command for the host to apply. A
//! failed, timed-out or empty run returns an error and no command, so the
//! live document stays byte-identical.

use std::collections::BTreeMap;
use std::time::Duration;

use de_editor_core::EditorState;
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};

#[cfg(test)]
use crate::design_agent_run::run_with_driver;
use crate::design_agent_run::{
    run_design_agent, select_saved_provider, validate_brief, DesignAgentRun, DesignAgentRunError,
    DesignAgentRunOutcome, RunControl,
};
#[cfg(test)]
pub(crate) use crate::design_agent_run::{DesignLoopDriver, HeadlessDesignExecutor};

/// Default and ceiling wall-clock budgets. A full design loop with
/// screenshots takes minutes; the ceiling keeps a stuck provider from
/// pinning a connection thread for an hour.
const DEFAULT_TIMEOUT_SECONDS: u64 = 600;
const MAX_TIMEOUT_SECONDS: u64 = 1800;
/// `max_turns` ceiling — matches the loop's own hard bound territory.
const MAX_TURNS_CEILING: usize = 64;

/// Test seam: builds a scripted driver around the run's executor.
#[cfg(test)]
pub(crate) type TestDriverFactory =
    Box<dyn Fn(std::sync::Arc<HeadlessDesignExecutor>) -> Box<dyn DesignLoopDriver> + Send + Sync>;

/// The `run_design_agent` tool: a snapshot of the document at registration
/// time. Tests inject a scripted driver instead of the saved provider.
pub struct RunDesignAgentTool {
    state: EditorState,
    #[cfg(test)]
    driver_factory: Option<TestDriverFactory>,
}

/// Snapshot constructor used by the registry.
pub fn run_design_agent_snapshot(doc: &EditorState) -> RunDesignAgentTool {
    RunDesignAgentTool {
        state: doc.clone(),
        #[cfg(test)]
        driver_factory: None,
    }
}

#[cfg(test)]
impl RunDesignAgentTool {
    /// Test-only constructor with an injected loop driver (never network).
    pub(crate) fn for_test(
        doc: &EditorState,
        driver_factory: TestDriverFactory,
    ) -> RunDesignAgentTool {
        RunDesignAgentTool {
            state: doc.clone(),
            driver_factory: Some(driver_factory),
        }
    }
}

struct ParsedArgs {
    brief: String,
    timeout: Duration,
    max_turns: Option<usize>,
    provider_id: Option<String>,
    model: Option<String>,
}

fn parse_args(args: &BTreeMap<String, String>) -> Result<ParsedArgs, String> {
    let brief = validate_brief(args.get("brief").map(String::as_str).unwrap_or_default())
        .map_err(|error| error.to_string())?;
    let timeout_seconds = match args.get("timeout_seconds").map(|value| value.trim()) {
        None | Some("") => DEFAULT_TIMEOUT_SECONDS,
        Some(raw) => {
            let parsed = raw
                .parse::<u64>()
                .map_err(|_| format!("invalid timeout_seconds: {raw}"))?;
            if parsed == 0 {
                return Err("timeout_seconds must be greater than zero".to_string());
            }
            parsed.min(MAX_TIMEOUT_SECONDS)
        }
    };
    let max_turns = match args.get("max_turns").map(|value| value.trim()) {
        None | Some("") => None,
        Some(raw) => {
            let parsed = raw
                .parse::<usize>()
                .map_err(|_| format!("invalid max_turns: {raw}"))?;
            if parsed == 0 {
                return Err("max_turns must be greater than zero".to_string());
            }
            Some(parsed.min(MAX_TURNS_CEILING))
        }
    };
    let optional = |key: &str| {
        args.get(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    Ok(ParsedArgs {
        brief,
        timeout: Duration::from_secs(timeout_seconds),
        max_turns,
        provider_id: optional("provider_id"),
        model: optional("model"),
    })
}

impl RunDesignAgentTool {
    fn run(&self, parsed: &ParsedArgs) -> Result<DesignAgentRunOutcome, DesignAgentRunError> {
        #[cfg(test)]
        if let Some(factory) = &self.driver_factory {
            return run_with_driver(
                &self.state,
                &parsed.brief,
                parsed.timeout,
                factory.as_ref(),
                RunControl::default(),
            );
        }
        let provider = select_saved_provider(
            &self.state.editor_ui.agent_settings.builtin_agents,
            parsed.provider_id.as_deref(),
        )?
        .clone();
        let run = DesignAgentRun {
            brief: parsed.brief.clone(),
            timeout: parsed.timeout,
            max_turns: parsed.max_turns,
            provider,
            model: parsed.model.clone(),
        };
        run_design_agent(&self.state, &run, RunControl::default())
    }
}

impl McpTool for RunDesignAgentTool {
    fn name(&self) -> &str {
        "run_design_agent"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let parsed = match parse_args(args) {
            Ok(parsed) => parsed,
            Err(message) => return ToolOutcome::Err(ToolErrorCode::InvalidArgument, message),
        };
        match self.run(&parsed) {
            Ok(outcome) => ToolOutcome::OkJsonWithCommand(
                outcome.report.to_json().to_string(),
                outcome.command,
            ),
            Err(error) => {
                let code = match error {
                    DesignAgentRunError::NoConfiguredProvider
                    | DesignAgentRunError::UnknownProvider(_)
                    | DesignAgentRunError::ProviderNotReady(_)
                    | DesignAgentRunError::ModelNotSaved { .. }
                    | DesignAgentRunError::EmptyBrief
                    | DesignAgentRunError::BriefTooLarge { .. } => ToolErrorCode::InvalidArgument,
                    DesignAgentRunError::Timeout { .. }
                    | DesignAgentRunError::Cancelled
                    | DesignAgentRunError::Loop(_)
                    | DesignAgentRunError::EmptyResult => ToolErrorCode::ToolFailed,
                };
                ToolOutcome::Err(code, error.to_string())
            }
        }
    }
}
