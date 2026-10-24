//! Stroke paint for layout-scene nodes (#13).
//!
//! A stroke whose paint is a single solid colour draws with that colour, as
//! before. A richer stroke paint (gradient, shader, image, or several
//! layers, `SceneNode::stroke_fill_layers`) follows the same rules as a node
//! fill: the paint stack is laid out over the stroke's outer box, then the
//! stroke geometry, drawn in opaque white, masks it (`DstIn`). Every
//! primitive (rect, oval, polygon, path, line) keeps its own stroke code and
//! only takes the colour it is handed, so caps, joins, dashes and alignment
//! carry over unchanged. Backends without pixel masks keep the solid
//! fallback colour.
//!
//! The rect stroke helpers moved here from `canvas_viewport_overlay.rs`
//! (pure code motion apart from the paint wrapper).

use crate::layout_scene::{SceneFillLayer, SceneNode, SceneStroke, SceneStrokeAlign};
use crate::widgets::canvas_viewport_fill_layers::paint_clipped_shape_rich_fill_layer;
use crate::widgets::canvas_viewport_overlay::{align_stroke_rect, scaled_non_uniform_corner_radii};
use crate::widgets::PaintCx;
use crate::{Color, ImageBlendMode, Point2D, Rect};

/// Coverage colour for the mask pass: only its alpha matters.
const MASK_COVER: Color = Color::WHITE;

/// Run `draw`, which strokes `node`'s geometry with the colour it is handed.
/// A solid stroke gets `stroke.color`; a rich stroke paint gets the mask
/// treatment described in the module docs. No stroke, no call.
pub(crate) fn with_stroke_paint(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    draw: impl FnOnce(&mut PaintCx<'_>, Color),
) {
    if let Some(stroke) = node.stroke {
        with_stroke_paint_or(cx, node, world_rect, zoom, stroke.color, draw);
    }
}

/// [`with_stroke_paint`] for primitives that also draw without an authored
/// stroke (a bare line or open path keeps an implicit outline): `solid` is
/// the colour used when there is no rich stroke paint.
pub(crate) fn with_stroke_paint_or(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    solid: Color,
    draw: impl FnOnce(&mut PaintCx<'_>, Color),
) {
    let stroke = match node.stroke {
        Some(stroke)
            if !node.stroke_fill_layers.is_empty() && cx.backend.supports_pixel_masks() =>
        {
            stroke
        }
        _ => {
            draw(cx, solid);
            return;
        }
    };
    let paint_rect = stroke_paint_rect(world_rect, &stroke, zoom);
    // Miter joins and square caps of open paths can reach past the band;
    // the layer bounds only limit the offscreen allocation.
    let reach = stroke.width * zoom * 2.0 + 2.0;
    let layer_rect = inflate(paint_rect, reach);
    cx.backend
        .push_composite_layer(layer_rect, node.opacity, ImageBlendMode::Normal);
    // Canonical paints are front-to-back; raster painting runs back-to-front.
    for layer in node.stroke_fill_layers.iter().rev() {
        let blend_mode = layer.blend_mode();
        let isolated = blend_mode != ImageBlendMode::Normal;
        if isolated {
            cx.backend.push_composite_layer(layer_rect, 1.0, blend_mode);
        }
        if !paint_clipped_shape_rich_fill_layer(cx, node, layer, paint_rect, zoom) {
            if let SceneFillLayer::Solid { color, .. } = layer {
                cx.backend.fill_rect(paint_rect, *color);
            }
        }
        if isolated {
            cx.backend.restore();
        }
    }
    cx.backend.push_mask_source_layer(false);
    draw(cx, MASK_COVER);
    cx.backend.restore();
    cx.backend.restore();
}

/// The box a rich stroke paint is laid out over: the node box grown by the
/// part of the band that can lie outside it, so an image or gradient spans
/// the whole visible stroke. At least half the width even for an inside
/// stroke: lines, polygons, paths and arcs stroke centred whatever the
/// alignment, and a zero-height line box would mask its stroke away.
pub(crate) fn stroke_paint_rect(world_rect: Rect, stroke: &SceneStroke, zoom: f32) -> Rect {
    let width = stroke.width * zoom;
    let outward = match stroke.align {
        SceneStrokeAlign::Inside | SceneStrokeAlign::Center => width / 2.0,
        SceneStrokeAlign::Outside => width,
    };
    inflate(world_rect, outward)
}

fn inflate(rect: Rect, by: f32) -> Rect {
    Rect::xywh(
        rect.origin.x - by,
        rect.origin.y - by,
        rect.size.x + by * 2.0,
        rect.size.y + by * 2.0,
    )
}

/// An oval's centred stroke across `world_rect` (image-filled ellipses).
pub(crate) fn paint_oval_stroke(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
) {
    let width = node.stroke.map_or(0.0, |stroke| stroke.width * zoom);
    with_stroke_paint(cx, node, world_rect, zoom, |cx, color| {
        cx.backend.stroke_oval(world_rect, color, width);
    });
}

/// A polygon outline through `points` (world space).
pub(crate) fn paint_polygon_stroke(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    points: &[Point2D],
    world_rect: Rect,
    zoom: f32,
) {
    let width = node.stroke.map_or(0.0, |stroke| stroke.width * zoom);
    with_stroke_paint(cx, node, world_rect, zoom, |cx, color| {
        cx.backend.stroke_polygon(points, color, width);
    });
}

/// A line node: the bounds' top-left → bottom-right diagonal, at the stroke
/// width, or a 1.5 px implicit outline in the fill colour without a stroke.
pub(crate) fn paint_line_node_stroke(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
) {
    let from = world_rect.origin;
    let to = Point2D::new(
        world_rect.origin.x + world_rect.size.x,
        world_rect.origin.y + world_rect.size.y,
    );
    let (color, width) = match node.stroke {
        Some(s) => (s.color, s.width * zoom),
        None => (node.fill.unwrap_or(Color::BLACK), (1.5_f32).max(zoom)),
    };
    with_stroke_paint_or(cx, node, world_rect, zoom, color, |cx, color| {
        cx.backend.stroke_line(from, to, color, width);
    });
}

/// Paint only a node's stroke. Layered-fill nodes use this after painting all
/// background layers so the outline lands exactly once above the stack.
pub(crate) fn paint_node_stroke(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
) {
    let r = node.corner_radius * zoom;
    let use_round = r > 0.5;
    let per_corner = scaled_non_uniform_corner_radii(node, zoom);
    let Some(stroke) = node.stroke else {
        return;
    };
    with_stroke_paint(cx, node, world_rect, zoom, |cx, color| {
        if let Some(sides) = stroke
            .sides
            .filter(|sides| !stroke_sides_are_uniform(*sides))
        {
            paint_sided_rect_stroke(cx, world_rect, color, sides, zoom);
        } else {
            let w = stroke.width * zoom;
            let (rect, r) = align_stroke_rect(world_rect, r, w, stroke.align);
            if use_round {
                if let Some(radii) = per_corner {
                    cx.backend.stroke_round_rect_per_corner(
                        rect,
                        align_corner_radii(radii, w, stroke.align),
                        color,
                        w,
                    );
                } else {
                    cx.backend.stroke_round_rect(rect, r, color, w);
                }
            } else {
                cx.backend.stroke_rect(rect, color, w);
            }
        }
    });
}

fn align_corner_radii(radii: [f32; 4], width: f32, align: SceneStrokeAlign) -> [f32; 4] {
    let half = width / 2.0;
    match align {
        SceneStrokeAlign::Center => radii,
        SceneStrokeAlign::Inside => radii.map(|radius| (radius - half).max(0.0)),
        SceneStrokeAlign::Outside => {
            radii.map(|radius| if radius > 0.0 { radius + half } else { 0.0 })
        }
    }
}

fn stroke_sides_are_uniform(sides: [f32; 4]) -> bool {
    sides
        .iter()
        .all(|side| (*side - sides[0]).abs() < f32::EPSILON)
}

fn paint_sided_rect_stroke(
    cx: &mut PaintCx<'_>,
    rect: Rect,
    color: Color,
    sides: [f32; 4],
    zoom: f32,
) {
    let x0 = rect.origin.x;
    let y0 = rect.origin.y;
    let x1 = rect.origin.x + rect.size.x;
    let y1 = rect.origin.y + rect.size.y;
    let [top, right, bottom, left] = sides;
    if top > 0.0 {
        let width = top * zoom;
        cx.backend.stroke_line(
            Point2D::new(x0, y0 + width / 2.0),
            Point2D::new(x1, y0 + width / 2.0),
            color,
            width,
        );
    }
    if right > 0.0 {
        let width = right * zoom;
        cx.backend.stroke_line(
            Point2D::new(x1 - width / 2.0, y0),
            Point2D::new(x1 - width / 2.0, y1),
            color,
            width,
        );
    }
    if bottom > 0.0 {
        let width = bottom * zoom;
        cx.backend.stroke_line(
            Point2D::new(x0, y1 - width / 2.0),
            Point2D::new(x1, y1 - width / 2.0),
            color,
            width,
        );
    }
    if left > 0.0 {
        let width = left * zoom;
        cx.backend.stroke_line(
            Point2D::new(x0 + width / 2.0, y0),
            Point2D::new(x0 + width / 2.0, y1),
            color,
            width,
        );
    }
}

#[cfg(test)]
#[path = "canvas_viewport_stroke_paint_tests.rs"]
mod tests;
