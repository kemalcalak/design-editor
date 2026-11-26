//! `de agent`'s own argument parser. It runs before the generic `de`
//! parser, which has no short flags with values (`-p`) and treats unknown
//! flags as MCP tool arguments.

use std::io::Read;
use std::path::PathBuf;

use crate::agent_args_error::AgentArgsError;
use crate::agent_provider::ProviderFlags;
use crate::agent_task::{build_task, file_stem, RawTaskFields, TaskOptions, TaskSpec};
use crate::agent_tasks::load_tasks_file;

/// Most tasks `--parallel` runs at once.
pub(crate) const MAX_PARALLEL: u64 = 64;

/// What `de agent` was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentMode {
    Help,
    ListProviders,
    Run(Vec<TaskSpec>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentArgs {
    pub mode: AgentMode,
    pub parallel: usize,
    /// Machine-readable report on stdout.
    pub json: bool,
    /// No progress on stderr.
    pub quiet: bool,
    pub provider: ProviderFlags,
}

/// Every flag that takes a value.
const VALUE_FLAGS: &[&str] = &[
    "-p",
    "--prompt",
    "--prompt-file",
    "--in",
    "--out",
    "--export",
    "--export-dir",
    "--model",
    "--provider",
    "--api-format",
    "--base-url",
    "--api-key-env",
    "--settings",
    "--timeout",
    "--max-turns",
    "--tasks",
    "--parallel",
];

const SWITCHES: &[&str] = &[
    "--json",
    "--quiet",
    "-q",
    "--list-providers",
    "--help",
    "-h",
];

#[derive(Default)]
struct Collected {
    prompt: Option<String>,
    prompt_file: Option<String>,
    input: Option<String>,
    output: Option<String>,
    exports: Vec<String>,
    export_dir: Option<String>,
    model: Option<String>,
    provider: Option<String>,
    timeout: Option<String>,
    max_turns: Option<String>,
    tasks: Option<String>,
    parallel: Option<String>,
    json: bool,
    quiet: bool,
    list_providers: bool,
    help: bool,
    flags: ProviderFlags,
}

/// Parse the arguments after `de agent`.
pub(crate) fn parse_agent_args(args: &[String]) -> Result<AgentArgs, AgentArgsError> {
    let c = collect(args)?;
    let parallel = match &c.parallel {
        None => 1,
        Some(raw) => parse_number("--parallel", raw, 1, MAX_PARALLEL)? as usize,
    };
    let mode = if c.help {
        AgentMode::Help
    } else if c.list_providers {
        AgentMode::ListProviders
    } else {
        AgentMode::Run(tasks_from(&c)?)
    };
    if c.tasks.is_none() && c.parallel.is_some() && matches!(mode, AgentMode::Run(_)) {
        return Err(AgentArgsError::Conflict {
            flag: "--parallel".into(),
            with: "a single -p prompt (it needs --tasks)".into(),
        });
    }
    Ok(AgentArgs {
        mode,
        parallel,
        json: c.json,
        quiet: c.quiet,
        provider: c.flags,
    })
}

fn collect(args: &[String]) -> Result<Collected, AgentArgsError> {
    let mut c = Collected::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arg, None),
        };
        if SWITCHES.contains(&flag) {
            if inline.is_some() {
                return Err(AgentArgsError::UnexpectedArgument {
                    arg: arg.to_string(),
                });
            }
            match flag {
                "--json" => c.json = true,
                "--quiet" | "-q" => c.quiet = true,
                "--list-providers" => c.list_providers = true,
                _ => c.help = true,
            }
            i += 1;
            continue;
        }
        if !VALUE_FLAGS.contains(&flag) {
            return Err(if flag.starts_with('-') {
                AgentArgsError::UnknownFlag {
                    flag: flag.to_string(),
                }
            } else {
                AgentArgsError::UnexpectedArgument {
                    arg: arg.to_string(),
                }
            });
        }
        // A value flag always takes the next argument verbatim, so a prompt
        // may start with "-".
        let value = match inline {
            Some(value) => value,
            None => {
                i += 1;
                args.get(i)
                    .cloned()
                    .ok_or_else(|| AgentArgsError::MissingValue {
                        flag: flag.to_string(),
                    })?
            }
        };
        let slot = match flag {
            "-p" | "--prompt" => &mut c.prompt,
            "--prompt-file" => &mut c.prompt_file,
            "--in" => &mut c.input,
            "--out" => &mut c.output,
            "--export" => {
                c.exports.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string),
                );
                i += 1;
                continue;
            }
            "--export-dir" => &mut c.export_dir,
            "--model" => &mut c.model,
            "--provider" => &mut c.provider,
            "--api-format" => &mut c.flags.api_format,
            "--base-url" => &mut c.flags.base_url,
            "--api-key-env" => &mut c.flags.api_key_env,
            "--settings" => {
                c.flags.settings = Some(PathBuf::from(value));
                i += 1;
                continue;
            }
            "--timeout" => &mut c.timeout,
            "--max-turns" => &mut c.max_turns,
            "--tasks" => &mut c.tasks,
            _ => &mut c.parallel,
        };
        *slot = Some(value);
        i += 1;
    }
    Ok(c)
}

fn tasks_from(c: &Collected) -> Result<Vec<TaskSpec>, AgentArgsError> {
    let options = TaskOptions {
        exports: (!c.exports.is_empty()).then(|| c.exports.clone()),
        export_dir: c.export_dir.as_deref().map(PathBuf::from),
        provider: non_blank(&c.provider),
        model: non_blank(&c.model),
        timeout_secs: c
            .timeout
            .as_deref()
            .map(|raw| parse_number("--timeout", raw, 1, crate::agent_task::MAX_TIMEOUT_SECS))
            .transpose()?,
        max_turns: c
            .max_turns
            .as_deref()
            .map(|raw| parse_number("--max-turns", raw, 1, crate::agent_task::MAX_TURNS_CEILING))
            .transpose()?,
    };
    if let Some(tasks) = &c.tasks {
        for (present, flag) in [
            (c.prompt.is_some(), "-p/--prompt"),
            (c.prompt_file.is_some(), "--prompt-file"),
            (c.input.is_some(), "--in"),
            (c.output.is_some(), "--out"),
        ] {
            if present {
                return Err(AgentArgsError::Conflict {
                    flag: "--tasks".into(),
                    with: flag.into(),
                });
            }
        }
        return Ok(load_tasks_file(&PathBuf::from(tasks), &options)?);
    }
    if c.prompt.is_some() && c.prompt_file.is_some() {
        return Err(AgentArgsError::Conflict {
            flag: "-p/--prompt".into(),
            with: "--prompt-file".into(),
        });
    }
    let prompt = match &c.prompt_file {
        Some(path) => Some(read_prompt_file(path)?),
        None => c.prompt.clone(),
    };
    let output = c.output.as_deref().map(PathBuf::from);
    let name = output
        .as_deref()
        .map(file_stem)
        .unwrap_or_else(|| "design".into());
    let task = build_task(
        name,
        RawTaskFields {
            prompt,
            input: c.input.as_deref().map(PathBuf::from),
            output,
            options,
        },
    )
    .map_err(AgentArgsError::Task)?;
    Ok(vec![task])
}

fn read_prompt_file(path: &str) -> Result<String, AgentArgsError> {
    let error = |detail: String| AgentArgsError::PromptFile {
        path: path.to_string(),
        detail,
    };
    if path == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| error(e.to_string()))?;
        return Ok(text);
    }
    std::fs::read_to_string(path).map_err(|e| error(e.to_string()))
}

fn parse_number(flag: &str, raw: &str, min: u64, max: u64) -> Result<u64, AgentArgsError> {
    raw.trim()
        .parse::<u64>()
        .ok()
        .filter(|value| (min..=max).contains(value))
        .ok_or_else(|| AgentArgsError::InvalidNumber {
            flag: flag.to_string(),
            value: raw.to_string(),
            min,
            max,
        })
}

fn non_blank(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "agent_args_tests.rs"]
mod tests;
