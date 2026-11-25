//! A node's canonical fill stack (#179) as SVG: the layers back to front,
//! each in the node box with its own `mix-blend-mode`, inside an isolated
//! group clipped to the node's silhouette (the glyphs for text, the rounded
//! box for rects and frames).
//!
//! Approximations: a shader layer writes its fallback colour (SVG cannot
//! run SkSL), an image layer ignores colour adjustments and a crop
//! transform (Crop paints like Fill). Each is reported in `warnings`.

use super::image::{emit_image_pattern, preserve_aspect_ratio};
use super::writer::Writer;
use super::{color_to_rgb, emit_gradient, normalize_rect, svg_id, xml_escape};
use crate::layout_scene::{NodeKind, SceneFillLayer, SceneImageFit, SceneNode};
use crate::{Color, ImageAdjustments, ImageBlendMode, Rect};
use std::fmt::Write as _;
use std::sync::Arc;

/// Defs the stack's layers reference: one gradient per gradient layer and
/// one pattern per tiled image layer.
pub(super) fn emit_fill_stack_defs(out: &mut String, n: &SceneNode, id: &str, w: &mut Writer) {
    let bounds = normalize_rect(n.bounds);
    for (index, layer) in n.fill_layers.iter().enumerate() {
        let layer_id = layer_def_id(id, index);
        match layer {
            SceneFillLayer::Gradient { gradient, .. } => {
                emit_gradient(out, &format!("gradient-{layer_id}"), gradient, bounds);
            }
            SceneFillLayer::Image {
                src,
                src_id,
                fit: SceneImageFit::Tile,
                ..
            } => {
                let href = w.image_href(n, src, *src_id);
                emit_image_pattern(out, &image_node(n, layer), &href, &layer_id);
            }
            _ => {}
        }
    }
}

/// The stack itself, clipped by `clip_id` when given, with `attrs` (layer id and the
/// like) on its group.
pub(super) fn emit_fill_stack(
    out: &mut String,
    n: &SceneNode,
    clip_id: Option<&str>,
    attrs: &str,
    w: &mut Writer,
) {
    let id = svg_id(&n.id);
    let opacity = if n.opacity.is_finite() {
        n.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let _ = write!(out, r#"<g{attrs}"#);
    if let Some(clip_id) = clip_id {
        let _ = write!(out, r#" clip-path="url(#{clip_id})""#);
    }
    out.push_str(r#" style="isolation:isolate""#);
    if opacity < 1.0 {
        let _ = write!(out, r#" opacity="{opacity}""#);
    }
    out.push('>');
    let r = normalize_rect(n.bounds);
    // Canonical fills are front-to-back; SVG paints in document order.
    for (index, layer) in n.fill_layers.iter().enumerate().rev() {
        let blend = blend_style(layer.blend_mode());
        let layer_id = layer_def_id(&id, index);
        match layer {
            SceneFillLayer::Solid { color, .. } => emit_solid_layer(out, r, *color, &blend),
            SceneFillLayer::Shader { shader, .. } => {
                w.warn(n, "shader fill (SkSL) is exported as its fallback colour");
                let color = Color {
                    a: shader.fallback.a * shader.opacity.clamp(0.0, 1.0),
                    ..shader.fallback
                };
                emit_solid_layer(out, r, color, &blend);
            }
            SceneFillLayer::Gradient { .. } => {
                let _ = write!(
                    out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="url(#gradient-{layer_id})"{blend}/>"#,
                    r.origin.x, r.origin.y, r.size.x, r.size.y,
                );
            }
            SceneFillLayer::Image {
                src,
                src_id,
                fit,
                opacity,
                adjustments,
                ..
            } => {
                if *adjustments != ImageAdjustments::default() {
                    w.warn(n, "image colour adjustments are left out");
                }
                let href = w.image_href(n, src, *src_id);
                emit_image_layer(out, r, &href, *fit, *opacity, &layer_id, &blend);
            }
        }
    }
    out.push_str("</g>");
}

fn emit_solid_layer(out: &mut String, r: Rect, color: Color, blend: &str) {
    let _ = write!(
        out,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}""#,
        r.origin.x,
        r.origin.y,
        r.size.x,
        r.size.y,
        color_to_rgb(color),
    );
    if color.a < 0.999 {
        let _ = write!(out, r#" fill-opacity="{}""#, color.a);
    }
    let _ = write!(out, "{blend}/>");
}

fn emit_image_layer(
    out: &mut String,
    r: Rect,
    href: &str,
    fit: SceneImageFit,
    opacity: f32,
    layer_id: &str,
    blend: &str,
) {
    let opacity = opacity.clamp(0.0, 1.0);
    let preserve = match fit {
        SceneImageFit::Tile => {
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="url(#image-pattern-{layer_id})" opacity="{opacity}"{blend}/>"#,
                r.origin.x, r.origin.y, r.size.x, r.size.y,
            );
            return;
        }
        fit => preserve_aspect_ratio(fit),
    };
    let _ = write!(
        out,
        r#"<image x="{}" y="{}" width="{}" height="{}" xlink:href="{}" preserveAspectRatio="{preserve}" opacity="{opacity}"{blend}/>"#,
        r.origin.x,
        r.origin.y,
        r.size.x,
        r.size.y,
        xml_escape(href),
    );
}

/// Def id stem shared by a layer's gradient / pattern def and its use;
/// `id` is the hex `svg_id`, so it cannot itself contain `-fill-`.
fn layer_def_id(id: &str, index: usize) -> String {
    format!("{id}-fill-{index}")
}

/// A stand-in image node carrying one image layer, for the shared tile
/// pattern writer (which sizes the cell from the source's dimensions).
fn image_node(n: &SceneNode, layer: &SceneFillLayer) -> SceneNode {
    let mut image = SceneNode::leaf(n.id.as_str(), NodeKind::Other("image".into()));
    image.bounds = n.bounds;
    if let SceneFillLayer::Image { src, src_id, .. } = layer {
        image.image_src = Some(Arc::clone(src));
        image.image_src_id = *src_id;
    }
    image
}

/// `style="mix-blend-mode:…"`, empty for Normal.
pub(super) fn blend_style(mode: ImageBlendMode) -> String {
    let css = match mode {
        ImageBlendMode::Normal => return String::new(),
        // Same CSS as the HTML export (#14); `plus-darker` is WebKit-only.
        ImageBlendMode::LinearBurn => "plus-darker",
        ImageBlendMode::LinearDodge => "plus-lighter",
        other => super::effects::blend_name(other).unwrap_or("normal"),
    };
    format!(r#" style="mix-blend-mode:{css}""#)
}
