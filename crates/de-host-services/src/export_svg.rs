//! Layered SVG export (#407) for the MCP tools and the CLI: one node (a
//! frame, usually) per file, through the platform-neutral writer in
//! `de_editor_ui::svg_export`, with the document's layer names as element
//! ids and linked images embedded, so a team can hand the file to someone
//! who drops it into Figma and gets the same layer tree back.

use de_editor_ui::layout_scene::{LayoutScene, ScenePage};
use de_editor_ui::svg_export::{
    serialize_node_on_page_svg_with, serialize_page_svg_with, SvgExport, SvgExportError,
    SvgExportOptions,
};
use de_pen_loader::SceneLayerNames;

fn options(names: &SceneLayerNames) -> SvgExportOptions<'_> {
    // Icons that name a brand glyph resolve through the same catalog the
    // raster exports install before painting.
    let _ = de_editor_ui::set_brand_catalog(crate::web_static::ICONIFY_BRANDS_JSON);
    SvgExportOptions {
        layer_names: Some(names),
        embed_images: true,
    }
}

/// Node `node_id` of the scene's active page as a layered SVG.
pub fn render_layered_node_svg(
    scene: &LayoutScene,
    names: &SceneLayerNames,
    node_id: &str,
) -> Result<SvgExport, SvgExportError> {
    let page = scene.active_page().ok_or(SvgExportError::NoActivePage)?;
    serialize_node_on_page_svg_with(page, node_id, &options(names))
}

/// Node `node_id` of `page` as a layered SVG.
pub fn render_layered_node_on_page_svg(
    page: &ScenePage,
    names: &SceneLayerNames,
    node_id: &str,
) -> Result<SvgExport, SvgExportError> {
    serialize_node_on_page_svg_with(page, node_id, &options(names))
}

/// All of `page` as one layered SVG (each top-level frame a group).
pub fn render_layered_page_svg(
    page: &ScenePage,
    names: &SceneLayerNames,
) -> Result<SvgExport, SvgExportError> {
    serialize_page_svg_with(page, &options(names))
}

#[cfg(test)]
#[path = "export_svg_tests.rs"]
mod tests;
