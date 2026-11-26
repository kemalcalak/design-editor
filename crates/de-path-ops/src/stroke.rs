//! Outline stroke and offset path, on skia's stroker.
//!
//! Outline stroke turns a node's stroke into the filled area it paints,
//! honouring cap, join, dash and alignment: an inside stroke is the
//! doubled stroke clipped to the shape, an outside one the doubled stroke
//! minus the shape. Offset path grows (or shrinks) an area by `delta`
//! with round / miter / bevel corners: the area united with (or minus) a
//! stroke of width `2 |delta|` along its edge.

use crate::outline::{node_geometry, node_outline};
pub use de_editor_core::path_ops::OffsetJoin;
use jian_scene::layout_scene::{SceneNode, SceneStrokeAlign, StrokeCap, StrokeJoin, StrokeStyle};
use skia_safe::{Paint, PaintCap, PaintJoin, PaintStyle, Path, PathBuilder, PathOp};

/// Miter limit for offset and outlined joins (skia's and SVG's default).
const MITER_LIMIT: f32 = 4.0;

/// The filled area of a `width` stroke along `path` with `style`'s cap,
/// join and dash (centred). Self-overlaps are merged, so the result fills
/// the same pixels under either fill rule.
pub fn stroke_to_fill(path: &Path, width: f32, style: &StrokeStyle) -> Option<Path> {
    if !(width > 0.0 && width.is_finite()) {
        return None;
    }
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_stroke_width(width);
    paint.set_stroke_miter(MITER_LIMIT);
    paint.set_stroke_cap(match style.cap {
        Some(StrokeCap::Round) => PaintCap::Round,
        Some(StrokeCap::Square) => PaintCap::Square,
        _ => PaintCap::Butt,
    });
    paint.set_stroke_join(match style.join {
        Some(StrokeJoin::Round) => PaintJoin::Round,
        Some(StrokeJoin::Bevel) => PaintJoin::Bevel,
        _ => PaintJoin::Miter,
    });
    if let Some(dash) = style.dash {
        if let Some(effect) = skia_safe::PathEffect::dash(dash.lengths(), dash.offset()) {
            paint.set_path_effect(effect);
        }
    }
    let mut out = PathBuilder::new();
    if !skia_safe::path_utils::fill_path_with_paint(path, &paint, &mut out, None, None) {
        return None;
    }
    let filled = out.detach();
    if filled.is_empty() {
        return None;
    }
    filled.simplify().or(Some(filled))
}

/// The area `node`'s stroke paints (doc space), with its alignment:
/// `None` when the node has no visible stroke or no vector geometry.
pub fn outline_stroke(node: &SceneNode) -> Option<Path> {
    let stroke = node.stroke.filter(|s| s.width > 0.0)?;
    let geometry = node_geometry(node)?;
    if !geometry.closed || stroke.align == SceneStrokeAlign::Center {
        return stroke_to_fill(&geometry.path, stroke.width, &stroke.style);
    }
    let band = stroke_to_fill(&geometry.path, stroke.width * 2.0, &stroke.style)?;
    let op = if stroke.align == SceneStrokeAlign::Inside {
        PathOp::Intersect
    } else {
        PathOp::Difference
    };
    band.op(&geometry.path, op).filter(|p| !p.is_empty())
}

/// `path` grown by `delta` doc px (shrunk when negative), corners joined
/// with `join`. `None` when the result is empty (an inset past the
/// shape's thickness) or skia gives up.
pub fn offset_path(path: &Path, delta: f32, join: OffsetJoin) -> Option<Path> {
    if delta == 0.0 {
        return Some(path.clone());
    }
    let style = StrokeStyle {
        miter_limit: None,
        start: None,
        end: None,
        cap: Some(StrokeCap::Round),
        join: Some(match join {
            OffsetJoin::Round => StrokeJoin::Round,
            OffsetJoin::Miter => StrokeJoin::Miter,
            OffsetJoin::Bevel => StrokeJoin::Bevel,
        }),
        dash: None,
    };
    let band = stroke_to_fill(path, delta.abs() * 2.0, &style)?;
    let op = if delta > 0.0 {
        PathOp::Union
    } else {
        PathOp::Difference
    };
    path.op(&band, op).filter(|p| !p.is_empty())
}

/// [`offset_path`] over a node's closed outline.
pub fn offset_node(node: &SceneNode, delta: f32, join: OffsetJoin) -> Option<Path> {
    offset_path(&node_outline(node)?, delta, join)
}

#[cfg(test)]
#[path = "stroke_tests.rs"]
mod tests;
