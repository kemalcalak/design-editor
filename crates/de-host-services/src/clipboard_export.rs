//! Copy as PNG / SVG (#399): the selection rendered for the system
//! clipboard through the existing export paths — PNG through
//! `de_render_export`, SVG through the layered SVG export (#407). The
//! desktop writes the result to the OS clipboard.
//!
//! One selected node renders exactly as File ▸ Export does for it, cropped
//! tight. Several render together over their combined bounds; a node whose
//! ancestor is also selected is left to that ancestor.

pub use crate::clipboard_export_error::ClipboardExportError;
use crate::export::{render_node_on_page_raster_bytes, render_nodes_raster_bytes, RasterFormat};
use de_editor_core::EditorState;
use de_editor_ui::layout_scene::{SceneNode, ScenePage};

/// Copy as PNG renders at 2x, like Figma, so a paste stays sharp on a
/// high-density screen.
pub const COPY_PNG_SCALE: f32 = 2.0;

/// The selection as PNG bytes.
pub fn selection_png(state: &EditorState) -> Result<Vec<u8>, ClipboardExportError> {
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
    let page = scene
        .active_page()
        .ok_or(ClipboardExportError::NoActivePage)?;
    let ids = topmost_selected(state, page);
    let bytes = match ids.as_slice() {
        [] => return Err(ClipboardExportError::NothingSelected),
        [id] => render_node_on_page_raster_bytes(page, id, RasterFormat::Png, COPY_PNG_SCALE)?,
        _ => {
            let nodes: Vec<&SceneNode> = ids.iter().filter_map(|id| page.find(id)).collect();
            render_nodes_raster_bytes(&nodes, RasterFormat::Png, COPY_PNG_SCALE)?
        }
    };
    Ok(bytes)
}

/// The selection as layered SVG markup (layer names as ids, images
/// embedded).
pub fn selection_svg(state: &EditorState) -> Result<String, ClipboardExportError> {
    let (scene, names) = de_pen_loader::editor_state_to_active_page_layout_scene_with_names(state);
    let page = scene
        .active_page()
        .ok_or(ClipboardExportError::NoActivePage)?;
    let ids = topmost_selected(state, page);
    let export = match ids.as_slice() {
        [] => return Err(ClipboardExportError::NothingSelected),
        [id] => crate::export_svg::render_layered_node_on_page_svg(page, &names, id)?,
        _ => {
            let nodes: Vec<SceneNode> = ids
                .iter()
                .filter_map(|id| page.find(id))
                .filter(|n| !n.hidden)
                .cloned()
                .collect();
            crate::export_svg::render_layered_nodes_svg(&nodes, &names)?
        }
    };
    Ok(export.svg)
}

/// Selected scene ids on `page`, without any whose ancestor is selected
/// too (the ancestor's render already holds them).
fn topmost_selected(state: &EditorState, page: &ScenePage) -> Vec<String> {
    let ids: Vec<&str> = state
        .selection
        .set
        .iter()
        .filter(|id| id.is_real())
        .map(|id| id.as_str())
        .filter(|id| page.find(id).is_some())
        .collect();
    ids.iter()
        .filter(|id| {
            !ids.iter().any(|other| {
                other != *id
                    && page
                        .find(other)
                        .is_some_and(|n| n.children.iter().any(|c| c.find(id).is_some()))
            })
        })
        .map(|id| id.to_string())
        .collect()
}

#[cfg(test)]
#[path = "clipboard_export_tests.rs"]
mod tests;
