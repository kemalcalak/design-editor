//! MCP `generate` (pen.dev `Generate`) and `get_generation_status`.
//!
//! This file holds the fill types (`ai`, `stock`); the frame and background
//! types are in `generate_tool_transforms.rs`.
//!
//! `generate` is asynchronous like pen.dev's: the call validates, reserves a
//! job, writes the job's `de:pending-image-…` url as the target's image fill
//! (one `PatchNodeData` the host applies as usual) and returns at once. The
//! provider runs on a worker thread with the user's own key, and the host
//! lands the result later (see `crate::generate_jobs`). The client checks in
//! a later call: `get_generation_status` reports the job, and the fill url is
//! rewritten to the image, or disappears when the job failed (an image node's
//! `src` becomes the failed-image placeholder instead).

use std::collections::BTreeMap;
use std::sync::Arc;

use de_editor_core::{EditorCommand, EditorState, NodeId, PenNodeExt as _};
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};

use crate::generate_jobs::{
    landing, resolve_image_profile, GenerateJobError, GenerateJobs, GenerateKind, ImageJobRequest,
    JobStatus, JobWork, StockJobRequest, PENDING_IMAGE_PREFIX,
};

/// Longest prompt or query accepted.
const MAX_PROMPT_CHARS: usize = 4000;

/// The `generate` tool over a snapshot of the document.
pub struct GenerateTool {
    pub(super) state: EditorState,
    pub(super) jobs: Arc<GenerateJobs>,
}

pub fn generate_snapshot(doc: &EditorState) -> GenerateTool {
    GenerateTool {
        state: doc.clone(),
        jobs: Arc::clone(GenerateJobs::global()),
    }
}

#[cfg(test)]
impl GenerateTool {
    pub(crate) fn for_test(doc: &EditorState, jobs: Arc<GenerateJobs>) -> Self {
        Self {
            state: doc.clone(),
            jobs,
        }
    }
}

pub(super) fn arg<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| args.get(*key))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

pub(super) fn invalid(message: impl Into<String>) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message.into())
}

pub(crate) use crate::generate_jobs::landing::find_node_any_page;

impl McpTool for GenerateTool {
    fn name(&self) -> &str {
        "generate"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let kind = match arg(args, &["type"]) {
            Some(raw) => match GenerateKind::parse(raw) {
                Some(kind) => kind,
                None => {
                    let known: Vec<&str> = GenerateKind::ALL.iter().map(|k| k.as_str()).collect();
                    return invalid(format!(
                        "unknown generate type {raw:?}: expected one of {}",
                        known.join(", ")
                    ));
                }
            },
            None => {
                return ToolOutcome::Err(ToolErrorCode::MissingArgument, "type is required".into())
            }
        };
        match kind {
            GenerateKind::Svg | GenerateKind::VectorizeImage => {
                return super::generate_tool_transforms::call_frame(self, kind, args);
            }
            GenerateKind::RemoveBackground | GenerateKind::ReplaceBackground => {
                return super::generate_tool_transforms::call_background(self, kind, args);
            }
            GenerateKind::Ai | GenerateKind::Stock => {}
        }
        let Some(node_id) = arg(args, &["nodeId", "node_id"]) else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "nodeId is required".into());
        };
        let input_keys: &[&str] = if kind == GenerateKind::Stock {
            &["query", "prompt"]
        } else {
            &["prompt"]
        };
        let Some(input) = arg(args, input_keys) else {
            return ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                format!("{} is required", input_keys[0]),
            );
        };
        if input.chars().count() > MAX_PROMPT_CHARS {
            return invalid(format!(
                "{} exceeds {MAX_PROMPT_CHARS} characters",
                input_keys[0]
            ));
        }
        let Some((node, page_id)) = find_node_any_page(&self.state, node_id) else {
            return invalid(format!("node not found: {node_id}"));
        };
        if !landing::accepts_image(node) {
            return invalid(format!(
                "node {node_id} cannot take an image fill: insert a frame or rectangle first"
            ));
        }
        let work = if kind == GenerateKind::Ai {
            let profile = match resolve_image_profile(&self.state.editor_ui.agent_settings) {
                Ok(profile) => profile,
                Err(error) => {
                    return ToolOutcome::Err(ToolErrorCode::ToolFailed, error.to_string())
                }
            };
            JobWork::Image(ImageJobRequest {
                prompt: input.to_string(),
                width: node.width_px(),
                height: node.height_px(),
                profile,
                timeout: None,
            })
        } else {
            JobWork::Stock(StockJobRequest {
                query: input.to_string(),
                credentials: super::enrich_images_tool::openverse_credentials(&self.state),
            })
        };
        let ticket = match self.jobs.reserve(kind, Some(node_id)) {
            Ok(ticket) => ticket,
            Err(error) => return ToolOutcome::Err(ToolErrorCode::ToolFailed, error.to_string()),
        };
        let Some(patch) = landing::pending_patch(node, &ticket.pending_url) else {
            // Unreachable after `accepts_image` short of a serde failure;
            // release the slot without waking any host.
            self.jobs
                .record_result(&ticket, Err(GenerateJobError::Panicked));
            return ToolOutcome::Err(
                ToolErrorCode::Internal,
                format!("node {node_id} could not be serialized"),
            );
        };
        let json = serde_json::json!({
            "jobId": ticket.id,
            "type": kind.as_str(),
            "nodeId": node_id,
            "url": ticket.pending_url,
            "status": "pending",
            "note": format!(
                "The image lands after this call returns. Check in a later call with \
                 get_generation_status; the node's fill url stays {PENDING_IMAGE_PREFIX}… until \
                 the image arrives and the pending fill disappears if the generation failed."
            ),
        })
        .to_string();
        self.jobs.spawn(ticket, work);
        ToolOutcome::OkJsonWithCommand(
            json,
            EditorCommand::PatchNodeData {
                node_id: NodeId::new(node_id),
                patch_json: patch.to_string(),
                page_id,
            },
        )
    }
}

/// `get_generation_status`: the jobs this process started, or one of them.
pub struct GenerationStatusTool {
    jobs: Arc<GenerateJobs>,
}

pub fn generation_status_snapshot() -> GenerationStatusTool {
    GenerationStatusTool {
        jobs: Arc::clone(GenerateJobs::global()),
    }
}

#[cfg(test)]
impl GenerationStatusTool {
    pub(crate) fn for_test(jobs: Arc<GenerateJobs>) -> Self {
        Self { jobs }
    }
}

impl McpTool for GenerationStatusTool {
    fn name(&self) -> &str {
        "get_generation_status"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let id = arg(args, &["jobId", "job_id"]);
        let records = self.jobs.records(id);
        if let (Some(id), true) = (id, records.is_empty()) {
            return invalid(format!("unknown job id: {id}"));
        }
        let jobs: Vec<serde_json::Value> = records
            .iter()
            .map(|record| {
                let (status, error) = match &record.status {
                    JobStatus::Pending => ("pending", None),
                    JobStatus::Done => ("done", None),
                    JobStatus::Failed(error) => ("failed", Some(error.as_str())),
                };
                serde_json::json!({
                    "jobId": record.id,
                    "type": record.kind.as_str(),
                    "nodeId": record.node_id,
                    "url": record.pending_url,
                    "status": status,
                    "error": error,
                })
            })
            .collect();
        ToolOutcome::OkJson(
            serde_json::json!({ "running": self.jobs.any_running(), "jobs": jobs }).to_string(),
        )
    }
}

/// File-backed servers load only the document; their image providers and
/// Openverse credentials come from the user's saved settings, as in the app.
pub(crate) fn load_user_provider_settings(state: &mut EditorState) {
    let mut saved = EditorState::new();
    crate::settings_io::load(&mut saved);
    state.editor_ui.agent_settings = saved.editor_ui.agent_settings;
}

/// File-backed servers: land finished generations on `state` and save, so a
/// result arrives on disk at the latest with the next request.
pub(crate) fn land_generations_and_save(state: &mut EditorState, path: &std::path::Path) {
    let landed = crate::generate_jobs::land_into(GenerateJobs::global(), state, |state, cmd| {
        state.apply(cmd)
    });
    if landed {
        if let Err(error) = super::save_editor_state(state, path) {
            eprintln!("design-editor mcp: save after generation failed: {error}");
        }
    }
}

/// Before a file-backed session ends: give still-running generations up to
/// `wait` to finish, then land them, so a paid result is not thrown away.
pub(crate) fn wait_and_land_generations(
    state: &mut EditorState,
    path: &std::path::Path,
    wait: std::time::Duration,
) {
    let jobs = GenerateJobs::global();
    if jobs.any_running() {
        eprintln!("design-editor mcp: waiting for running generations to finish");
        jobs.wait_idle(wait);
    }
    land_generations_and_save(state, path);
}
