//! The filled import glyph, split out of `icons.rs` (800-line cap) and
//! re-exported there, so `icons::draw_import_icon` is unchanged.

use crate::{Color, Point2D, RenderBackend};

/// Paint the filled import glyph supplied for Design Editor's import
/// launcher. Its source SVG uses a non-square `1201 × 1024` viewBox, so the
/// vertical offset keeps the original aspect ratio centred in the requested
/// square without rewriting either authored path.
pub fn draw_import_icon(
    backend: &mut dyn RenderBackend,
    top_left: Point2D,
    size: f32,
    color: Color,
) {
    const VIEWBOX_WIDTH: f32 = 1201.0;
    const VIEWBOX_HEIGHT: f32 = 1024.0;
    const PATHS: &[&str] = &[
        "M1130.726 590.552c0 47.88-8.23 94.264-23.192 136.908-54.613-159.352-199.75-273.067-371.82-273.067v68.08c0 26.184-14.214 49.376-36.657 61.346-8.978 4.489-19.452 7.481-29.177 7.481-14.215 0-27.681-4.488-39.651-13.466L366.888 372.847c-16.46-12.718-26.185-32.918-26.185-54.613s9.726-41.895 26.185-54.614l263.34-205.735c11.223-8.977 25.437-13.466 39.652-13.466 9.725 0 20.199 2.244 29.177 7.481 22.443 11.222 36.658 35.162 36.658 61.347v68.08c217.705-0.749 395.011 183.29 395.011 409.225z",
        "M982.198 580.875c-26.184 0-47.88 21.695-47.88 47.88v57.606c0 100.997-3.74 181.795-191.52 181.795H359.755c-193.765 0-191.52-86.035-191.52-191.521V437.234c-13.467-150.374 57.605-183.291 143.64-190.025h2.992c26.185 0 47.88-21.695 47.88-47.88 0-26.184-21.695-48.628-47.88-48.628h-2.992v-0.748h-47.88c-105.486 0-191.521 86.035-191.521 191.52v430.922c0 105.486 86.035 191.521 191.52 191.521H837.81c105.486 0 191.52-86.035 191.52-191.52V628.754c0.749-26.185-20.947-47.88-47.132-47.88z",
    ];

    let rendered_height = size * VIEWBOX_HEIGHT / VIEWBOX_WIDTH;
    let origin = Point2D::new(top_left.x, top_left.y + (size - rendered_height) / 2.0);
    for d in PATHS {
        backend.fill_svg_path(d, origin, size, VIEWBOX_WIDTH, color);
    }
}
