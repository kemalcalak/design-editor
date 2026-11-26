//! `de agent`'s summary: JSON on stdout with `--json`, a few readable lines
//! otherwise.

use std::path::Path;
use std::time::Duration;

use de_ai::chat_usage::{TokenUsage, UsageMeter};
use de_host_services::design_agent_run::usage_json;
use serde_json::{json, Value};

use crate::agent_runner::{ExportOutcome, TaskResult, TaskStatus};

/// Version of the JSON report's shape, bumped on breaking changes.
pub(crate) const REPORT_VERSION: u32 = 1;

fn path(path: &Path) -> String {
    de_util::display_path::display_path(path)
}

/// Milliseconds, the unit the JSON report uses.
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// `12.3s` / `2m 05s`.
pub(crate) fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let whole = duration.as_secs();
        format!("{}m {:02}s", whole / 60, whole % 60)
    }
}

/// `1200 in / 340 out tokens`, or a note when the provider sent none.
pub(crate) fn format_usage(usage: &TokenUsage) -> String {
    if !usage.reported() {
        return "tokens not reported".into();
    }
    let side = |count: Option<u64>| count.map_or_else(|| "?".to_string(), |n| n.to_string());
    format!(
        "{} in / {} out tokens",
        side(usage.input_tokens),
        side(usage.output_tokens)
    )
}

fn export_json(export: &ExportOutcome) -> Value {
    match &export.result {
        Ok(report) => json!({
            "format": export.format.as_str(),
            "ok": report.failed.is_empty(),
            "files": report.files.iter().map(|file| path(file)).collect::<Vec<_>>(),
            "failed": report
                .failed
                .iter()
                .map(|(file, error)| json!({ "file": file, "error": error }))
                .collect::<Vec<_>>(),
            "warnings": report.warnings,
        }),
        Err(error) => json!({
            "format": export.format.as_str(),
            "ok": false,
            "files": [],
            "error": error,
        }),
    }
}

fn task_json(result: &TaskResult) -> Value {
    let spec = &result.spec;
    let report = result.report.as_ref();
    json!({
        "name": spec.name,
        "status": result.status.as_str(),
        "prompt": spec.prompt,
        "in": spec.input.as_deref().map(path),
        "out": path(&spec.output),
        "saved": result.saved,
        "provider": result.provider_label,
        "model": result.model,
        "durationMs": millis(result.duration),
        "toolCalls": report.map(|r| r.tool_calls),
        "landedRoots": report.map(|r| r.landed_roots),
        "stopReason": report.and_then(|r| r.stop_reason).map(|reason| format!("{reason:?}")),
        "usage": report.map_or_else(
            || usage_json(&TokenUsage::default(), 0),
            |r| usage_json(&r.usage, r.provider_requests),
        ),
        "agent": report.map(|r| r.to_json()),
        "exportDir": (!spec.exports.is_empty()).then(|| path(&spec.export_dir)),
        "exports": result.exports.iter().map(export_json).collect::<Vec<_>>(),
        "error": result.error.as_ref().map(|(stage, message)| json!({
            "stage": stage.as_str(),
            "message": message,
        })),
    })
}

/// Counts over every task.
pub(crate) struct Totals {
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub usage: TokenUsage,
    pub requests: u64,
}

pub(crate) fn totals(results: &[TaskResult]) -> Totals {
    let meter = UsageMeter::new();
    let count = |status| results.iter().filter(|r| r.status == status).count();
    let mut requests = 0;
    for report in results.iter().filter_map(|r| r.report.as_ref()) {
        meter.record(report.usage);
        requests += report.provider_requests;
    }
    Totals {
        succeeded: count(TaskStatus::Ok),
        failed: count(TaskStatus::Failed),
        cancelled: count(TaskStatus::Cancelled),
        usage: meter.total(),
        requests,
    }
}

/// The `--json` report.
pub(crate) fn json_report(results: &[TaskResult], elapsed: Duration, cancelled: bool) -> Value {
    let totals = totals(results);
    json!({
        "version": REPORT_VERSION,
        "ok": totals.failed == 0 && totals.cancelled == 0 && !cancelled,
        "cancelled": cancelled,
        "summary": {
            "total": results.len(),
            "succeeded": totals.succeeded,
            "failed": totals.failed,
            "cancelled": totals.cancelled,
            "durationMs": millis(elapsed),
            "usage": usage_json(&totals.usage, totals.requests),
        },
        "tasks": results.iter().map(task_json).collect::<Vec<_>>(),
    })
}

/// The readable summary printed without `--json`.
pub(crate) fn text_report(results: &[TaskResult], elapsed: Duration) -> String {
    let totals = totals(results);
    let mut lines = Vec::new();
    for result in results {
        let line = match result.status {
            TaskStatus::Ok => {
                let mut line = format!(
                    "ok      {}  {}",
                    result.spec.name,
                    path(&result.spec.output)
                );
                let files: usize = result
                    .exports
                    .iter()
                    .filter_map(|export| export.result.as_ref().ok())
                    .map(|report| report.files.len())
                    .sum();
                if files > 0 {
                    line.push_str(&format!(
                        "  (+{files} exported to {})",
                        path(&result.spec.export_dir)
                    ));
                }
                line
            }
            TaskStatus::Failed => format!(
                "failed  {}  {}",
                result.spec.name,
                result
                    .error
                    .as_ref()
                    .map(|(stage, message)| format!("{}: {message}", stage.as_str()))
                    .unwrap_or_default()
            ),
            TaskStatus::Cancelled => format!("cancelled  {}", result.spec.name),
        };
        lines.push(line);
    }
    let mut summary = format!(
        "{}/{} task(s) succeeded in {}; {}",
        totals.succeeded,
        results.len(),
        format_duration(elapsed),
        format_usage(&totals.usage)
    );
    if totals.cancelled > 0 {
        summary.push_str(&format!("; {} cancelled", totals.cancelled));
    }
    lines.push(summary);
    lines.join("\n")
}
