//! The left rail's Images tab (#30): every image node on the active page,
//! plus the Search / Generate entry points that reuse the image-node
//! popovers.
//!
//! The tab never owns a second copy of the search / generate pipeline.
//! Its buttons put an image node under the selection — the selected one
//! when it already is an image, otherwise a fresh placeholder centred in
//! the viewport — and open the property panel's popover on it, so the
//! hosts' existing pumps, inputs and result writes do the rest.

use jian_ops_schema::node::PenNode;

use crate::host_image_panel_transitions::{
    apply_image_generate_toggle, apply_image_search_toggle,
    close_other_property_popovers_for_image, selected_image_seed,
};
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, NodeId};

/// One image node, in document paint order (top of the layer list first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentImage {
    pub node_id: NodeId,
    /// Layer name, or empty when the node has none.
    pub name: String,
    /// The node's `src` as authored (data URL, remote URL or path).
    pub src: String,
}

/// Every image node on the active page, depth first.
pub fn document_images(state: &EditorState) -> Vec<DocumentImage> {
    let mut out = Vec::new();
    collect(state.active_children(), &mut out);
    out
}

fn collect(children: &[PenNode], out: &mut Vec<DocumentImage>) {
    for child in children {
        if let PenNode::Image(image) = child {
            out.push(DocumentImage {
                node_id: NodeId::new(child.id_str()),
                name: image.base.name.clone().unwrap_or_default(),
                src: image.src.to_string(),
            });
        }
        if let Some(grand) = child.children() {
            collect(grand, out);
        }
    }
}

/// Which popover an Images-tab button opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagesTabAction {
    Search,
    Generate,
}

/// What a press on the Images tab hits. Lives here rather than with the
/// widget because the hovered hit is UI state (`images_tab_hover`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImagesTabHit {
    Button(ImagesTabAction),
    Image(NodeId),
    /// The tab's own blank space — consumed so it never reaches the canvas.
    Inside,
}

/// Run an Images-tab button. Returns `true` when a placeholder node was
/// inserted, i.e. the host should mark the document dirty.
pub fn apply_images_tab_action(
    state: &mut EditorState,
    action: ImagesTabAction,
    now_ms: u64,
) -> bool {
    let inserted = !matches!(state.selected_node(), Some(PenNode::Image(_)));
    // The insert selects the new node itself.
    if inserted && state.insert_image_node_at_viewport("Image", "").is_none() {
        return false;
    }
    close_other_property_popovers_for_image(&mut state.editor_ui);
    let prompt = action == ImagesTabAction::Generate;
    // A placeholder's name says nothing about what the user wants.
    let seed = if inserted {
        String::new()
    } else {
        selected_image_seed(state, prompt)
    };
    if prompt {
        apply_image_generate_toggle(state, true, seed, now_ms);
    } else {
        apply_image_search_toggle(state, true, seed, now_ms);
    }
    inserted
}

#[cfg(test)]
#[path = "document_images_tests.rs"]
mod document_images_tests;
