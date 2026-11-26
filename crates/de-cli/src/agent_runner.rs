//! Runs `de agent` tasks: load → design agent → atomic save → exports,
//! one at a time or `--parallel N` at once.
//!
//! The output file is written only after the agent finished, through the
//! app's atomic save (sibling temp file + rename), so a failed, timed-out
//! or cancelled task never leaves a partial or changed `--out`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use de_ai::chat_provider::ChatDelta;
use de_ai::chat_usage::{TokenUsage, UsageMeter};
use de_editor_core::{EditorState, Locale};
use de_host_services::design_agent_run::{
    run_design_agent, DesignAgentRun, DesignAgentRunError, DesignAgentRunReport, RunControl,
};
use de_host_services::doc_io;
use de_host_services::headless_export::{
    export_document, HeadlessExportFormat, HeadlessExportReport,
};

use crate::agent_progress::Progress;
use crate::agent_provider::ResolvedProvider;
use crate::agent_task::{file_stem, TaskSpec};

/// A task with the provider it runs on (resolved before anything starts).
pub(crate) struct PlannedTask {
    pub spec: TaskSpec,
    pub provider: ResolvedProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Ok,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Ok => "ok",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

/// The step a task failed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Load,
    Agent,
    Save,
    Export,
}

impl Stage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Stage::Load => "load",
            Stage::Agent => "agent",
            Stage::Save => "save",
            Stage::Export => "export",
        }
    }
}

/// One format's export outcome.
#[derive(Debug, Clone)]
pub(crate) struct ExportOutcome {
    pub format: HeadlessExportFormat,
    pub result: Result<HeadlessExportReport, String>,
}

impl ExportOutcome {
    fn succeeded(&self) -> bool {
        matches!(&self.result, Ok(report) if report.failed.is_empty())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TaskResult {
    pub spec: TaskSpec,
    pub provider_label: String,
    pub model: String,
    pub status: TaskStatus,
    /// Where and why a failed (or cancelled) task stopped.
    pub error: Option<(Stage, String)>,
    pub duration: Duration,
    pub report: Option<DesignAgentRunReport>,
    pub exports: Vec<ExportOutcome>,
    /// True once `out` was written.
    pub saved: bool,
    /// Tokens the provider reported for this task, failed runs included.
    pub usage: TokenUsage,
    /// Provider requests this task made.
    pub requests: u64,
}

/// Run every task, at most `parallel` at once, in task order of results.
pub(crate) fn run_tasks(
    tasks: Vec<PlannedTask>,
    parallel: usize,
    cancel: &Arc<AtomicBool>,
    progress: &Progress,
) -> Vec<TaskResult> {
    let total = tasks.len();
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<TaskResult>>> = Mutex::new((0..total).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..parallel.clamp(1, total.max(1)) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(task) = tasks.get(index) else {
                    return;
                };
                let result = run_one(task, cancel, progress);
                results.lock().expect("results mutex poisoned")[index] = Some(result);
            });
        }
    });
    results
        .into_inner()
        .expect("results mutex poisoned")
        .into_iter()
        .map(|result| result.expect("every task ran"))
        .collect()
}

fn run_one(task: &PlannedTask, cancel: &Arc<AtomicBool>, progress: &Progress) -> TaskResult {
    let spec = &task.spec;
    let started = Instant::now();
    let mut result = TaskResult {
        spec: spec.clone(),
        provider_label: task.provider.label(),
        model: task.provider.model.clone(),
        status: TaskStatus::Ok,
        error: None,
        duration: Duration::ZERO,
        report: None,
        exports: Vec::new(),
        saved: false,
        usage: TokenUsage::default(),
        requests: 0,
    };
    let meter = Arc::new(UsageMeter::new());
    let finish = |mut result: TaskResult| {
        result.duration = started.elapsed();
        result.usage = meter.total();
        result.requests = meter.requests();
        progress.finished(&result);
        result
    };
    if cancel.load(Ordering::Acquire) {
        result.status = TaskStatus::Cancelled;
        result.error = Some((Stage::Agent, DesignAgentRunError::Cancelled.to_string()));
        return finish(result);
    }
    progress.started(spec, &result.provider_label, &result.model);

    let mut state = match load(spec) {
        Ok(state) => state,
        Err(message) => return finish(failed(result, Stage::Load, message)),
    };
    let run = DesignAgentRun {
        brief: spec.prompt.clone(),
        timeout: spec.timeout,
        max_turns: spec.max_turns,
        provider: task.provider.config.clone(),
        model: Some(task.provider.model.clone()),
    };
    let mut on_delta = |delta: &ChatDelta| progress.delta(&spec.name, delta);
    let control = RunControl {
        cancel: Some(cancel.clone()),
        on_delta: Some(&mut on_delta),
        usage: Some(meter.clone()),
    };
    let outcome = match run_design_agent(&state, &run, control) {
        Ok(outcome) => outcome,
        Err(DesignAgentRunError::Cancelled) => {
            result.status = TaskStatus::Cancelled;
            result.error = Some((Stage::Agent, DesignAgentRunError::Cancelled.to_string()));
            return finish(result);
        }
        Err(error) => return finish(failed(result, Stage::Agent, error.to_string())),
    };
    result.report = Some(outcome.report);
    if !state.apply(outcome.command) {
        let message = "the agent's design could not be applied to the document".to_string();
        return finish(failed(result, Stage::Agent, message));
    }
    // Ctrl+C after the agent finished still leaves `out` untouched.
    if cancel.load(Ordering::Acquire) {
        result.status = TaskStatus::Cancelled;
        result.error = Some((Stage::Save, DesignAgentRunError::Cancelled.to_string()));
        return finish(result);
    }
    if let Err(message) = save(&mut state, &spec.output) {
        return finish(failed(result, Stage::Save, message));
    }
    result.saved = true;
    for format in &spec.exports {
        let outcome = ExportOutcome {
            format: *format,
            result: export_document(&state, *format, &spec.export_dir, &file_stem(&spec.output))
                .map_err(|error| error.to_string()),
        };
        result.exports.push(outcome);
    }
    if let Some(bad) = result.exports.iter().find(|export| !export.succeeded()) {
        let message = match &bad.result {
            Err(message) => format!("{}: {message}", bad.format.as_str()),
            Ok(report) => format!(
                "{}: {} file(s) failed: {}",
                bad.format.as_str(),
                report.failed.len(),
                report
                    .failed
                    .iter()
                    .map(|(file, error)| format!("{file} ({error})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        return finish(failed(result, Stage::Export, message));
    }
    finish(result)
}

fn failed(mut result: TaskResult, stage: Stage, message: String) -> TaskResult {
    result.status = TaskStatus::Failed;
    result.error = Some((stage, message));
    result
}

fn load(spec: &TaskSpec) -> Result<EditorState, String> {
    match &spec.input {
        None => Ok(EditorState::new()),
        Some(path) => doc_io::load_editor_state_with_report(path, Locale::EnUs)
            .map(|loaded| loaded.state)
            .map_err(|error| format!("cannot open {}: {error}", path.display())),
    }
}

fn save(state: &mut EditorState, output: &Path) -> Result<(), String> {
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    doc_io::set_file_name_display(state, Some(output));
    doc_io::save_to_path(state, output)
        .map_err(|error| format!("cannot save {}: {error}", output.display()))
}
