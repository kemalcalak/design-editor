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

/// The options every layered export uses: layer names as ids, linked
/// images embedded.
fn layered_options(names: &SceneLayerNames) -> SvgExportOptions<'_> {
    SvgExportOptions {
        layer_names: Some(names),
        embed_images: true,
    }
}

/// Icons that name a brand glyph resolve through the same catalog the
/// raster exports install before painting (a no-op once installed).
fn install_brand_catalog() {
    let _ = de_editor_ui::set_brand_catalog(crate::web_static::ICONIFY_BRANDS_JSON);
}

/// Node `node_id` of the scene's active page as a layered SVG.
pub fn render_layered_node_svg(
    scene: &LayoutScene,
    names: &SceneLayerNames,
    node_id: &str,
) -> Result<SvgExport, SvgExportError> {
    let page = scene.active_page().ok_or(SvgExportError::NoActivePage)?;
    render_layered_node_on_page_svg(page, names, node_id)
}

/// Node `node_id` of `page` as a layered SVG.
pub fn render_layered_node_on_page_svg(
    page: &ScenePage,
    names: &SceneLayerNames,
    node_id: &str,
) -> Result<SvgExport, SvgExportError> {
    install_brand_catalog();
    serialize_node_on_page_svg_with(page, node_id, &layered_options(names))
}

/// All of `page` as one layered SVG (each top-level frame a group).
pub fn render_layered_page_svg(
    page: &ScenePage,
    names: &SceneLayerNames,
) -> Result<SvgExport, SvgExportError> {
    install_brand_catalog();
    serialize_page_svg_with(page, &layered_options(names))
}

#[cfg(test)]
#[path = "export_svg_tests.rs"]
mod tests;
