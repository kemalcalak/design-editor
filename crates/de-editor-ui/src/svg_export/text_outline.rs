//! Outlined text (#282) in the SVG export.
//!
//! The outline is the text again with `fill="none"` and the node's stroke,
//! which SVG centres on the glyph contours. Alignment is a paint order, as
//! on the canvas: an outside outline is twice the width before the fill, a
//! centred one the width after it, and an inside one twice the width after
//! it, clipped to the glyphs. A rich stroke paint keeps its solid fallback
//! colour, like shape strokes in this export. Widget text keeps its stroke
//! as the field border and is not outlined, and decorations are not
//! outlined (as on the canvas).

use std::fmt::Write as _;

use super::text::{emit_text_lines, emit_text_outline_lines};
use super::{stroke_attrs, stroke_style_attrs, svg_id};
use crate::layout_scene::{SceneNode, SceneStroke, SceneStrokeAlign};
use crate::text_outline::text_outline_stroke as outline_stroke;

/// The outline that paints under the fill (outside alignment).
pub(super) fn emit_outline_under(out: &mut String, n: &SceneNode, text: &str) {
    if let Some(stroke) = outline_stroke(n).filter(|s| s.align == SceneStrokeAlign::Outside) {
        emit_outline(out, n, text, stroke, stroke.width * 2.0, None);
    }
}

/// The outline that paints over the fill (centre and inside alignment).
pub(super) fn emit_outline_over(out: &mut String, n: &SceneNode, text: &str) {
    let Some(stroke) = outline_stroke(n) else {
        return;
    };
    match stroke.align {
        SceneStrokeAlign::Center => emit_outline(out, n, text, stroke, stroke.width, None),
        SceneStrokeAlign::Inside => {
            let clip = format!("text-outline-clip-{}", svg_id(&n.id));
            emit_outline(out, n, text, stroke, stroke.width * 2.0, Some(&clip));
        }
        SceneStrokeAlign::Outside => {}
    }
}

/// The glyph clip an inside outline references.
pub(super) fn emit_outline_defs(out: &mut String, n: &SceneNode, text: &str, id: &str) {
    if outline_stroke(n).is_some_and(|s| s.align == SceneStrokeAlign::Inside) {
        let _ = write!(out, r#"<clipPath id="text-outline-clip-{id}">"#);
        emit_text_lines(out, n, text, "");
        out.push_str("</clipPath>");
    }
}

fn emit_outline(
    out: &mut String,
    n: &SceneNode,
    text: &str,
    stroke: SceneStroke,
    width: f32,
    clip: Option<&str>,
) {
    if let Some(clip) = clip {
        let _ = write!(out, r#"<g clip-path="url(#{clip})">"#);
    }
    let attrs = format!(
        r#" fill="none"{}{}"#,
        stroke_attrs(stroke.color, width),
        stroke_style_attrs(n)
    );
    emit_text_outline_lines(out, n, text, &attrs);
    if clip.is_some() {
        out.push_str("</g>");
    }
}

#[cfg(test)]
#[path = "text_outline_tests.rs"]
mod tests;
