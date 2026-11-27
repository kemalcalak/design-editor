//! Rectangle emission helpers for the SVG serializer. `attrs` are the layer
//! attributes of the node the rect stands for (`svg_export/node.rs`); the
//! stroke overlay of a container is an unnamed paint detail.

use super::shapes::stroke_aligned;
use super::{fill_attrs, fill_stroke_attrs, normalize_rect, stroke_attrs, stroke_style_attrs};
use crate::layout_scene::{NodeKind, SceneNode};
use std::fmt::Write as _;

pub(super) fn emit_rect(out: &mut String, node: &SceneNode, attrs: &str) {
    if node.fill.is_none()
        && node.gradient.is_none()
        && node.stroke.is_none()
        && !matches!(node.kind, NodeKind::Rect)
    {
        return;
    }
    emit_rect_shape(out, node, attrs, &fill_stroke_attrs(node), true);
}

pub(super) fn emit_rect_fill(out: &mut String, node: &SceneNode, attrs: &str) {
    emit_rect_shape(out, node, attrs, &fill_attrs(node), false);
}

pub(super) fn emit_rect_stroke_overlay(out: &mut String, node: &SceneNode) {
    let Some(stroke) = node.stroke else {
        return;
    };
    let paint = format!(
        r#" fill="none"{}{}"#,
        stroke_attrs(stroke.color, stroke.width),
        stroke_style_attrs(node)
    );
    // The overlay is a paint detail, not a second scene node. Leaving it
    // unlabelled avoids collisions with authored ids such as `foo-stroke`.
    emit_rect_shape(out, node, "", &paint, true);
}

fn emit_rect_shape(out: &mut String, node: &SceneNode, attrs: &str, paint: &str, stroked: bool) {
    let rect = normalize_rect(node.bounds);
    if rect.size.x == 0.0 && rect.size.y == 0.0 {
        return;
    }
    // Smoothed corners (#389): the precomputed outline, or for an inside /
    // outside stroke its aligned centre line (the same shift as below).
    let outline = if stroked {
        super::outline::stroked_outline(node)
    } else {
        super::outline::node_outline(node)
    };
    if let Some(d) = outline {
        let _ = write!(out, r#"<path{attrs} d="{d}"{paint}/>"#);
        return;
    }
    let (rect, radius) = if stroked {
        stroke_aligned(node, rect, node.corner_radius)
    } else {
        (rect, node.corner_radius)
    };
    let rx = if radius > 0.0 {
        format!(r#" rx="{radius}""#)
    } else {
        String::new()
    };
    let _ = write!(
        out,
        r#"<rect{attrs} x="{}" y="{}" width="{}" height="{}"{rx}{paint}/>"#,
        rect.origin.x, rect.origin.y, rect.size.x, rect.size.y,
    );
}
