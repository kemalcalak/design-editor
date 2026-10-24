//! Turning a finished job into document writes.
//!
//! A landing is computed against the document as it is when the host drains
//! it, not as it was when the job started: the client may have moved,
//! restyled or deleted the target meanwhile. Fill-type jobs are found by
//! their unique pending url, wherever it now sits (pen.dev rewrites the
//! placeholder url "in place"), so a url the client copied onto several
//! nodes resolves on all of them.

use de_editor_core::{EditorCommand, EditorState, NodeId, PenNodeExt as _};
use de_image_enrich::SEARCH_FAILED_PLACEHOLDER_SRC;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{ImageFillBody, ImageFillMode, PenFill};

/// The fill list a node paints, for the node kinds a generated image can
/// land on as a fill.
pub(crate) fn node_fills(node: &PenNode) -> Option<&Vec<PenFill>> {
    match node {
        PenNode::Frame(n) => n.container.fill.as_ref(),
        PenNode::Group(n) => n.container.fill.as_ref(),
        PenNode::Rectangle(n) => n.container.fill.as_ref(),
        PenNode::Ellipse(n) => n.fill.as_ref(),
        PenNode::Polygon(n) => n.fill.as_ref(),
        PenNode::Path(n) => n.fill.as_ref(),
        _ => None,
    }
}

/// Whether a node can take an image fill (or, for an image node, a `src`).
pub(crate) fn accepts_image(node: &PenNode) -> bool {
    matches!(
        node,
        PenNode::Frame(_)
            | PenNode::Rectangle(_)
            | PenNode::Ellipse(_)
            | PenNode::Polygon(_)
            | PenNode::Path(_)
            | PenNode::Image(_)
    )
}

/// A fresh crop-mode image fill pointing at `url`.
pub(crate) fn image_fill(url: &str) -> PenFill {
    PenFill::Image(ImageFillBody {
        url: url.into(),
        mode: Some(ImageFillMode::Crop),
        original_size: None,
        transform: None,
        tile_scale: None,
        explain: None,
        opacity: None,
        blend_mode: None,
        exposure: None,
        contrast: None,
        saturation: None,
        temperature: None,
        tint: None,
        highlights: None,
        shadows: None,
    })
}

/// The patch that puts `url` on `node` at call time: an image node's `src`;
/// otherwise the node's last image fill keeps its mode and crop and takes the
/// url, and a node without one gets the image as its fill.
pub(crate) fn pending_patch(node: &PenNode, url: &str) -> Option<serde_json::Value> {
    if let PenNode::Image(_) = node {
        return Some(serde_json::json!({ "src": url }));
    }
    if !accepts_image(node) {
        return None;
    }
    let mut fills = node_fills(node).cloned().unwrap_or_default();
    match fills
        .iter_mut()
        .rev()
        .find(|fill| matches!(fill, PenFill::Image(_)))
    {
        Some(PenFill::Image(body)) => body.url = url.into(),
        _ => fills = vec![image_fill(url)],
    }
    Some(serde_json::json!({ "fill": serde_json::to_value(fills).ok()? }))
}

/// Every write that resolves `pending` to `outcome` (`Ok(url)` = the landed
/// image; `Err(())` = the job failed), across all pages. Empty when the
/// pending url is nowhere in the document (yet).
pub(crate) fn resolve_pending_url(
    state: &EditorState,
    pending: &str,
    outcome: Result<&str, ()>,
) -> Vec<EditorCommand> {
    let mut out = Vec::new();
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => {
            for page in pages {
                walk(&page.children, Some(&page.id), pending, outcome, &mut out);
            }
        }
        _ => walk(&state.doc.children, None, pending, outcome, &mut out),
    }
    out
}

fn walk(
    nodes: &[PenNode],
    page_id: Option<&String>,
    pending: &str,
    outcome: Result<&str, ()>,
    out: &mut Vec<EditorCommand>,
) {
    for node in nodes {
        if let Some(patch) = resolve_on_node(node, pending, outcome) {
            out.push(EditorCommand::PatchNodeData {
                node_id: NodeId::new(node.id_str()),
                patch_json: patch.to_string(),
                page_id: page_id.cloned(),
            });
        }
        if let Some(children) = node.children() {
            walk(children, page_id, pending, outcome, out);
        }
    }
}

fn resolve_on_node(
    node: &PenNode,
    pending: &str,
    outcome: Result<&str, ()>,
) -> Option<serde_json::Value> {
    if let PenNode::Image(image) = node {
        if image.src.as_str() != pending {
            return None;
        }
        let src = outcome.unwrap_or(SEARCH_FAILED_PLACEHOLDER_SRC);
        return Some(serde_json::json!({ "src": src }));
    }
    let fills = node_fills(node)?;
    let holds =
        |fill: &PenFill| matches!(fill, PenFill::Image(body) if body.url.as_str() == pending);
    if !fills.iter().any(holds) {
        return None;
    }
    let next: Vec<PenFill> = match outcome {
        Ok(url) => fills
            .iter()
            .cloned()
            .map(|mut fill| {
                if let PenFill::Image(body) = &mut fill {
                    if body.url.as_str() == pending {
                        body.url = url.into();
                    }
                }
                fill
            })
            .collect(),
        // pen.dev's failure signal: the pending fill disappears.
        Err(()) => fills.iter().filter(|fill| !holds(fill)).cloned().collect(),
    };
    Some(serde_json::json!({ "fill": serde_json::to_value(next).ok()? }))
}
