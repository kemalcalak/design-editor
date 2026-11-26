//! `de agent --tasks <file>`: the tasks file.
//!
//! ```yaml
//! defaults:            # optional; every task inherits these
//!   export: [png, pdf] # or a single name: png
//!   exportDir: exports # default: <out folder>/<out stem>-export
//!   provider: Claude   # a saved provider (id, name or preset)
//!   model: claude-sonnet-4-5
//!   timeout: 900       # seconds, 1-86400 (default 600)
//!   maxTurns: 28       # 1-64
//! tasks:
//!   - name: landing    # optional; default: the out file's stem
//!     prompt: A landing page for a coffee roaster
//!     in: base.de      # optional .de / .pen / .pptx; default: empty document
//!     out: out/landing.de
//!     export: png      # any defaults key can be set per task
//! ```
//!
//! JSON is accepted too (YAML is a superset). Relative paths resolve
//! against the tasks file's folder. Unknown keys are errors, so a typo
//! never silently drops a setting. A task's own value wins over the file's
//! `defaults`, which win over the command-line flags.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::agent_task::{build_task, file_stem, RawTaskFields, TaskOptions, TaskSpec};
use crate::agent_tasks_error::TasksFileError;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TasksFile {
    #[serde(default)]
    defaults: RawOptions,
    tasks: Vec<RawTask>,
}

/// `export: png` or `export: [png, pdf]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(value) => value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
            OneOrMany::Many(values) => values,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawOptions {
    export: Option<OneOrMany>,
    export_dir: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    timeout: Option<u64>,
    max_turns: Option<u64>,
}

/// `RawOptions`' keys plus the task's own (serde cannot combine `flatten`
/// with `deny_unknown_fields`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawTask {
    name: Option<String>,
    prompt: Option<String>,
    #[serde(rename = "in")]
    input: Option<String>,
    out: Option<String>,
    export: Option<OneOrMany>,
    export_dir: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    timeout: Option<u64>,
    max_turns: Option<u64>,
}

impl RawOptions {
    fn into_options(self, base: &Path) -> TaskOptions {
        TaskOptions {
            exports: self.export.map(OneOrMany::into_vec),
            export_dir: self.export_dir.map(|dir| resolve(base, &dir)),
            provider: non_blank(self.provider),
            model: non_blank(self.model),
            timeout_secs: self.timeout,
            max_turns: self.max_turns,
        }
    }
}

/// Read and check a tasks file. `cli` holds the command line's run options,
/// the lowest-priority layer.
pub(crate) fn load_tasks_file(
    path: &Path,
    cli: &TaskOptions,
) -> Result<Vec<TaskSpec>, TasksFileError> {
    let text = std::fs::read_to_string(path).map_err(|error| TasksFileError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    let base = path.parent().unwrap_or_else(|| Path::new(""));
    parse_tasks(&text, &path.display().to_string(), base, cli)
}

/// Parse tasks-file text. `label` names the file in errors; `base` is the
/// folder relative paths resolve against.
pub(crate) fn parse_tasks(
    text: &str,
    label: &str,
    base: &Path,
    cli: &TaskOptions,
) -> Result<Vec<TaskSpec>, TasksFileError> {
    let file: TasksFile = serde_saphyr::from_str(text).map_err(|error| TasksFileError::Parse {
        path: label.to_string(),
        detail: error.to_string(),
    })?;
    if file.tasks.is_empty() {
        return Err(TasksFileError::NoTasks {
            path: label.to_string(),
        });
    }
    let defaults = file.defaults.into_options(base).or(cli);
    let explicit: BTreeSet<String> = file
        .tasks
        .iter()
        .filter_map(|task| non_blank(task.name.clone()))
        .collect();
    let mut names = BTreeSet::new();
    let mut outs: BTreeMap<PathBuf, usize> = BTreeMap::new();
    let mut specs = Vec::with_capacity(file.tasks.len());
    for (offset, task) in file.tasks.into_iter().enumerate() {
        let index = offset + 1;
        let explicit_name = non_blank(task.name.clone());
        let output = task.out.as_deref().map(|out| resolve(base, out));
        let name = match &explicit_name {
            Some(name) => {
                if !names.insert(name.clone()) {
                    return Err(TasksFileError::DuplicateName { name: name.clone() });
                }
                name.clone()
            }
            None => default_name(output.as_deref(), index, &explicit, &mut names),
        };
        let raw = RawTaskFields {
            prompt: task.prompt,
            input: task.input.as_deref().map(|input| resolve(base, input)),
            output,
            options: RawOptions {
                export: task.export,
                export_dir: task.export_dir,
                provider: task.provider,
                model: task.model,
                timeout: task.timeout,
                max_turns: task.max_turns,
            }
            .into_options(base)
            .or(&defaults),
        };
        let spec = build_task(name, raw).map_err(|problem| TasksFileError::Task {
            index,
            name: explicit_name,
            problem,
        })?;
        if let Some(first) = outs.insert(normalized(&spec.output), index) {
            return Err(TasksFileError::DuplicateOut {
                out: spec.output.display().to_string(),
                first,
                second: index,
            });
        }
        specs.push(spec);
    }
    Ok(specs)
}

/// An unnamed task is called after its `out` file, or `task-<n>` when that
/// name is taken.
fn default_name(
    output: Option<&Path>,
    index: usize,
    explicit: &BTreeSet<String>,
    taken: &mut BTreeSet<String>,
) -> String {
    let candidate = output.map(file_stem);
    let name = match candidate {
        Some(stem) if !explicit.contains(&stem) && !taken.contains(&stem) => stem,
        _ => format!("task-{index}"),
    };
    taken.insert(name.clone());
    name
}

fn resolve(base: &Path, path: &str) -> PathBuf {
    let path = Path::new(path.trim());
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// `out` paths compared without `.` segments, so `./a.de` and `a.de` collide.
fn normalized(path: &Path) -> PathBuf {
    path.components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect()
}

fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "agent_tasks_tests.rs"]
mod tests;
