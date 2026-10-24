//! The `Generate` half of `enrich_images` (#10): explicit `G()` slots go to
//! the user's configured image provider, a few at a time, under the run's
//! deadline; plus the per-slot report the tool returns.

use std::time::Instant;

use de_editor_core::agent_settings::ImageGenProfile;
use de_image_enrich::{ImageRequestMode, ImageSearchTarget};

use super::enrich_images_tool::ImageSearchBackend;
use crate::generate_jobs::{GenerateBackend, GenerateJobError, ImageJobRequest, ProviderBackend};

/// Generations started at once. Each is a paid provider call that takes
/// seconds to a minute, so fewer than the stock searches.
const MAX_PARALLEL_GENERATIONS: usize = 2;

/// Where a slot ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotStatus {
    Resolved,
    Failed,
    Unresolved,
}

/// One slot in the tool's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlotReport {
    pub node_id: String,
    pub mode: ImageRequestMode,
    pub status: SlotStatus,
    pub error: Option<String>,
}

impl SlotReport {
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let mode = match self.mode {
            ImageRequestMode::Generate => "generate",
            ImageRequestMode::Search => "search",
            ImageRequestMode::Auto => "auto",
        };
        let status = match self.status {
            SlotStatus::Resolved => "resolved",
            SlotStatus::Failed => "failed",
            SlotStatus::Unresolved => "unresolved",
        };
        let mut value = serde_json::json!({
            "nodeId": self.node_id,
            "mode": mode,
            "status": status,
        });
        if let Some(error) = &self.error {
            value["error"] = serde_json::Value::String(error.clone());
        }
        value
    }
}

/// Generate every target, at most [`MAX_PARALLEL_GENERATIONS`] at a time.
/// `None` = not started before the deadline.
pub(crate) fn generate_many_before(
    backend: &dyn ImageSearchBackend,
    targets: &[ImageSearchTarget],
    deadline: Instant,
) -> Vec<Option<Result<String, GenerateJobError>>> {
    let mut out = Vec::with_capacity(targets.len());
    for chunk in targets.chunks(MAX_PARALLEL_GENERATIONS) {
        if Instant::now() >= deadline {
            out.resize_with(targets.len(), || None);
            break;
        }
        let results = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|target| scope.spawn(move || backend.generate_before(target, deadline)))
                .collect();
            handles
                .into_iter()
                .map(|handle| Some(handle.join().expect("image generation worker panicked")))
                .collect::<Vec<_>>()
        });
        out.extend(results);
    }
    out
}

/// Production: one provider call for one slot. The slot's `imagePrompt`
/// wins over its search query. A call that finishes after the deadline
/// still lands — the money is spent — but no new one starts past it.
pub(crate) fn generate_slot_before(
    profile: ImageGenProfile,
    target: &ImageSearchTarget,
    deadline: Instant,
) -> Result<String, GenerateJobError> {
    if Instant::now() >= deadline {
        return Err(GenerateJobError::Failed(
            "not started before the timeout".into(),
        ));
    }
    let prompt = target
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
        .unwrap_or(&target.query);
    ProviderBackend.generate_image(&ImageJobRequest {
        prompt: prompt.to_string(),
        width: target.width,
        height: target.height,
        profile,
    })
}
