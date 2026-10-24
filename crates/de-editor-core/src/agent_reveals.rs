//! Reveal bookkeeping for freshly inserted subtrees.
//!
//! Single-sources the before/after node-id diff plus the staggered
//! reveal-registration walk that de-orchestrator (`subagent.rs`) and
//! de-host-services (`design_agent_tools.rs` / `mcp_live.rs`) previously
//! kept as byte-identical private copies. Timing constants and the reveal
//! store live next door in [`crate::agent_indicators`].

use std::collections::HashSet;

use jian_ops_schema::node::{ContainerProps, PenNode, TextContent};

use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;

/// Every node id in the active page's tree (recursive).
pub fn collect_active_node_ids(state: &EditorState) -> HashSet<String> {
    let mut out = HashSet::new();
    for node in state.active_children() {
        collect_node_ids(node, &mut out);
    }
    out
}

/// Insert `node`'s id and all descendant ids into `out`.
pub fn collect_node_ids(node: &PenNode, out: &mut HashSet<String>) {
    out.insert(node.id_str().to_string());
    if let Some(children) = node.children() {
        for child in children {
            collect_node_ids(child, out);
        }
    }
}

/// Register staggered reveals for every node that is present now but was
/// absent from `ids_before` (see [`crate::agent_indicators::add_reveal`]).
/// No-op when `indicator_epoch` is `None`.
pub fn register_new_node_reveals(
    ids_before: &HashSet<String>,
    state: &EditorState,
    indicator_epoch: Option<u64>,
    reveal_started_ms: u64,
) {
    let Some(epoch) = indicator_epoch else {
        return;
    };
    let count: u64 = state
        .active_children()
        .iter()
        .map(|node| count_new_reveals(node, ids_before, 0))
        .sum();
    let mut stream = RevealStream {
        index: 0,
        next_start_ms: reveal_started_ms,
        stagger_ms: reveal_stagger_ms(count),
    };
    for node in state.active_children() {
        register_node_reveals(
            node,
            ids_before,
            epoch,
            reveal_started_ms,
            0,
            None,
            &mut stream,
        );
    }
}

/// Longest reveal queue one applied batch may take (#17). Each new visual
/// node gets [`crate::agent_indicators::REVEAL_STAGGER_MS`] until the batch
/// would run past this; a larger batch shares the window (never closer than
/// one frame), so a 300-node design no longer keeps the canvas repainting at
/// 60 fps for most of a minute.
pub const REVEAL_QUEUE_CAP_MS: u64 = 6_000;

/// The per-node reveal beat for a batch of `count` new visual nodes.
pub fn reveal_stagger_ms(count: u64) -> u64 {
    let full = crate::agent_indicators::REVEAL_STAGGER_MS;
    let capped = REVEAL_QUEUE_CAP_MS / count.max(1);
    capped
        .min(full)
        .max(crate::agent_indicators::REVEAL_FRAME_MS)
}

/// The beat a burst-recovery reschedule keeps: the queue's own spacing
/// (smallest positive gap between its starts), clamped to the regular
/// beat, so recovering a capped batch does not stretch it back out.
pub(crate) fn recovery_stagger_ms(ordered_starts: impl Iterator<Item = u64>) -> u64 {
    let mut previous = None;
    let mut gap = crate::agent_indicators::REVEAL_BURST_RECOVERY_STAGGER_MS;
    for start in ordered_starts {
        if let Some(prev) = previous {
            if start > prev {
                gap = gap.min(start - prev);
            }
        }
        previous = Some(start);
    }
    gap.max(crate::agent_indicators::REVEAL_FRAME_MS)
}

/// How many nodes under `node` (inclusive) [`register_node_reveals`] would
/// give a slot to.
fn count_new_reveals(node: &PenNode, ids_before: &HashSet<String>, depth: u64) -> u64 {
    let own = u64::from(!ids_before.contains(node.id_str()) && should_reveal_node(node, depth));
    own + node.children().map_or(0, |children| {
        children
            .iter()
            .map(|child| count_new_reveals(child, ids_before, depth + 1))
            .sum()
    })
}

struct RevealStream {
    index: u64,
    next_start_ms: u64,
    stagger_ms: u64,
}

fn register_node_reveals(
    node: &PenNode,
    ids_before: &HashSet<String>,
    epoch: u64,
    reveal_started_ms: u64,
    depth: u64,
    parent_reveal_start_ms: Option<u64>,
    stream: &mut RevealStream,
) {
    let id = node.id_str();
    let mut own_reveal_start_ms = parent_reveal_start_ms;
    if !ids_before.contains(id) && should_reveal_node(node, depth) {
        let own_stream_index = stream.index;
        stream.index += 1;
        let base_start = reveal_started_ms
            + depth.saturating_mul(crate::agent_indicators::REVEAL_DEPTH_STAGGER_MS)
            + own_stream_index.saturating_mul(stream.stagger_ms);
        let child_runway_start = parent_reveal_start_ms
            .map(|started_at| {
                started_at.saturating_add(crate::agent_indicators::REVEAL_CHILD_RUNWAY_MS)
            })
            .unwrap_or(reveal_started_ms);
        let started_at = base_start.max(child_runway_start).max(stream.next_start_ms);
        crate::agent_indicators::add_reveal(epoch, id, started_at);
        stream.next_start_ms = started_at.saturating_add(stream.stagger_ms);
        own_reveal_start_ms = Some(started_at);
    }
    if let Some(children) = node.children() {
        for child in children {
            register_node_reveals(
                child,
                ids_before,
                epoch,
                reveal_started_ms,
                depth + 1,
                own_reveal_start_ms,
                stream,
            );
        }
    }
}

fn should_reveal_node(node: &PenNode, depth: u64) -> bool {
    depth == 0 || node_has_own_visual(node) || node_is_named_structure(node)
}

fn node_has_own_visual(node: &PenNode) -> bool {
    match node {
        PenNode::Frame(n) => {
            container_has_own_visual(&n.container) || n.image_search_query.is_some()
        }
        PenNode::Group(n) => container_has_own_visual(&n.container),
        PenNode::Rectangle(n) => container_has_own_visual(&n.container),
        PenNode::Ref(_) => false,
        PenNode::Text(n) => match &n.content {
            TextContent::Plain(s) => !s.is_empty(),
            TextContent::Styled(segments) => !segments.is_empty(),
        },
        _ => true,
    }
}

fn container_has_own_visual(container: &ContainerProps) -> bool {
    container
        .fill
        .as_ref()
        .is_some_and(|fills| !fills.is_empty())
        || container.stroke.is_some()
        || container
            .effects
            .as_ref()
            .is_some_and(|effects| !effects.is_empty())
}

fn node_is_named_structure(node: &PenNode) -> bool {
    if !node.is_container() {
        return false;
    }
    let base = node.base();
    base.role.as_deref().is_some_and(|role| !role.is_empty())
        || base.name.as_deref().is_some_and(|name| !name.is_empty())
}

#[cfg(test)]
#[path = "agent_reveals_tests.rs"]
mod tests;
