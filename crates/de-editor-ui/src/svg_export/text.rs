//! Text nodes in SVG export. Plain text writes `<text>` lines in the node's
//! solid fill. Text whose fill stack is richer than one solid (#179, the
//! same test the canvas uses, `text_fill_stack`) writes its glyphs into a
//! `<clipPath>` and paints the fill stack inside it, back to front, each
//! layer with its own `mix-blend-mode` in an isolated group, so a photo or
//! gradient shows through the letters.
//!
//! Approximations: a shader layer writes its fallback colour (SVG cannot
//! run SkSL), an image layer ignores colour adjustments and a crop
//! transform (Crop paints like Fill), as the rect writer does, and text
//! decorations drop out of the clip in most SVG renderers.

use super::{
    color_to_rgb, emit_gradient, emit_image_pattern, normalize_rect, preserve_aspect_ratio, svg_id,
    xml_escape,
};
use crate::layout_scene::{NodeKind, SceneFillLayer, SceneImageFit, SceneNode};
use crate::text_fill_stack::text_needs_fill_stack;
use crate::{Color, ImageBlendMode, Rect};
use std::fmt::Write as _;
use std::sync::Arc;

const TEXT_DEFAULT_FONT_SIZE: f32 = 13.0;
const TEXT_DEFAULT_FILL: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.08,
    a: 1.0,
};

pub(super) fn emit_text(out: &mut String, n: &SceneNode) {
    let Some(text) = n.text.as_deref().filter(|t| !t.is_empty()) else {
        return;
    };
    let _ = write!(out, r#"<g id="{}">"#, xml_escape(&n.id));
    if text_needs_fill_stack(n) {
        emit_fill_stack(out, n);
    } else {
        let color = n.fill.unwrap_or(TEXT_DEFAULT_FILL);
        let fill_attr = if color.a < 0.999 {
            format!(
                r#" fill="{}" fill-opacity="{}""#,
                color_to_rgb(color),
                color.a
            )
        } else {
            format!(r#" fill="{}""#, color_to_rgb(color))
        };
        emit_text_lines(out, n, text, &fill_attr);
    }
    out.push_str("</g>");
}

/// Defs a fill-stacked text node references: the glyph clip, one gradient
/// per gradient layer and one pattern per tiled image layer.
pub(super) fn emit_text_fill_defs(out: &mut String, n: &SceneNode, id: &str) {
    let Some(text) = n.text.as_deref().filter(|t| !t.is_empty()) else {
        return;
    };
    if !text_needs_fill_stack(n) {
        return;
    }
    let _ = write!(out, r#"<clipPath id="text-clip-{id}">"#);
    emit_text_lines(out, n, text, "");
    out.push_str("</clipPath>");
    let bounds = normalize_rect(n.bounds);
    for (index, layer) in n.fill_layers.iter().enumerate() {
        let layer_id = layer_def_id(id, index);
        match layer {
            SceneFillLayer::Gradient { gradient, .. } => {
                emit_gradient(out, &format!("gradient-{layer_id}"), gradient, bounds);
            }
            SceneFillLayer::Image {
                src,
                fit: SceneImageFit::Tile,
                ..
            } => {
                emit_image_pattern(out, &image_node(n, layer), src, &layer_id);
            }
            _ => {}
        }
    }
}

fn emit_fill_stack(out: &mut String, n: &SceneNode) {
    let id = svg_id(&n.id);
    let opacity = if n.opacity.is_finite() {
        n.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let _ = write!(
        out,
        r#"<g clip-path="url(#text-clip-{id})" style="isolation:isolate""#
    );
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
                src, fit, opacity, ..
            } => emit_image_layer(out, r, src, *fit, *opacity, &layer_id, &blend),
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
    src: &str,
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
        r#"<image x="{}" y="{}" width="{}" height="{}" href="{}" preserveAspectRatio="{preserve}" opacity="{opacity}"{blend}/>"#,
        r.origin.x,
        r.origin.y,
        r.size.x,
        r.size.y,
        xml_escape(src),
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
fn blend_style(mode: ImageBlendMode) -> String {
    let css = match mode {
        ImageBlendMode::Normal => return String::new(),
        ImageBlendMode::Darken => "darken",
        ImageBlendMode::Multiply => "multiply",
        ImageBlendMode::Screen => "screen",
        ImageBlendMode::Overlay => "overlay",
        ImageBlendMode::Lighten => "lighten",
        ImageBlendMode::Difference => "difference",
        ImageBlendMode::Hue => "hue",
        ImageBlendMode::Saturation => "saturation",
        ImageBlendMode::Color => "color",
        ImageBlendMode::Luminosity => "luminosity",
        ImageBlendMode::SoftLight => "soft-light",
        ImageBlendMode::ColorDodge => "color-dodge",
        ImageBlendMode::ColorBurn => "color-burn",
        ImageBlendMode::HardLight => "hard-light",
        ImageBlendMode::Exclusion => "exclusion",
    };
    format!(r#" style="mix-blend-mode:{css}""#)
}

/// One `<text>` per authored line, with `fill_attr` (empty inside a clip).
fn emit_text_lines(out: &mut String, n: &SceneNode, text: &str, fill_attr: &str) {
    let r = normalize_rect(n.bounds);
    let base_size = if n.font_size > 0.0 {
        n.font_size
    } else {
        TEXT_DEFAULT_FONT_SIZE
    };
    let line_h = base_size * 1.35;
    let weight_attr = if n.font_weight > 0 {
        format!(r#" font-weight="{}""#, n.font_weight)
    } else {
        String::new()
    };
    let mut baseline_y = r.origin.y + base_size + 1.0;
    for line in text.split('\n') {
        let _ = write!(
            out,
            r#"<text x="{}" y="{baseline_y}" font-family="{}" font-size="{base_size}"{weight_attr}{fill_attr}{}{}>{}</text>"#,
            r.origin.x,
            xml_escape(if n.font_family.is_empty() {
                "system-ui, sans-serif"
            } else {
                &n.font_family
            }),
            if n.italic {
                r#" font-style="italic""#
            } else {
                ""
            },
            text_decoration_attr(n),
            xml_escape(line),
        );
        baseline_y += line_h;
    }
}

fn text_decoration_attr(n: &SceneNode) -> &'static str {
    match (n.underline, n.strikethrough) {
        (true, true) => r#" text-decoration="underline line-through""#,
        (true, false) => r#" text-decoration="underline""#,
        (false, true) => r#" text-decoration="line-through""#,
        (false, false) => "",
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
