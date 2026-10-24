//! Canvas paint for canonical `icon_font` nodes.

use super::icon_catalog::IconRenderStyle;
use super::icon_family::{canonical_family, lookup_icon_weighted};
use super::icons::{draw_icon, draw_icon_catalog_entry, Icon};
use crate::{Color, Point2D, RenderBackend};

/// Paint a canonical `icon_font` node by Iconify collection + name at the
/// regular weight. See [`paint_icon_font_node_weighted`].
pub fn paint_icon_font_node(
    backend: &mut dyn RenderBackend,
    family: &str,
    name: &str,
    rect: crate::Rect,
    fill: Option<Color>,
) {
    paint_icon_font_node_weighted(backend, family, name, 0, rect, fill);
}

/// Paint a canonical `icon_font` node by Iconify collection + name, scaled
/// into `rect` with aspect preserved. `weight` is the node's icon weight
/// (`0` = unset); `icon_family` says what each family does with it. Unknown
/// names are left unpainted instead of rendering a misleading fallback glyph.
pub fn paint_icon_font_node_weighted(
    backend: &mut dyn RenderBackend,
    family: &str,
    name: &str,
    weight: u16,
    rect: crate::Rect,
    fill: Option<Color>,
) {
    let size = rect.size.x.min(rect.size.y).max(0.0);
    if size <= 0.0 {
        return;
    }
    let color = fill.unwrap_or(Color {
        r: 0.39,
        g: 0.45,
        b: 0.55,
        a: 1.0,
    });
    let top_left = Point2D::new(
        rect.origin.x + (rect.size.x - size) / 2.0,
        rect.origin.y + (rect.size.y - size) / 2.0,
    );
    let stroke_width = (size / 24.0 * 2.0).max(1.0);
    let family = canonical_family(family);
    if let Some(icon) = lookup_icon_weighted(&family, name, weight) {
        draw_icon_catalog_entry(backend, icon.entry, top_left, size, color, stroke_width);
        // Weight above the catalogued master (Material Symbols): thicken
        // every stem by stroking the filled outline in the same colour.
        // `stroke_svg_path` scales a 24-unit path, which is the Material grid.
        if icon.outline_units > 0.0 && icon.entry.style == IconRenderStyle::Fill {
            backend.stroke_svg_path(
                &icon.entry.d,
                top_left,
                size,
                color,
                icon.outline_units * size / 24.0,
            );
        }
    } else {
        // Web only: the core catalog is fetched lazily, and historically only
        // the icon picker asked for it — a document full of `icon_font` nodes
        // painted unresolved dots forever unless the picker was opened. The
        // paint that misses the catalog now requests the fetch itself (same
        // seam the prompt-center and scene-template paints use); the install
        // wakes a repaint and the glyphs resolve. Extended families request
        // their own catalog inside the lookup.
        #[cfg(all(target_arch = "wasm32", feature = "runtime-icon-catalog"))]
        if !super::icon_catalog::core_catalog_loaded() {
            de_editor_core::web_assets::request(super::icon_catalog::ICONIFY_CORE_ROUTE);
        }
        if family == "lucide" {
            if let Some(icon) = Icon::from_name(name) {
                draw_icon(backend, icon, top_left, size, color, stroke_width);
            }
        }
    }
}

#[cfg(test)]
#[path = "icons_font_node_tests.rs"]
mod tests;
