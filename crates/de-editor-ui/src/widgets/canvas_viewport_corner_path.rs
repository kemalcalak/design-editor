//! Painting a node's precomputed corner outline (#389).
//!
//! The scene builder stores `SceneNode::corner_path` for smoothed
//! rect-like corners, rounded polygons / stars and rounded path vertices
//! (node-local doc px). Fills paint inside a clip to the outline with
//! square corners, so the clip owns the only anti-aliased edge; the stroke
//! follows the outline (or its aligned centre line); clips and backdrop
//! silhouettes clip to it. A node without an outline takes none of these
//! branches, so plain rects, rounded rects and sharp polygons paint
//! exactly as before.

use crate::layout_scene::{Effect, SceneCornerPath, SceneNode};
use crate::widgets::PaintCx;
use crate::{Color, Rect};

/// The node's corner outline, when the scene built one.
pub(crate) fn corner_outline(node: &SceneNode) -> Option<&SceneCornerPath> {
    node.corner_path.as_deref()
}

/// The radius of the node's BOX corners, for chrome drawn around its box
/// (the reveal ghost, the interaction ring). A polygon's or a path's
/// `corner_radius` rounds its vertices, not its box, so it gives 0.
pub(crate) fn box_corner_radius(node: &SceneNode) -> f32 {
    use crate::layout_scene::NodeKind;
    match node.kind {
        NodeKind::Path | NodeKind::Polygon | NodeKind::Line => 0.0,
        _ => node.corner_radius,
    }
}

/// Node-local `bounds` placed on screen: offset from `world_rect`'s origin
/// and scaled by `zoom` — the rect the backends fit outline data into.
pub(crate) fn outline_rect(world_rect: Rect, bounds: Rect, zoom: f32) -> Rect {
    Rect::xywh(
        world_rect.origin.x + bounds.origin.x * zoom,
        world_rect.origin.y + bounds.origin.y * zoom,
        bounds.size.x * zoom,
        bounds.size.y * zoom,
    )
}

/// Intersect the clip with the outline. The caller owns the `save`.
pub(crate) fn clip_to_outline(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    outline: &SceneCornerPath,
    world_rect: Rect,
    zoom: f32,
) {
    cx.backend.clip_svg_path_in_rect(
        &outline.d,
        outline_rect(world_rect, outline.bounds, zoom),
        node.even_odd_fill,
    );
}

/// Run `paint` inside the outline clip when `node` has one, else as is.
pub(crate) fn with_outline_clip<R>(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    paint: impl FnOnce(&mut PaintCx<'_>) -> R,
) -> R {
    let Some(outline) = corner_outline(node) else {
        return paint(cx);
    };
    cx.backend.save();
    clip_to_outline(cx, node, outline, world_rect, zoom);
    let out = paint(cx);
    cx.backend.restore();
    out
}

/// The uniform radius (`0` below half a pixel) and the per-corner radii a
/// rect fill draws at `zoom`. Square inside an outline clip: the clip
/// shapes the corners.
pub(crate) fn box_fill_radii(node: &SceneNode, zoom: f32) -> (f32, Option<[f32; 4]>) {
    if node.corner_path.is_some() {
        return (0.0, None);
    }
    let radius = node.corner_radius * zoom;
    (
        if radius > 0.5 { radius } else { 0.0 },
        crate::widgets::canvas_viewport_overlay::scaled_non_uniform_corner_radii(node, zoom),
    )
}

/// Stroke along the outline in `color` at `width` screen px: the aligned
/// centre line for an inside / outside stroke, else the outline itself.
pub(crate) fn stroke_outline(
    cx: &mut PaintCx<'_>,
    outline: &SceneCornerPath,
    world_rect: Rect,
    zoom: f32,
    color: Color,
    width: f32,
) {
    let (d, bounds) = if outline.stroke_d.is_empty() {
        (&outline.d, outline.bounds)
    } else {
        (&outline.stroke_d, outline.stroke_bounds)
    };
    cx.backend
        .stroke_svg_path_in_rect(d, outline_rect(world_rect, bounds, zoom), color, width);
}

/// Inset drop shadows inside the outline (outer shadows stay with the
/// node kind's shadow pass).
pub(crate) fn paint_outline_inner_shadows(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    outline: &SceneCornerPath,
    world_rect: Rect,
    zoom: f32,
) {
    let rect = outline_rect(world_rect, outline.bounds, zoom);
    for effect in &node.effects {
        let Effect::DropShadow(s) = effect else {
            continue;
        };
        if !s.inner {
            continue;
        }
        let isolated = s.blend_mode != crate::ImageBlendMode::Normal;
        if isolated {
            cx.backend.push_blend_layer(s.blend_mode);
        }
        cx.backend.fill_inner_shadow_svg_path_with_fill_rule(
            &outline.d,
            rect,
            s.offset_x * zoom,
            s.offset_y * zoom,
            s.blur * zoom,
            s.color,
            node.even_odd_fill,
        );
        if isolated {
            cx.backend.restore();
        }
    }
}

/// Paint a polygon or path through its outline: the fill stack clipped to
/// it (when `filled`), inset shadows, then the stroke — its end geometry
/// when it has one — along it. A path with neither stroke nor fill keeps
/// the editor's implicit hairline so it stays visible.
pub(crate) fn paint_outline_shape(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    outline: &SceneCornerPath,
    world_rect: Rect,
    zoom: f32,
    filled: bool,
) {
    if filled && !super::canvas_viewport_fill_layers::paint_fill_layers(cx, node, world_rect, zoom)
    {
        super::canvas_viewport_overlay::paint_node_fill(cx, node, world_rect, zoom, node.fill);
    }
    paint_outline_inner_shadows(cx, node, outline, world_rect, zoom);
    if let (Some(_), Some(geometry)) = (node.stroke, node.stroke_geometry.as_deref()) {
        super::canvas_viewport_stroke_paint::paint_stroke_geometry(
            cx, node, world_rect, zoom, geometry,
        );
        return;
    }
    if node.stroke.is_some() {
        super::canvas_viewport_stroke_paint::paint_node_stroke(cx, node, world_rect, zoom);
    } else if node.kind == crate::layout_scene::NodeKind::Path && !(filled && node.fill.is_some()) {
        let color = node.fill.unwrap_or(Color::BLACK);
        stroke_outline(cx, outline, world_rect, zoom, color, 1.5_f32.max(zoom));
    }
}

#[cfg(test)]
#[path = "canvas_viewport_corner_path_tests.rs"]
mod tests;
