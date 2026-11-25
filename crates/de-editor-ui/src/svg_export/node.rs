//! The layer tree of the SVG export. Every written node is at most one
//! element of the file (none when it paints nothing), so an importer
//! (Figma, Illustrator) shows the same layer tree as the editor:
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
//! not part of the design and are left out. The defs pass
//! (`svg_export.rs`) asks the same predicates as this one ([`is_written`],
//! [`is_container`], [`clips_children`], [`box_clip`]), so every
//! `url(#…)` written here has its def.

use super::effects::{blend_name, group_blur_attr, shape_filter_attr};
use super::fill_stack::emit_fill_stack;
use super::icon::{emit_icon, is_icon};
use super::image::{emit_box_image, BoxImage};
use super::rect::{emit_rect, emit_rect_fill, emit_rect_stroke_overlay};
use super::shapes::{emit_ellipse, emit_line, emit_path, emit_polygon};
use super::svg_id;
use super::text::emit_text;
use super::transform::{matrix_value, node_transform};
use super::writer::Writer;
use crate::layout_scene::{NodeKind, SceneFillLayer, SceneGradient, SceneNode, SceneStrokeAlign};
use crate::ImageBlendMode;
use std::fmt::Write as _;

/// The attributes that make an element stand for a node.
pub(super) struct LayerAttrs {
    /// ` id="…"` (and ` data-name`), empty for an unnamed part.
    pub id_attrs: String,
    pub xform: glam::Affine2,
    pub opacity: f32,
    /// The node's `mix-blend-mode` value; `None` for normal.
    pub blend: Option<&'static str>,
    pub filter: String,
}

impl LayerAttrs {
    /// Attributes of an unnamed part of a node's paint (a container's
    /// background) that only carries a filter.
    fn part(filter: String) -> Self {
        Self {
            id_attrs: String::new(),
            xform: glam::Affine2::IDENTITY,
            opacity: 1.0,
            blend: None,
            filter,
        }
    }

    /// The attribute string, with `extra` (the primitive's own transform,
    /// e.g. a path's fit into its box) composed inside the node transform
    /// and `own_opacity` folded into the layer opacity.
    pub(super) fn attrs_with(&self, extra: glam::Affine2, own_opacity: f32) -> String {
        let mut s = self.attrs_without_blend(extra, own_opacity);
        if let Some(blend) = self.blend {
            let _ = write!(s, r#" style="mix-blend-mode:{blend}""#);
        }
        s
    }

    /// [`Self::attrs_with`] with no transform or opacity of the primitive's own.
    pub(super) fn attrs(&self) -> String {
        self.attrs_with(glam::Affine2::IDENTITY, 1.0)
    }

    /// The attribute string without the blend, for an element that writes
    /// its own `style` (live text) and puts [`Self::blend`] in it.
    pub(super) fn attrs_without_blend(&self, extra: glam::Affine2, own_opacity: f32) -> String {
        let mut s = self.id_attrs.clone();
        let xform = self.xform * extra;
        if xform != glam::Affine2::IDENTITY {
            let _ = write!(s, r#" transform="{}""#, matrix_value(xform));
        }
        let opacity = (self.opacity * own_opacity).clamp(0.0, 1.0);
        if opacity < 1.0 {
            let _ = write!(s, r#" opacity="{opacity}""#);
        }
        s.push_str(&self.filter);
        s
    }
}

/// Whether `n` is written as a `<g>` of layers.
pub(super) fn is_container(n: &SceneNode) -> bool {
    n.kind == NodeKind::Group || !n.children.is_empty()
}

/// Whether the node is part of the exported design.
fn is_exported(n: &SceneNode) -> bool {
    !n.hidden && !crate::canvas_only::is_canvas_only(n)
}

/// Whether the node is written at all: part of the design, and not a group
/// left with nothing to hold.
pub(super) fn is_written(n: &SceneNode) -> bool {
    is_exported(n) && !(n.kind == NodeKind::Group && !n.children.iter().any(is_exported))
}

/// Whether the node clips its children (to `clip-…`).
pub(super) fn clips_children(n: &SceneNode) -> bool {
    n.clip_content && !n.children.is_empty() && has_area(n)
}

/// Whether a rect-shaped box's own paint is clipped to its rounded corners
/// (`clip-…`): an image or a fill stack in a box with a radius.
pub(super) fn box_clip(n: &SceneNode) -> bool {
    is_box(n) && n.corner_radius > 0.0 && has_area(n) && !matches!(box_paint(n), BoxPaint::Plain)
}

/// Rect, frame and the other box-shaped kinds; icons are glyphs, not boxes.
pub(super) fn is_box(n: &SceneNode) -> bool {
    matches!(
        n.kind,
        NodeKind::Rect | NodeKind::Frame | NodeKind::Other(_)
    ) && !is_icon(n)
}

fn has_area(n: &SceneNode) -> bool {
    n.bounds.size.x != 0.0 && n.bounds.size.y != 0.0
}

pub(super) fn emit_node(out: &mut String, n: &SceneNode, w: &mut Writer) {
    if !is_written(n) {
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
        id_attrs: w.id_attrs(n),
        xform: node_transform(n),
        opacity: composite,
        blend: css_blend(n.blend_mode),
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

/// The CSS `mix-blend-mode` for a node blend; `None` for normal.
pub(super) fn css_blend(mode: ImageBlendMode) -> Option<&'static str> {
    match mode {
        ImageBlendMode::Normal => None,
        // Same CSS as the HTML export (#14); `plus-darker` is WebKit-only.
        ImageBlendMode::LinearBurn => Some("plus-darker"),
        ImageBlendMode::LinearDodge => Some("plus-lighter"),
        other => blend_name(other),
    }
}

fn emit_container(out: &mut String, n: &SceneNode, attrs: &LayerAttrs, w: &mut Writer) {
    let _ = write!(out, "<g{}>", attrs.attrs());
    let own = LayerAttrs::part(shape_filter_attr(n, false));
    let defers_stroke = matches!(n.kind, NodeKind::Rect | NodeKind::Frame) && n.stroke.is_some();
    if is_box(n) {
        emit_box(out, n, &own, !defers_stroke, w);
    } else if n.kind != NodeKind::Group {
        emit_leaf(out, n, &own, w);
    }
    let clips = clips_children(n);
    if clips {
        let _ = write!(out, r#"<g clip-path="url(#clip-{})">"#, svg_id(&n.id));
    }
    for child in &n.children {
        emit_node(out, child, w);
    }
    if clips {
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
        NodeKind::Ellipse => emit_ellipse(out, n, &attrs.attrs()),
        NodeKind::Polygon => emit_polygon(out, n, &attrs.attrs()),
        NodeKind::Line => emit_line(out, n, &attrs.attrs()),
        NodeKind::Path => emit_path(out, n, attrs),
        NodeKind::Text => emit_text(out, n, attrs, w),
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
        [layer @ SceneFillLayer::Image { .. }] if layer.blend_mode() == ImageBlendMode::Normal => {
            BoxPaint::Image(BoxImage::of_layer(layer).expect("image layer"))
        }
        [SceneFillLayer::Solid { blend_mode, .. } | SceneFillLayer::Gradient { blend_mode, .. }]
            if *blend_mode == ImageBlendMode::Normal =>
        {
            BoxPaint::Plain
        }
        _ => BoxPaint::Stack,
    }
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
        BoxPaint::Plain if with_stroke => emit_rect(out, n, &attrs.attrs()),
        BoxPaint::Plain => emit_rect_fill(out, n, &attrs.attrs()),
        BoxPaint::Image(image) if !stroke => {
            let attrs = attrs.attrs_with(glam::Affine2::IDENTITY, image.opacity);
            emit_box_image(out, n, &image, &attrs, w);
        }
        BoxPaint::Image(image) => {
            let _ = write!(out, "<g{}>", attrs.attrs());
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
            let _ = write!(out, "<g{}>", attrs.attrs());
            let clip = box_clip(n).then(|| format!("clip-{}", svg_id(&n.id)));
            emit_fill_stack(out, n, clip.as_deref(), w);
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
