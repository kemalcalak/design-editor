//! Outlined text (#282): a text node's stroke runs along its glyph
//! contours, never around its box.
//!
//! Backends stroke glyphs centred on the contour, so the alignment is a
//! paint order: an outside outline is twice the width under the fill (the
//! fill covers its inner half), a centred one the width over the fill, and
//! an inside one twice the width over the fill, cut to the glyph coverage.
//! Backends without pixel masks draw an inside outline centred.
//!
//! The outline paints through `with_stroke_paint`, so a gradient or image
//! stroke works like a shape's. Glyphs are laid out in document space under
//! the viewport scale, like the fill, so the authored dash is set unscaled
//! there and the node's screen-space style is restored after. Widget text
//! (its stroke is the field border) and greeked text get no outline. While
//! editing, the caret is painted again over an outline that covers it.

use crate::layout_scene::{SceneNode, SceneStroke, SceneStrokeAlign};
use crate::widgets::canvas_viewport::EditCaret;
use crate::widgets::canvas_viewport_stroke_paint::with_stroke_paint;
use crate::widgets::canvas_viewport_text::{paint_text_node_pass, text_is_greeked, TextPass};
use crate::widgets::canvas_viewport_text_fill::{in_doc_space, inflate};
use crate::widgets::PaintCx;
use crate::{ImageBlendMode, Point2D, Rect};

/// Paint `node`'s text through `fill`, with its outline before or after it
/// as the stroke's alignment asks.
pub(super) fn with_text_outline(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    viewport_origin: Point2D,
    zoom: f32,
    edit_caret: &Option<EditCaret>,
    fill: impl FnOnce(&mut PaintCx<'_>),
) {
    let Some(stroke) = outline_stroke(cx, node, zoom, edit_caret) else {
        fill(cx);
        return;
    };
    let at = Placement {
        node,
        world_rect,
        viewport_origin,
        zoom,
        edit_caret,
    };
    match stroke.align {
        SceneStrokeAlign::Outside => {
            at.outline(cx, stroke.width * 2.0);
            fill(cx);
        }
        SceneStrokeAlign::Center => {
            fill(cx);
            at.outline(cx, stroke.width);
            at.caret_over_outline(cx);
        }
        SceneStrokeAlign::Inside if cx.backend.supports_pixel_masks() => {
            fill(cx);
            at.inside_outline(cx, stroke.width * 2.0);
            at.caret_over_outline(cx);
        }
        SceneStrokeAlign::Inside => {
            fill(cx);
            at.outline(cx, stroke.width);
            at.caret_over_outline(cx);
        }
    }
}

/// The stroke to outline `node`'s glyphs with, if it gets one this frame.
fn outline_stroke(
    cx: &PaintCx<'_>,
    node: &SceneNode,
    zoom: f32,
    edit_caret: &Option<EditCaret>,
) -> Option<SceneStroke> {
    let stroke = node.stroke?;
    if node.widget.is_some() || stroke.width.is_nan() || stroke.width <= 0.0 {
        return None;
    }
    // An edited node paints its live buffer, which may hold text the
    // committed node does not yet.
    if is_editing(node, edit_caret) {
        return Some(stroke);
    }
    let has_text = node.text.as_deref().is_some_and(|text| !text.is_empty());
    (has_text && !text_is_greeked(cx.backend, node, zoom)).then_some(stroke)
}

fn is_editing(node: &SceneNode, edit_caret: &Option<EditCaret>) -> bool {
    edit_caret.as_ref().is_some_and(|c| c.editing == node.id)
}

/// Where the text node sits on screen, shared by the outline passes.
struct Placement<'a> {
    node: &'a SceneNode,
    world_rect: Rect,
    viewport_origin: Point2D,
    zoom: f32,
    edit_caret: &'a Option<EditCaret>,
}

impl Placement<'_> {
    /// Stroke every glyph `width` doc px wide, centred on its contour.
    fn outline(&self, cx: &mut PaintCx<'_>, width: f32) {
        let node = self.node;
        let style = node.stroke.map(|stroke| stroke.style).unwrap_or_default();
        with_stroke_paint(cx, node, self.world_rect, self.zoom, |cx, color| {
            in_doc_space(cx, self.viewport_origin, self.zoom, |cx| {
                cx.backend.set_stroke_style(style);
                let pass = TextPass::Stroke { color, width };
                paint_text_node_pass(cx, node, node.bounds, self.zoom, self.edit_caret, pass);
                cx.backend.set_stroke_style(style.scaled(self.zoom));
            });
        });
    }

    /// The caret and IME underline again, over an outline painted after
    /// them, while the node is being edited.
    fn caret_over_outline(&self, cx: &mut PaintCx<'_>) {
        if !is_editing(self.node, self.edit_caret) {
            return;
        }
        in_doc_space(cx, self.viewport_origin, self.zoom, |cx| {
            let node = self.node;
            let pass = TextPass::Caret;
            paint_text_node_pass(cx, node, node.bounds, self.zoom, self.edit_caret, pass);
        });
    }

    /// [`Self::outline`] cut to the glyph coverage, so only its inner half
    /// shows.
    fn inside_outline(&self, cx: &mut PaintCx<'_>, width: f32) {
        let size = self.world_rect.size;
        let layer = inflate(self.world_rect, size.x.abs().max(size.y.abs()));
        cx.backend
            .push_composite_layer(layer, 1.0, ImageBlendMode::Normal);
        self.outline(cx, width);
        cx.backend.push_mask_source_layer(false);
        in_doc_space(cx, self.viewport_origin, self.zoom, |cx| {
            let node = self.node;
            let pass = TextPass::Coverage;
            paint_text_node_pass(cx, node, node.bounds, self.zoom, self.edit_caret, pass);
        });
        // The first restore cuts the outline to the glyphs with DstIn, the
        // second lands it on the backdrop.
        cx.backend.restore();
        cx.backend.restore();
    }
}

#[cfg(test)]
#[path = "canvas_viewport_text_stroke_tests.rs"]
mod tests;
