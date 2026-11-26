//! `de agent --tasks <file>`: the tasks file.
//!
//! ```yaml
//! defaults:            # optional; every task inherits these
//!   export: [png, pdf] # or a single name: png
//!   exportDir: shots   # export root (default: <out folder>/exports); each
//!                      # task writes into <exportDir>/<task name>/
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
//! never silently drops a setting. A task's own value wins over the
//! command-line flags, which win over the file's `defaults` (so a CI job can
//! override a checked-in file without editing it). Task names must be
//! portable folder names and are compared ignoring case; two tasks may not
//! write the same `out` (compared after `.` / `..` and, on macOS and
//! Windows, case) or export into overlapping folders.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::agent_paths::{is_portable_name, is_within, path_key};
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
    // Command-line flags beat the file's defaults; a task's own keys beat both.
    let defaults = cli.clone().or(&file.defaults.into_options(base));
    let explicit: BTreeSet<String> = file
        .tasks
        .iter()
        .filter_map(|task| non_blank(task.name.clone()))
        .map(|name| name.to_lowercase())
        .collect();
    let mut names = BTreeSet::new();
    let mut specs = Vec::with_capacity(file.tasks.len());
    for (offset, task) in file.tasks.into_iter().enumerate() {
        let index = offset + 1;
        let explicit_name = non_blank(task.name.clone());
        let output = task.out.as_deref().map(|out| resolve(base, out));
        let name = match &explicit_name {
            Some(name) => {
                if !names.insert(name.to_lowercase()) {
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
        specs.push(spec);
    }
    check_plan(&specs)?;
    Ok(specs)
}

/// An unnamed task is called after its `out` file, or `task-<n>` when that
/// name is taken or cannot be a folder name.
fn default_name(
    output: Option<&Path>,
    index: usize,
    explicit: &BTreeSet<String>,
    taken: &mut BTreeSet<String>,
) -> String {
    let candidate = output.map(file_stem).filter(|stem| {
        let key = stem.to_lowercase();
        is_portable_name(stem) && !explicit.contains(&key) && !taken.contains(&key)
    });
    let name = candidate.unwrap_or_else(|| format!("task-{index}"));
    taken.insert(name.to_lowercase());
    name
}

/// Checks across tasks, before anything runs: every `out` is its own file,
/// and no task exports into another task's export folder, a document, or a
/// document's asset folder. Paths are compared as [`path_key`]s.
pub(crate) fn check_plan(specs: &[TaskSpec]) -> Result<(), TasksFileError> {
    let mut outs: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for (offset, spec) in specs.iter().enumerate() {
        if let Some(first) = outs.insert(path_key(&spec.output), offset + 1) {
            return Err(TasksFileError::DuplicateOut {
                out: spec.output.display().to_string(),
                first,
                second: offset + 1,
            });
        }
    }
    let exporting: Vec<(usize, &TaskSpec, PathBuf)> = specs
        .iter()
        .enumerate()
        .filter(|(_, spec)| !spec.exports.is_empty())
        .map(|(offset, spec)| (offset + 1, spec, path_key(&spec.export_dir)))
        .collect();
    for (i, (first, _, dir)) in exporting.iter().enumerate() {
        for (second, spec, other) in &exporting[i + 1..] {
            if is_within(dir, other) || is_within(other, dir) {
                return Err(TasksFileError::ExportDirClash {
                    first: *first,
                    second: *second,
                    dir: spec.export_dir.display().to_string(),
                });
            }
        }
    }
    for (index, spec, dir) in &exporting {
        for owner in specs {
            let document = path_key(&owner.output);
            let assets = de_pen_loader::asset_folder::folder_path(&owner.output)
                .map(|folder| path_key(&folder));
            let overlaps_assets = assets.as_ref().is_some_and(|assets| is_within(dir, assets));
            if dir == &document || overlaps_assets {
                return Err(TasksFileError::ExportDirOverDocument {
                    index: *index,
                    dir: spec.export_dir.display().to_string(),
                    out: owner.output.display().to_string(),
                });
            }
        }
    }
    Ok(())
}

fn resolve(base: &Path, path: &str) -> PathBuf {
    let path = Path::new(path.trim());
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "agent_tasks_tests.rs"]
mod tests;
