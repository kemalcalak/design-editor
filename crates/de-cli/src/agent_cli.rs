//! `de agent`: the in-app design agent, headless (#397).
//!
//! The engine is `de_host_services::design_agent_run`, the one the MCP
//! `run_design_agent` tool runs, so a CLI run, an MCP run and the app's own
//! generation share prompt, tools, repairs and finalize. This module wires
//! the command line to it: parse ([`crate::agent_args`]), pick providers
//! ([`crate::agent_provider`]), run ([`crate::agent_runner`]), report
//! ([`crate::agent_report`]). See `agent_usage.txt` for the user-facing
//! contract.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::json;

use crate::agent_args::{parse_agent_args, AgentArgs, AgentMode};
use crate::agent_progress::Progress;
use crate::agent_provider::{process_env, ProviderResolver, ENV_API_KEY};
use crate::agent_report::{json_report, text_report};
use crate::agent_runner::{run_tasks, PlannedTask, TaskStatus};
use crate::agent_task::TaskSpec;

pub(crate) const AGENT_USAGE: &str = include_str!("agent_usage.txt");

/// Every task succeeded.
pub(crate) const EXIT_OK: i32 = 0;
/// At least one task failed.
pub(crate) const EXIT_FAILED: i32 = 1;
/// Bad arguments, tasks file or provider setup; nothing ran.
pub(crate) const EXIT_USAGE: i32 = 2;
/// Cancelled by Ctrl+C (128 + SIGINT, the shell convention).
pub(crate) const EXIT_CANCELLED: i32 = 130;

/// Run `de agent <args>` and return the process exit status.
pub(crate) fn run(args: &[String]) -> i32 {
    let parsed = match parse_agent_args(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("de agent: {error}");
            return EXIT_USAGE;
        }
    };
    let resolver = ProviderResolver::new(parsed.provider.clone(), process_env());
    match &parsed.mode {
        AgentMode::Help => {
            print!("{AGENT_USAGE}");
            EXIT_OK
        }
        AgentMode::ListProviders => list_providers(&parsed, &resolver),
        AgentMode::Run(tasks) => run_mode(&parsed, &resolver, tasks.clone()),
    }
}

fn run_mode(args: &AgentArgs, resolver: &ProviderResolver, tasks: Vec<TaskSpec>) -> i32 {
    // Every task's provider is settled before the first one starts, so a
    // typo in task 9 does not surface after eight paid runs.
    let mut planned = Vec::with_capacity(tasks.len());
    for spec in tasks {
        match resolver.resolve(spec.provider.as_deref(), spec.model.as_deref()) {
            Ok(provider) => planned.push(PlannedTask { spec, provider }),
            Err(error) => {
                eprintln!("de agent: task {}: {error}", spec.name);
                return EXIT_USAGE;
            }
        }
    }
    let progress = Progress::new(args.quiet);
    let cancel = Arc::new(AtomicBool::new(false));
    if let Err(error) = install_cancel_handler(&cancel) {
        progress.note(&format!("Ctrl+C will stop the process abruptly: {error}"));
    }
    if planned.len() > 1 {
        progress.note(&format!(
            "{} tasks, {} at a time",
            planned.len(),
            args.parallel.min(planned.len())
        ));
    }
    let started = Instant::now();
    let results = run_tasks(planned, args.parallel, &cancel, &progress);
    let elapsed = started.elapsed();
    let cancelled = cancel.load(Ordering::Acquire);
    if args.json {
        let report = json_report(&results, elapsed, cancelled);
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| report.to_string())
        );
    } else {
        println!("{}", text_report(&results, elapsed));
    }
    if cancelled {
        EXIT_CANCELLED
    } else if results.iter().any(|result| result.status != TaskStatus::Ok) {
        EXIT_FAILED
    } else {
        EXIT_OK
    }
}

/// First Ctrl+C (or SIGTERM) raises `cancel`: running tasks stop and write
/// nothing. A second one exits at once with 130.
fn install_cancel_handler(cancel: &Arc<AtomicBool>) -> std::io::Result<()> {
    #[cfg(unix)]
    let signals = [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM];
    #[cfg(not(unix))]
    let signals = [signal_hook::consts::SIGINT];
    for signal in signals {
        // Registered first so it sees the flag before this signal sets it.
        signal_hook::flag::register_conditional_shutdown(signal, EXIT_CANCELLED, cancel.clone())?;
        signal_hook::flag::register(signal, cancel.clone())?;
    }
    Ok(())
}

fn list_providers(args: &AgentArgs, resolver: &ProviderResolver) -> i32 {
    let (settings, agents) = match resolver.saved() {
        Ok(saved) => saved,
        Err(error) => {
            eprintln!("de agent: {error}");
            return EXIT_USAGE;
        }
    };
    let env_key = (process_env())(ENV_API_KEY).is_some();
    if args.json {
        let providers: Vec<_> = agents
            .iter()
            .map(|agent| {
                json!({
                    "id": agent.id,
                    "name": agent.display_name,
                    "preset": agent.preset.as_str(),
                    "apiFormat": api_format(agent.kind),
                    "baseUrl": agent.base_url,
                    "models": agent.models,
                    "enabled": agent.enabled,
                    "hasKey": !agent.api_key.trim().is_empty(),
                    "ready": agent.ready(),
                })
            })
            .collect();
        let report = json!({
            "settings": settings,
            "environmentKey": env_key,
            "providers": providers,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| report.to_string())
        );
        return EXIT_OK;
    }
    println!("saved providers ({settings}):");
    if agents.is_empty() {
        println!("  none; add one in Design Editor > Settings > Agents");
    }
    for agent in agents {
        let state = if agent.ready() {
            "ready"
        } else if !agent.enabled {
            "disabled"
        } else if agent.api_key.trim().is_empty() {
            "no API key"
        } else {
            "no model"
        };
        println!(
            "  {}  {}  [{}, {}]  models: {}",
            agent.id,
            agent.display_name.trim(),
            api_format(agent.kind),
            state,
            if agent.models.is_empty() {
                "-".to_string()
            } else {
                agent.models.join(", ")
            }
        );
    }
    if env_key {
        println!("{ENV_API_KEY} is set: runs without --provider use it (with --model).");
    }
    EXIT_OK
}

fn api_format(kind: de_editor_core::BuiltinAgentKind) -> &'static str {
    match kind {
        de_editor_core::BuiltinAgentKind::Anthropic => "anthropic",
        de_editor_core::BuiltinAgentKind::OpenAiCompat => "openai",
    }
}
