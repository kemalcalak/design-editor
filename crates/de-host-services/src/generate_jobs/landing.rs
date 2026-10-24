//! Turning a finished job into document writes.
//!
//! A landing is computed against the document as it is when the host drains
//! it, not as it was when the job started: the client may have moved,
//! restyled or deleted the target meanwhile. Fill-type jobs are found by
//! their unique pending url, wherever it now sits (pen.dev rewrites the
//! placeholder url "in place"), so a url the client copied onto several
//! nodes resolves on all of them.

use de_editor_core::fills::node_fills;
use de_editor_core::{EditorCommand, EditorState, NodeId, PenNodeExt as _};
use de_image_enrich::SEARCH_FAILED_PLACEHOLDER_SRC;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{ImageFillBody, ImageFillMode, PenFill};

/// A node anywhere in the document, with the id of the page holding it.
pub(crate) fn find_node_any_page<'a>(
    state: &'a EditorState,
    node_id: &str,
) -> Option<(&'a PenNode, Option<String>)> {
    let id = NodeId::new(node_id);
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages.iter().find_map(|page| {
            de_editor_core::walkers::find_node(&page.children, &id)
                .map(|node| (node, Some(page.id.clone())))
        }),
        _ => de_editor_core::walkers::find_node(&state.doc.children, &id).map(|node| (node, None)),
    }
}

/// Whether `url` is an image fill url or image `src` somewhere in the
/// document: transforms only take images the document already has.
pub(crate) fn document_has_image_url(state: &EditorState, url: &str) -> bool {
    fn walk(nodes: &[PenNode], url: &str) -> bool {
        nodes.iter().any(|node| {
            let here = match node {
                PenNode::Image(image) => image.src.as_str() == url,
                _ => node_fills(node).is_some_and(|fills| {
                    fills.iter().any(
                        |fill| matches!(fill, PenFill::Image(body) if body.url.as_str() == url),
                    )
                }),
            };
            here || node.children().is_some_and(|children| walk(children, url))
        })
    }
    match state.doc.pages.as_ref() {
        Some(pages) if !pages.is_empty() => pages.iter().any(|page| walk(&page.children, url)),
        _ => walk(&state.doc.children, url),
    }
}

/// The image a node shows: an image node's `src`, else its last image fill
/// (fills paint in list order, so the last one is on top).
pub(crate) fn node_image_url(node: &PenNode) -> Option<String> {
    if let PenNode::Image(image) = node {
        return Some(image.src.as_str().to_string());
    }
    node_fills(node)?.iter().rev().find_map(|fill| match fill {
        PenFill::Image(body) if !body.url.as_str().trim().is_empty() => {
            Some(body.url.as_str().to_string())
        }
        _ => None,
    })
}

/// Whether `node` shows `url` (an image fill or an image node's `src`).
pub(crate) fn node_shows_url(node: &PenNode, url: &str) -> bool {
    match node {
        PenNode::Image(image) => image.src.as_str() == url,
        _ => node_fills(node).is_some_and(|fills| {
            fills
                .iter()
                .any(|fill| matches!(fill, PenFill::Image(body) if body.url.as_str() == url))
        }),
    }
}

/// The patch that makes `node` show `pending` in place of `source`: the
/// image fill (or `src`) holding `source` takes the pending url, other fills
/// stay; a node not showing `source` gets [`pending_patch`] semantics.
pub(crate) fn replace_image_patch(
    node: &PenNode,
    source: &str,
    pending: &str,
) -> Option<serde_json::Value> {
    if let PenNode::Image(image) = node {
        if image.src.as_str() == source {
            return Some(serde_json::json!({ "src": pending }));
        }
    }
    if let Some(fills) = node_fills(node) {
        if fills
            .iter()
            .any(|fill| matches!(fill, PenFill::Image(body) if body.url.as_str() == source))
        {
            let next: Vec<PenFill> = fills
                .iter()
                .cloned()
                .map(|mut fill| {
                    if let PenFill::Image(body) = &mut fill {
                        if body.url.as_str() == source {
                            body.url = pending.into();
                        }
                    }
                    fill
                })
                .collect();
            return Some(serde_json::json!({ "fill": serde_json::to_value(next).ok()? }));
        }
    }
    pending_patch(node, pending)
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
/// url, and a node without one gets the image stacked on top of its fills
/// (so a failure, which removes the pending fill, leaves the node as it was).
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
        _ => fills.push(image_fill(url)),
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

/// The writes that land SVG text (`Ok`) or a failure (`Err`) in the frame
/// `frame_id`: the drawing fitted to the frame's size as its children, then
/// `placeholder` cleared, as one undo step. Empty while the frame is not
/// (yet) a `placeholder` frame in this document, so a same-id frame in
/// another document or one the client already finished is never drawn into.
/// A failure, or SVG that yields no shapes, only clears the flag: pen.dev's
/// "placeholder cleared on a frame that still has no children".
pub(crate) fn resolve_frame(
    state: &EditorState,
    frame_id: &str,
    outcome: Result<&str, ()>,
) -> Vec<EditorCommand> {
    let Some((node, page_id)) = find_node_any_page(state, frame_id) else {
        return Vec::new();
    };
    if !matches!(node, PenNode::Frame(frame) if frame.placeholder == Some(true)) {
        return Vec::new();
    }
    let nodes = match (outcome, node.width_px(), node.height_px()) {
        (Ok(svg), Some(width), Some(height)) => {
            de_editor_core::svg_import::svg_nodes_fitted(svg, width, height)
        }
        _ => Vec::new(),
    };
    let mut commands = Vec::new();
    if !nodes.is_empty() {
        commands.push(EditorCommand::InsertSubtree {
            nodes,
            parent_id: NodeId::new(frame_id),
            page_id: page_id.clone(),
        });
    }
    commands.push(EditorCommand::PatchNodeData {
        node_id: NodeId::new(frame_id),
        patch_json: serde_json::json!({ "placeholder": null }).to_string(),
        page_id,
    });
    vec![EditorCommand::Batch { commands }]
}
