//! Text whose fill is more than one solid colour (#179): an image,
//! gradient or shader fill, a stack of fills, or a blended fill paints
//! the whole canonical fill stack clipped to the glyphs, the way pen.dev
//! and Figma show a photo through a headline's letters.
//!
//! The stack paints in world space over the node box (so image decode
//! size, tile scale and gradient geometry match a rectangle with the same
//! fills), then a deferred mask source cuts it to the glyph coverage with
//! `DstIn`. Glyphs are laid out in document space under the viewport
//! scale, exactly like the plain path, so wrap and caret geometry do not
//! change. Runs with a fill of their own paint solid on top afterwards.
//!
//! Glyph ink that falls outside the node box gets no fill (the stack is
//! sized to the box). Backends without pixel masks, and greeked text,
//! keep the solid `fill` fallback.

use crate::layout_scene::SceneNode;
use crate::text_fill_stack::text_needs_fill_stack;
use crate::widgets::canvas_viewport::EditCaret;
use crate::widgets::canvas_viewport_fill_layers::paint_fill_layers;
use crate::widgets::canvas_viewport_text::{paint_text_node_pass, text_is_greeked, TextPass};
use crate::widgets::PaintCx;
use crate::{ImageBlendMode, Point2D, Rect};

/// Paint a text node at `world_rect` (its bounds under the viewport
/// transform `viewport_origin` + `zoom`). Plain-ink text takes the
/// single [`TextPass::Full`] paint; a rich fill stack takes the masked
/// path described in the module docs.
pub(super) fn paint_text_in_viewport(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    viewport_origin: Point2D,
    zoom: f32,
    edit_caret: &Option<EditCaret>,
) {
    let zoom = zoom.max(0.0001);
    let editing = edit_caret.as_ref().is_some_and(|c| c.editing == node.id);
    let stacked = text_needs_fill_stack(node)
        && cx.backend.supports_pixel_masks()
        && (editing || !text_is_greeked(cx.backend, node, zoom));
    if !stacked {
        in_doc_space(cx, viewport_origin, zoom, |cx| {
            paint_text_node_pass(cx, node, node.bounds, zoom, edit_caret, TextPass::Full);
        });
        return;
    }
    if editing {
        in_doc_space(cx, viewport_origin, zoom, |cx| {
            paint_text_node_pass(cx, node, node.bounds, zoom, edit_caret, TextPass::Selection);
        });
    }
    cx.backend
        .push_composite_layer(world_rect, 1.0, ImageBlendMode::Normal);
    paint_fill_layers(cx, node, world_rect, zoom);
    cx.backend.push_mask_source_layer(false);
    in_doc_space(cx, viewport_origin, zoom, |cx| {
        paint_text_node_pass(cx, node, node.bounds, zoom, edit_caret, TextPass::FillMask);
    });
    // First restore applies the glyph coverage to the stack with DstIn,
    // the second lands the cut stack on the backdrop.
    cx.backend.restore();
    cx.backend.restore();
    in_doc_space(cx, viewport_origin, zoom, |cx| {
        paint_text_node_pass(cx, node, node.bounds, zoom, edit_caret, TextPass::Overlay);
    });
}

/// Run `paint` with document coordinates mapped onto the viewport.
fn in_doc_space(
    cx: &mut PaintCx<'_>,
    viewport_origin: Point2D,
    zoom: f32,
    paint: impl FnOnce(&mut PaintCx<'_>),
) {
    cx.backend.save();
    cx.backend.translate(viewport_origin);
    cx.backend.scale(Point2D::new(zoom, zoom), Point2D::ZERO);
    paint(cx);
    cx.backend.restore();
}

#[cfg(test)]
#[path = "canvas_viewport_text_fill_tests.rs"]
mod tests;
