//! Text runs for `CanvasKitBackend` — `draw_text` fills them, `stroke_text`
//! outlines them (#282). Moved out of `backend.rs` (800-line cap) when the
//! outline joined the fill.

use de_editor_ui::{Color, Point2D, TextLayout};

use super::bindings::OpCk;

/// Paint every run of `layout` at `origin`: filled in the run's colour, or
/// with `outline` = (colour, width) stroked along the glyph contours.
/// Complex-script runs take the bridge's shaped (browser) path either way.
pub(super) fn paint_runs(
    ck: &OpCk,
    layout: &TextLayout,
    origin: Point2D,
    outline: Option<(Color, f32)>,
) {
    let italic = layout.italic();
    for run in layout.runs() {
        let x = origin.x + run.origin.x;
        let y = origin.y + run.origin.y;
        // `TextRun.color` is `jian_core::scene::Color` (0-255 u8 channels),
        // unlike the f32-channel `Color` the fill ops take.
        let c = run.color;
        let (cr, cg, cb, ca) = (
            f32::from(c.r()) / 255.0,
            f32::from(c.g()) / 255.0,
            f32::from(c.b()) / 255.0,
            f32::from(c.a()) / 255.0,
        );
        let text = run.content.as_str();
        let family = run.font_family.as_str();
        let weight = run.font_weight as i32;
        // Same predicate the native host uses, so the two never disagree
        // about which runs need bidi + contextual joining.
        let complex = de_editor_core::text_script::needs_complex_shaping(text);
        match outline {
            None if complex => ck.draw_shaped_text(
                text,
                family,
                x,
                y,
                run.font_size,
                weight,
                italic,
                cr,
                cg,
                cb,
                ca,
            ),
            None => ck.draw_text(
                text,
                family,
                x,
                y,
                run.font_size,
                weight,
                italic,
                cr,
                cg,
                cb,
                ca,
            ),
            Some((color, width)) if complex => ck.stroke_shaped_text(
                text,
                family,
                x,
                y,
                run.font_size,
                weight,
                italic,
                color.r,
                color.g,
                color.b,
                color.a,
                width,
            ),
            Some((color, width)) => ck.stroke_text(
                text,
                family,
                x,
                y,
                run.font_size,
                weight,
                italic,
                color.r,
                color.g,
                color.b,
                color.a,
                width,
            ),
        }
    }
}
