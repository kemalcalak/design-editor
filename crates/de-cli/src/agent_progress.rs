//! `de agent` progress lines on stderr, one line per event, prefixed with
//! the task name so parallel tasks stay readable. `--quiet` silences them.
//! Provider keys never reach these lines.

use std::io::Write;

use de_ai::chat_provider::ChatDelta;

use crate::agent_report::{format_duration, format_usage};
use crate::agent_runner::{TaskResult, TaskStatus};
use crate::agent_task::TaskSpec;

pub(crate) struct Progress {
    quiet: bool,
}

impl Progress {
    pub(crate) fn new(quiet: bool) -> Self {
        Self { quiet }
    }

    /// One line for task `name`. Locking stderr keeps lines from parallel
    /// tasks whole.
    pub(crate) fn line(&self, name: &str, text: &str) {
        if self.quiet {
            return;
        }
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "de agent [{name}] {text}");
    }

    /// A line not tied to a task.
    pub(crate) fn note(&self, text: &str) {
        if self.quiet {
            return;
        }
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "de agent: {text}");
    }

    pub(crate) fn started(&self, spec: &TaskSpec, provider: &str, model: &str) {
        self.line(&spec.name, &format!("started: {model} via {provider}"));
    }

    pub(crate) fn delta(&self, name: &str, delta: &ChatDelta) {
        match delta {
            ChatDelta::ToolUse { name: tool, .. } => self.line(name, &format!("tool {tool}")),
            ChatDelta::Error(message) => self.line(name, &format!("provider error: {message}")),
            ChatDelta::TextDelta(_) | ChatDelta::Thinking(_) | ChatDelta::Done { .. } => {}
        }
    }

    pub(crate) fn finished(&self, result: &TaskResult) {
        let took = format_duration(result.duration);
        let text = match result.status {
            TaskStatus::Ok => {
                let report = result.report.as_ref();
                format!(
                    "ok in {took}: {} tool calls, {} frame(s), {} -> {}",
                    report.map_or(0, |r| r.tool_calls),
                    report.map_or(0, |r| r.landed_roots),
                    format_usage(&result.usage),
                    de_util::display_path::display_path(&result.spec.output),
                )
            }
            TaskStatus::Failed => {
                let (stage, message) = result
                    .error
                    .as_ref()
                    .map(|(stage, message)| (stage.as_str(), message.as_str()))
                    .unwrap_or(("agent", "unknown error"));
                format!("failed in {took} ({stage}): {message}")
            }
            TaskStatus::Cancelled => format!("cancelled after {took}"),
        };
        self.line(&result.spec.name, &text);
    }
}
