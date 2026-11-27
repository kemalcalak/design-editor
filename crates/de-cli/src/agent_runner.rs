//! Runs `de agent` tasks: load → design agent → atomic save → exports,
//! one at a time or `--parallel N` at once.
//!
//! The output file is written only after the agent finished, through the
//! app's atomic save (sibling temp file + rename), so a failed, timed-out
//! or cancelled task never leaves a partial or changed `--out`. Ctrl+C
//! after the save skips the task's remaining exports and marks it
//! `interrupted`; the run then exits 130 like any cancelled run.

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
    export_document, HeadlessExportError, HeadlessExportFormat, HeadlessExportReport,
};

use crate::agent_progress::Progress;
use crate::agent_provider::ResolvedProvider;
pub(crate) use crate::agent_runner_error::TaskError;
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
    /// Stopped by Ctrl+C before `out` was written.
    Cancelled,
    /// Stopped by Ctrl+C after `out` was written: exports were skipped.
    Interrupted,
}

impl TaskStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Ok => "ok",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Interrupted => "interrupted",
        }
    }

    /// Cancelled or interrupted: the run was stopped by Ctrl+C.
    pub(crate) fn stopped(self) -> bool {
        matches!(self, TaskStatus::Cancelled | TaskStatus::Interrupted)
    }
}

/// One format's export outcome.
#[derive(Debug, Clone)]
pub(crate) struct ExportOutcome {
    pub format: HeadlessExportFormat,
    pub result: Result<HeadlessExportReport, HeadlessExportError>,
}

impl ExportOutcome {
    fn error(&self) -> Option<TaskError> {
        match &self.result {
            Err(source) => Some(TaskError::Export {
                format: self.format,
                source: source.clone(),
            }),
            Ok(report) if !report.failed.is_empty() => Some(TaskError::ExportPartial {
                format: self.format,
                failed: report.failed.clone(),
            }),
            Ok(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TaskResult {
    pub spec: TaskSpec,
    pub provider_label: String,
    pub model: String,
    pub status: TaskStatus,
    /// Why a task that is not `Ok` stopped.
    pub error: Option<TaskError>,
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
    let meter = Arc::new(UsageMeter::new());
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
    let outcome = if cancel.load(Ordering::Acquire) {
        Err(TaskError::Agent(DesignAgentRunError::Cancelled))
    } else {
        progress.started(spec, &result.provider_label, &result.model);
        run_steps(task, cancel, progress, &meter, &mut result)
    };
    if let Err(error) = outcome {
        result.status = status_for(&error, result.saved);
        result.error = Some(error);
    }
    result.duration = started.elapsed();
    result.usage = meter.total();
    result.requests = meter.requests();
    progress.finished(&result);
    result
}

/// The status of a task that stopped with `error`: a cancellation is
/// `Interrupted` once `out` was saved (exports were skipped) and `Cancelled`
/// before; anything else is `Failed`.
pub(crate) fn status_for(error: &TaskError, saved: bool) -> TaskStatus {
    match (error.is_cancellation(), saved) {
        (false, _) => TaskStatus::Failed,
        (true, true) => TaskStatus::Interrupted,
        (true, false) => TaskStatus::Cancelled,
    }
}

fn run_steps(
    task: &PlannedTask,
    cancel: &Arc<AtomicBool>,
    progress: &Progress,
    meter: &Arc<UsageMeter>,
    result: &mut TaskResult,
) -> Result<(), TaskError> {
    let spec = &task.spec;
    let mut state = load(spec)?;
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
    let outcome = run_design_agent(&state, &run, control)?;
    result.report = Some(outcome.report);
    if !state.apply(outcome.command) {
        return Err(TaskError::Apply);
    }
    // Ctrl+C after the agent finished still leaves `out` untouched.
    if cancel.load(Ordering::Acquire) {
        return Err(TaskError::CancelledBeforeSave);
    }
    save(&mut state, &spec.output)?;
    result.saved = true;
    let stem = file_stem(&spec.output);
    for (position, format) in spec.exports.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err(TaskError::Interrupted {
                skipped: spec.exports[position..].to_vec(),
            });
        }
        result.exports.push(ExportOutcome {
            format: *format,
            result: export_document(&state, *format, &spec.export_dir, &stem),
        });
    }
    match result.exports.iter().find_map(ExportOutcome::error) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Each task's document owns its blur-up thumbnails (#469), so parallel
/// tasks load and save without trading them and need no shared lock.
fn load(spec: &TaskSpec) -> Result<EditorState, TaskError> {
    match &spec.input {
        None => Ok(EditorState::new()),
        Some(path) => doc_io::load_editor_state_with_report(path, Locale::EnUs)
            .map(|loaded| loaded.state)
            .map_err(|source| TaskError::Load {
                path: path.display().to_string(),
                source,
            }),
    }
}

fn save(state: &mut EditorState, output: &Path) -> Result<(), TaskError> {
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|error| TaskError::CreateDir {
            path: parent.display().to_string(),
            detail: error.to_string(),
        })?;
    }
    doc_io::set_file_name_display(state, Some(output));
    doc_io::save_to_path(state, output).map_err(|source| TaskError::Save {
        path: output.display().to_string(),
        source,
    })
}
