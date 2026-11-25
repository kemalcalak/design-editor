//! The layer tree of the SVG export. Every exported node is exactly one
//! element of the file, so an importer (Figma, Illustrator) shows the same
//! layer tree as the editor:
//!
//! - a container (frame, group, or any node with children) is a `<g>`
//!   carrying the node's id, transform, opacity, blend and layer blur, with
//!   its own paint (background, image, stroke) as unnamed children around
//!   the child layers;
//! - a leaf is its one primitive (`<rect>`, `<text>`, `<image>`, …) with
//!   those attributes on it, or a `<g>` when its paint needs several
//!   elements (a fill stack, outlined text, an image with a border).
//!
//! Hidden and canvas-only nodes (notes, prompts, live browser pages) are
//! not part of the design and are left out.

use super::effects::{group_blur_attr, shape_filter_attr};
use super::fill_stack::{blend_style, emit_fill_stack};
use super::icon::{emit_icon, is_icon};
use super::image::{emit_box_image, BoxImage};
use super::rect::{emit_rect, emit_rect_fill, emit_rect_stroke_overlay};
use super::shapes::{emit_ellipse, emit_line, emit_path, emit_polygon};
use super::svg_id;
use super::text::emit_text;
use super::transform::{matrix_value, node_transform};
use super::writer::Writer;
use crate::layout_scene::{NodeKind, SceneFillLayer, SceneGradient, SceneNode, SceneStrokeAlign};
use std::fmt::Write as _;

/// The attributes that make an element stand for a node.
pub(super) struct LayerAttrs {
    /// ` id="…"` (and ` data-name`), empty for an unnamed part.
    pub id: String,
    pub xform: glam::Affine2,
    pub opacity: f32,
    pub blend: String,
    pub filter: String,
}

impl LayerAttrs {
    /// Attributes of an unnamed part of a node's paint (a container's
    /// background) that only carries a filter.
    fn part(filter: String) -> Self {
        Self {
            id: String::new(),
            xform: glam::Affine2::IDENTITY,
            opacity: 1.0,
            blend: String::new(),
            filter,
        }
    }

    /// Everything, with `extra` (the primitive's own transform, e.g. a
    /// path's fit into its box) composed inside the node transform and
    /// `own_opacity` folded into the layer opacity.
    pub(super) fn render(&self, extra: glam::Affine2, own_opacity: f32) -> String {
        let mut s = self.id.clone();
        let xform = self.xform * extra;
        if xform != glam::Affine2::IDENTITY {
            let _ = write!(s, r#" transform="{}""#, matrix_value(xform));
        }
        let opacity = (self.opacity * own_opacity).clamp(0.0, 1.0);
        if opacity < 1.0 {
            let _ = write!(s, r#" opacity="{opacity}""#);
        }
        s.push_str(&self.blend);
        s.push_str(&self.filter);
        s
    }

    pub(super) fn all(&self) -> String {
        self.render(glam::Affine2::IDENTITY, 1.0)
    }
}

/// Whether `n` is written as a `<g>` of layers.
pub(super) fn is_container(n: &SceneNode) -> bool {
    n.kind == NodeKind::Group || !n.children.is_empty()
}

/// Whether the node is part of the exported design.
pub(super) fn is_exported(n: &SceneNode) -> bool {
    !n.hidden && !crate::canvas_only::is_canvas_only(n)
}

pub(super) fn emit_node(out: &mut String, n: &SceneNode, w: &mut Writer) {
    if !is_exported(n) {
        return;
    }
    if n.kind == NodeKind::Group && n.children.iter().all(|c| !is_exported(c)) {
        return;
    }
    report_node_limits(n, w);
    let container = is_container(n);
    let composite = if n.composite_opacity.is_finite() {
        n.composite_opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let attrs = LayerAttrs {
        id: w.layer_attrs(n),
        xform: node_transform(n),
        opacity: composite,
        blend: blend_style(n.blend_mode),
        filter: if container {
            group_blur_attr(n)
        } else {
            shape_filter_attr(n, true)
        },
    };
    if container {
        emit_container(out, n, &attrs, w);
    } else {
        emit_leaf(out, n, &attrs, w);
    }
}

fn emit_container(out: &mut String, n: &SceneNode, attrs: &LayerAttrs, w: &mut Writer) {
    let _ = write!(out, "<g{}>", attrs.all());
    let own = LayerAttrs::part(shape_filter_attr(n, false));
    let defers_stroke = matches!(n.kind, NodeKind::Rect | NodeKind::Frame) && n.stroke.is_some();
    match &n.kind {
        NodeKind::Rect | NodeKind::Frame if defers_stroke => emit_box(out, n, &own, false, w),
        NodeKind::Rect | NodeKind::Frame | NodeKind::Other(_) => emit_box(out, n, &own, true, w),
        NodeKind::Group => {}
        _ => emit_leaf(out, n, &own, w),
    }
    let clips_children = n.clip_content && n.bounds.size.x != 0.0 && n.bounds.size.y != 0.0;
    if clips_children {
        let _ = write!(out, r#"<g clip-path="url(#clip-{})">"#, svg_id(&n.id));
    }
    for child in &n.children {
        emit_node(out, child, w);
    }
    if clips_children {
        out.push_str("</g>");
    }
    if defers_stroke {
        emit_rect_stroke_overlay(out, n);
    }
    out.push_str("</g>");
}

fn emit_leaf(out: &mut String, n: &SceneNode, attrs: &LayerAttrs, w: &mut Writer) {
    if is_icon(n) {
        return emit_icon(out, n, attrs, w);
    }
    match &n.kind {
        NodeKind::Rect | NodeKind::Frame | NodeKind::Other(_) => emit_box(out, n, attrs, true, w),
        NodeKind::Ellipse => emit_ellipse(out, n, &attrs.all()),
        NodeKind::Polygon => emit_polygon(out, n, &attrs.all()),
        NodeKind::Line => emit_line(out, n, &attrs.all()),
        NodeKind::Path => emit_path(out, n, attrs),
        NodeKind::Text => emit_text(out, n, &attrs.all(), w),
        NodeKind::Group => {}
    }
}

/// How a rect-shaped node's box is painted, in the canvas's order: the
/// canonical fill stack, else an image node's picture, else the legacy
/// single fill / gradient.
pub(super) enum BoxPaint<'a> {
    Stack,
    Image(BoxImage<'a>),
    Plain,
}

pub(super) fn box_paint(n: &SceneNode) -> BoxPaint<'_> {
    match n.fill_layers.as_slice() {
        [] => match BoxImage::of_node(n) {
            Some(image) => BoxPaint::Image(image),
            None => BoxPaint::Plain,
        },
        [layer @ SceneFillLayer::Image { .. }]
            if layer.blend_mode() == crate::ImageBlendMode::Normal =>
        {
            BoxPaint::Image(BoxImage::of_layer(layer).expect("image layer"))
        }
        [SceneFillLayer::Solid { blend_mode, .. } | SceneFillLayer::Gradient { blend_mode, .. }]
            if *blend_mode == crate::ImageBlendMode::Normal =>
        {
            BoxPaint::Plain
        }
        _ => BoxPaint::Stack,
    }
}

/// Whether the node's box paint needs the rounded `clip-…` def.
pub(super) fn box_needs_clip(n: &SceneNode) -> bool {
    n.corner_radius > 0.0
        && matches!(
            n.kind,
            NodeKind::Rect | NodeKind::Frame | NodeKind::Other(_)
        )
        && !matches!(box_paint(n), BoxPaint::Plain)
}

/// Paint a rect / frame / other node's box. `with_stroke` is false for a
/// container whose stroke is drawn over its children.
fn emit_box(
    out: &mut String,
    n: &SceneNode,
    attrs: &LayerAttrs,
    with_stroke: bool,
    w: &mut Writer,
) {
    let stroke = with_stroke && n.stroke.is_some();
    match box_paint(n) {
        BoxPaint::Plain if with_stroke => emit_rect(out, n, &attrs.all()),
        BoxPaint::Plain => emit_rect_fill(out, n, &attrs.all()),
        BoxPaint::Image(image) if !stroke => {
            let attrs = attrs.render(glam::Affine2::IDENTITY, image.opacity);
            emit_box_image(out, n, &image, &attrs, w);
        }
        BoxPaint::Image(image) => {
            let _ = write!(out, "<g{}>", attrs.all());
            let own = if image.opacity < 1.0 {
                format!(r#" opacity="{}""#, image.opacity.clamp(0.0, 1.0))
            } else {
                String::new()
            };
            emit_box_image(out, n, &image, &own, w);
            emit_rect_stroke_overlay(out, n);
            out.push_str("</g>");
        }
        BoxPaint::Stack => {
            let _ = write!(out, "<g{}>", attrs.all());
            let clip = (n.corner_radius > 0.0).then(|| format!("clip-{}", svg_id(&n.id)));
            emit_fill_stack(out, n, clip.as_deref(), "", w);
            if stroke {
                emit_rect_stroke_overlay(out, n);
            }
            out.push_str("</g>");
        }
    }
}

/// Report what this node carries that the file does not.
fn report_node_limits(n: &SceneNode, w: &mut Writer) {
    if n.is_mask {
        w.warn(
            n,
            "mask is exported as a plain shape; the masking is not kept",
        );
    }
    if n.video.is_some() {
        w.warn(n, "video is exported as its poster image");
    }
    if n.widget.is_some() {
        w.warn(n, "form control visuals are exported as the control's box");
    }
    if n.shader.is_some() && n.fill_layers.is_empty() {
        w.warn(n, "shader fill (SkSL) is exported as its fallback colour");
    }
    let rich_fill = n.fill_layers.len() > 1
        || n.fill_layers.iter().any(|layer| {
            matches!(
                layer,
                SceneFillLayer::Image { .. } | SceneFillLayer::Shader { .. }
            )
        });
    if rich_fill
        && matches!(
            n.kind,
            NodeKind::Ellipse | NodeKind::Polygon | NodeKind::Path | NodeKind::Line
        )
    {
        w.warn(
            n,
            "only the top solid or gradient fill of this shape is exported",
        );
    }
    match &n.gradient {
        Some(SceneGradient::Mesh { .. }) => {
            w.warn(n, "mesh gradient is exported as its first colour")
        }
        Some(SceneGradient::Angular { .. }) => {
            w.warn(n, "angular gradient is exported as a pattern of wedges")
        }
        _ => {}
    }
    if !n.stroke_fill_layers.is_empty() {
        w.warn(n, "stroke paint is exported as its solid fallback colour");
    }
    if let Some(stroke) = n.stroke {
        if stroke.sides.is_some() {
            w.warn(n, "per-side stroke widths are exported as one width");
        }
        let aligned_by_writer = matches!(
            n.kind,
            NodeKind::Rect
                | NodeKind::Frame
                | NodeKind::Ellipse
                | NodeKind::Other(_)
                | NodeKind::Text
        );
        if stroke.align != SceneStrokeAlign::Center && !aligned_by_writer {
            w.warn(
                n,
                "inside / outside stroke is exported centred on the outline",
            );
        }
    }
    if n.corner_radii
        .is_some_and(|radii| radii.iter().any(|r| *r != radii[0]))
    {
        w.warn(n, "per-corner radii are exported as one radius");
    }
}
