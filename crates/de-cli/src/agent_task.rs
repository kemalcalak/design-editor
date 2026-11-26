//! One `de agent` task — a prompt, its input and output documents, exports,
//! and its run options — and the checks every task passes before anything
//! runs, whether it came from the command line or a tasks file.

use std::path::{Path, PathBuf};
use std::time::Duration;

use de_host_services::design_agent_run::{validate_brief, DesignAgentRunError};
use de_host_services::headless_export::HeadlessExportFormat;

use crate::agent_paths::is_portable_name;
use crate::agent_tasks_error::TaskProblem;

/// Budget when neither the task, the tasks file nor `--timeout` sets one.
pub(crate) const DEFAULT_TIMEOUT_SECS: u64 = 600;
pub(crate) const MAX_TIMEOUT_SECS: u64 = 86_400;
/// Same ceiling the MCP `run_design_agent` tool applies.
pub(crate) const MAX_TURNS_CEILING: u64 = 64;

/// Run options a task, the tasks file's `defaults` and the command line can
/// each give; the first that sets a field wins.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TaskOptions {
    pub exports: Option<Vec<String>>,
    pub export_dir: Option<PathBuf>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub timeout_secs: Option<u64>,
    pub max_turns: Option<u64>,
}

impl TaskOptions {
    /// `self`, with every unset field taken from `fallback`.
    pub(crate) fn or(self, fallback: &TaskOptions) -> TaskOptions {
        TaskOptions {
            exports: self.exports.or_else(|| fallback.exports.clone()),
            export_dir: self.export_dir.or_else(|| fallback.export_dir.clone()),
            provider: self.provider.or_else(|| fallback.provider.clone()),
            model: self.model.or_else(|| fallback.model.clone()),
            timeout_secs: self.timeout_secs.or(fallback.timeout_secs),
            max_turns: self.max_turns.or(fallback.max_turns),
        }
    }
}

/// A checked task, ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskSpec {
    pub name: String,
    pub prompt: String,
    /// The document to start from; `None` starts from an empty one.
    pub input: Option<PathBuf>,
    pub output: PathBuf,
    pub exports: Vec<HeadlessExportFormat>,
    /// This task's own export folder: `<export root>/<name>`.
    pub export_dir: PathBuf,
    /// A saved provider to use instead of the command line's choice.
    pub provider: Option<String>,
    pub model: Option<String>,
    pub timeout: Duration,
    pub max_turns: Option<usize>,
}

/// The raw fields of one task before checking.
#[derive(Debug, Clone, Default)]
pub(crate) struct RawTaskFields {
    pub prompt: Option<String>,
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub options: TaskOptions,
}

/// Check one task's fields. `name` is already chosen by the caller.
pub(crate) fn build_task(name: String, raw: RawTaskFields) -> Result<TaskSpec, TaskProblem> {
    if !is_portable_name(&name) {
        return Err(TaskProblem::InvalidName { name });
    }
    let prompt = raw
        .prompt
        .map(|prompt| prompt.trim().to_string())
        .filter(|prompt| !prompt.is_empty())
        .ok_or(TaskProblem::MissingPrompt)?;
    // The engine's own check, run here so an oversized prompt stops the run
    // before any task starts (exit 2) instead of failing task by task.
    let prompt = match validate_brief(&prompt) {
        Ok(prompt) => prompt,
        Err(DesignAgentRunError::BriefTooLarge { bytes, max }) => {
            return Err(TaskProblem::PromptTooLarge { bytes, max })
        }
        Err(_) => return Err(TaskProblem::MissingPrompt),
    };
    let output = raw.output.ok_or(TaskProblem::MissingOut)?;
    if !has_extension(&output, &["de"]) {
        return Err(TaskProblem::OutNotDe {
            out: output.display().to_string(),
        });
    }
    if let Some(input) = &raw.input {
        if !has_extension(input, &["de", "pen", "pptx"]) {
            return Err(TaskProblem::UnsupportedInput {
                input: input.display().to_string(),
            });
        }
    }
    let options = raw.options;
    let mut exports = Vec::new();
    for value in options.exports.iter().flatten() {
        let format =
            HeadlessExportFormat::parse(value).ok_or_else(|| TaskProblem::UnknownExport {
                value: value.clone(),
            })?;
        if !exports.contains(&format) {
            exports.push(format);
        }
    }
    let timeout_secs = options.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
    if !(1..=MAX_TIMEOUT_SECS).contains(&timeout_secs) {
        return Err(TaskProblem::InvalidTimeout {
            value: timeout_secs,
        });
    }
    let max_turns = match options.max_turns {
        None => None,
        Some(value) if (1..=MAX_TURNS_CEILING).contains(&value) => Some(value as usize),
        Some(value) => return Err(TaskProblem::InvalidMaxTurns { value }),
    };
    // Every task exports into a folder of its own under the export root, so
    // tasks sharing a root never overwrite each other's files or `assets/`.
    let export_dir = options
        .export_dir
        .unwrap_or_else(|| default_export_root(&output))
        .join(&name);
    Ok(TaskSpec {
        name,
        prompt,
        input: raw.input,
        output,
        exports,
        export_dir,
        provider: options.provider,
        model: options.model,
        timeout: Duration::from_secs(timeout_secs),
        max_turns,
    })
}

/// `<out folder>/exports`: the export root when none is given; the task's
/// own folder goes inside it.
pub(crate) fn default_export_root(output: &Path) -> PathBuf {
    output
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("exports")
}

/// The file name without its extension (`design` for `out/design.de`).
pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "design".to_string())
}

fn has_extension(path: &Path, allowed: &[&str]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| allowed.iter().any(|a| ext.eq_ignore_ascii_case(a)))
}
