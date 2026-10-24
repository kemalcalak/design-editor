//! `generate` types that draw into a frame (`svg`, `vectorize-image`) or
//! transform an image the document already has (`remove-background`,
//! `replace-background`), #11. Same async contract as the fill types: the
//! call does the bookkeeping and returns, the result lands later.

use std::collections::BTreeMap;

use de_editor_core::{EditorCommand, NodeId, PenNodeExt as _};
use de_mcp::{ToolErrorCode, ToolOutcome};
use jian_ops_schema::node::PenNode;

use super::generate_tool::{arg, invalid, GenerateTool, MAX_PROMPT_CHARS};
use crate::generate_jobs::image_edit::check_supported;
use crate::generate_jobs::source::{SourceImage, MIN_SOURCE_PX, MIN_TRACE_SOURCE_PX};
use crate::generate_jobs::svg_llm::resolve_chat_agent;
use crate::generate_jobs::{
    landing, resolve_image_profile, EditJobRequest, EditKind, GenerateJobError, GenerateKind,
    JobWork, SourceRef, SvgJobRequest, PENDING_IMAGE_PREFIX,
};

fn failed(error: GenerateJobError) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::ToolFailed, error.to_string())
}

fn prompt_arg(args: &BTreeMap<String, String>) -> Result<&str, Box<ToolOutcome>> {
    let Some(prompt) = arg(args, &["prompt"]) else {
        return Err(Box::new(ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            "prompt is required".into(),
        )));
    };
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(Box::new(invalid(format!(
            "prompt exceeds {MAX_PROMPT_CHARS} characters"
        ))));
    }
    Ok(prompt)
}

/// The transform's source url: `imageUrl` (must already be in the
/// document, as on pen.dev), or `sourceNodeId`, the image that node shows —
/// a Design Editor shortcut that keeps megabytes of `data:` url out of the
/// call. A data url is decoded and checked now (sides at least `min_px`),
/// so a bad source fails the call instead of the job.
fn source_arg(
    tool: &GenerateTool,
    args: &BTreeMap<String, String>,
    min_px: u32,
) -> Result<(String, SourceRef), Box<ToolOutcome>> {
    let url = match (
        arg(args, &["imageUrl", "image_url"]),
        arg(args, &["sourceNodeId", "source_node_id"]),
    ) {
        (Some(url), _) => {
            if !landing::document_has_image_url(&tool.state, url) {
                return Err(Box::new(invalid(
                    "imageUrl is not in the document: pass the url of an image fill the \
                     document already has, or sourceNodeId",
                )));
            }
            url.to_string()
        }
        (None, Some(source_id)) => {
            let Some((node, _)) = landing::find_node_any_page(&tool.state, source_id) else {
                return Err(Box::new(invalid(format!("node not found: {source_id}"))));
            };
            landing::node_image_url(node)
                .ok_or_else(|| Box::new(invalid(format!("node {source_id} shows no image"))))?
        }
        (None, None) => {
            return Err(Box::new(ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                "imageUrl or sourceNodeId is required".into(),
            )))
        }
    };
    let bad_source = |e: GenerateJobError| Box::new(invalid(e.to_string()));
    let allow_remote = !crate::generate_jobs::backend_screening();
    let source = SourceRef::from_url(&url, allow_remote).map_err(bad_source)?;
    if let SourceRef::Data(data) = &source {
        SourceImage::from_data_url(data, min_px).map_err(bad_source)?;
    }
    Ok((url, source))
}

/// `svg` and `vectorize-image`: draw into the frame `nodeId`.
pub(super) fn call_frame(
    tool: &GenerateTool,
    kind: GenerateKind,
    args: &BTreeMap<String, String>,
) -> ToolOutcome {
    let Some(frame_id) = arg(args, &["nodeId", "node_id"]) else {
        return ToolOutcome::Err(ToolErrorCode::MissingArgument, "nodeId is required".into());
    };
    let Some((node, page_id)) = landing::find_node_any_page(&tool.state, frame_id) else {
        return invalid(format!("node not found: {frame_id}"));
    };
    if !matches!(node, PenNode::Frame(_)) {
        return invalid(format!(
            "node {frame_id} is not a frame: insert a frame with a width and height first"
        ));
    }
    let (Some(width), Some(height)) = (node.width_px(), node.height_px()) else {
        return invalid(format!(
            "frame {frame_id} needs a fixed width and height for the artwork to fit"
        ));
    };
    if width <= 0.0 || height <= 0.0 {
        return invalid(format!("frame {frame_id} has no area"));
    }
    if node.is_auto_layout_container() {
        // Auto layout would re-stack the fitted paths.
        return invalid(format!(
            "frame {frame_id} uses auto layout: draw into a frame without layout"
        ));
    }
    if tool.jobs.frame_job_running(frame_id) {
        return invalid(format!(
            "frame {frame_id} is still being drawn: check it with get_generation_status"
        ));
    }
    let work = if kind == GenerateKind::Svg {
        let prompt = match prompt_arg(args) {
            Ok(prompt) => prompt,
            Err(outcome) => return *outcome,
        };
        let agent = match resolve_chat_agent(&tool.state.editor_ui.agent_settings) {
            Ok(agent) => agent,
            Err(error) => return failed(error),
        };
        JobWork::Svg(SvgJobRequest {
            prompt: prompt.to_string(),
            width,
            height,
            agent,
        })
    } else {
        match source_arg(tool, args, MIN_TRACE_SOURCE_PX) {
            Ok((_, source)) => JobWork::Vectorize(source),
            Err(outcome) => return *outcome,
        }
    };
    let ticket = match tool.jobs.reserve(kind, Some(frame_id)) {
        Ok(ticket) => ticket,
        Err(error) => return failed(error),
    };
    let json = serde_json::json!({
        "jobId": ticket.id,
        "type": kind.as_str(),
        "nodeId": frame_id,
        "status": "pending",
        "note": "The artwork lands after this call returns, as vector children scaled to the \
                 frame. The frame is marked placeholder: true until then; check in a later \
                 call with get_generation_status. Never draw into or regenerate a pending frame.",
    })
    .to_string();
    tool.jobs.spawn(ticket, work);
    ToolOutcome::OkJsonWithCommand(
        json,
        EditorCommand::PatchNodeData {
            node_id: NodeId::new(frame_id),
            patch_json: serde_json::json!({ "placeholder": true }).to_string(),
            page_id,
        },
    )
}

/// `remove-background` and `replace-background`: return a pending url for
/// the edited image; with `nodeId`, also put it on that node in place of
/// the source image.
pub(super) fn call_background(
    tool: &GenerateTool,
    kind: GenerateKind,
    args: &BTreeMap<String, String>,
) -> ToolOutcome {
    let (source_url, source) = match source_arg(tool, args, MIN_SOURCE_PX) {
        Ok(found) => found,
        Err(outcome) => return *outcome,
    };
    let edit = if kind == GenerateKind::ReplaceBackground {
        match prompt_arg(args) {
            Ok(prompt) => EditKind::ReplaceBackground(prompt.to_string()),
            Err(outcome) => return *outcome,
        }
    } else {
        EditKind::RemoveBackground
    };
    let target = arg(args, &["nodeId", "node_id"]);
    let target_node = match target {
        Some(id) => match landing::find_node_any_page(&tool.state, id) {
            Some((node, page_id)) if landing::node_shows_url(node, &source_url) => {
                Some((id, node, page_id))
            }
            Some(_) => {
                return invalid(format!(
                    "node {id} does not show the source image: pass the node that shows it, \
                     or leave nodeId out and apply the returned url yourself"
                ))
            }
            None => return invalid(format!("node not found: {id}")),
        },
        None => None,
    };
    let profile = match resolve_image_profile(&tool.state.editor_ui.agent_settings) {
        Ok(profile) => profile,
        Err(error) => return failed(error),
    };
    if let Err(error) = check_supported(&edit, &profile, crate::generate_jobs::backend_screening())
    {
        return failed(error);
    }
    let ticket = match tool.jobs.reserve_edit(kind, target, &source_url) {
        Ok(ticket) => ticket,
        Err(error) => return failed(error),
    };
    let command = match &target_node {
        Some((id, node, page_id)) => {
            let Some(patch) = landing::replace_image_patch(node, &source_url, &ticket.pending_url)
            else {
                // Unreachable short of a serde failure; release the slot
                // without waking any host (the caller may hold a host lock).
                tool.jobs
                    .record_result(&ticket, Err(GenerateJobError::Panicked));
                return ToolOutcome::Err(
                    ToolErrorCode::Internal,
                    format!("node {id} could not be serialized"),
                );
            };
            Some(EditorCommand::PatchNodeData {
                node_id: NodeId::new(*id),
                patch_json: patch.to_string(),
                page_id: page_id.clone(),
            })
        }
        None => None,
    };
    let json = serde_json::json!({
        "jobId": ticket.id,
        "type": kind.as_str(),
        "nodeId": target,
        "url": ticket.pending_url,
        "status": "pending",
        "note": format!(
            "Apply url as an image fill (update_node / batch_design), or pass nodeId to have it \
             applied. It stays {PENDING_IMAGE_PREFIX}… until the edited image arrives, is \
             rewritten in place everywhere it is used, and disappears if the edit failed; check \
             with get_generation_status in a later call."
        ),
    })
    .to_string();
    tool.jobs.spawn(
        ticket,
        JobWork::Edit(EditJobRequest {
            source,
            kind: edit,
            profile,
        }),
    );
    match command {
        Some(command) => ToolOutcome::OkJsonWithCommand(json, command),
        None => ToolOutcome::OkJson(json),
    }
}
