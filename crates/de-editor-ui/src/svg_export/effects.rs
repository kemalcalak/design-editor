//! Node effects as SVG filters, written in the shape Figma's own SVG export
//! uses (`BackgroundImageFix` → per-shadow `hardAlpha` chain → `shape`), so
//! Figma's import can turn them back into drop shadow, inner shadow and
//! layer blur effects.
//!
//! What the canvas casts is what the file casts: outer shadows only from
//! the kinds `casts_drop_shadow` paints them for (frame, rect, ellipse, from
//! the node's own box), inner shadows on filled shapes, layer blur over the
//! node's whole output. Blur radii are CSS radii, so `stdDeviation = r / 2`
//! as on the canvas. Background blur and SkSL effects have no SVG filter
//! Figma reads; they are reported.

use super::writer::Writer;
use super::{color_to_rgb_triplet, normalize_rect, svg_id};
use crate::layout_scene::{DropShadow, Effect, NodeKind, SceneNode};
use crate::{ImageBlendMode, Rect};
use std::fmt::Write as _;

/// Outer shadows the canvas casts for `n`.
fn outer_shadows(n: &SceneNode) -> impl Iterator<Item = &DropShadow> {
    let casts = matches!(n.kind, NodeKind::Frame | NodeKind::Rect | NodeKind::Ellipse);
    n.effects.iter().filter_map(move |effect| match effect {
        Effect::DropShadow(s) if casts && !s.inner && visible(s) => Some(s),
        _ => None,
    })
}

fn inner_shadows(n: &SceneNode) -> impl Iterator<Item = &DropShadow> {
    let shaped = matches!(
        n.kind,
        NodeKind::Frame | NodeKind::Rect | NodeKind::Ellipse | NodeKind::Polygon | NodeKind::Path
    );
    n.effects.iter().filter_map(move |effect| match effect {
        Effect::DropShadow(s) if shaped && s.inner && visible(s) => Some(s),
        _ => None,
    })
}

fn visible(s: &DropShadow) -> bool {
    s.color.a > 0.0 && (s.blur > 0.0 || s.offset_x != 0.0 || s.offset_y != 0.0)
}

/// The layer blur radius, when the node has a visible one.
pub(super) fn layer_blur(n: &SceneNode) -> Option<f32> {
    n.effects.iter().find_map(|effect| match effect {
        Effect::Blur(b) if b.radius > 0.0 => Some(b.radius),
        _ => None,
    })
}

/// Whether the node's own shape carries a shadow filter.
pub(super) fn has_shape_shadows(n: &SceneNode) -> bool {
    outer_shadows(n).next().is_some() || inner_shadows(n).next().is_some()
}

/// ` filter="url(#filter-…)"` for the node's own shape: its shadows, and its
/// layer blur too when `with_blur` (a leaf, whose shape is its whole output).
pub(super) fn shape_filter_attr(n: &SceneNode, with_blur: bool) -> String {
    if has_shape_shadows(n) || (with_blur && layer_blur(n).is_some()) {
        format!(r#" filter="url(#filter-{})""#, svg_id(&n.id))
    } else {
        String::new()
    }
}

/// ` filter="url(#blur-…)"` for a container's group, whose children are
/// blurred with it.
pub(super) fn group_blur_attr(n: &SceneNode) -> String {
    if layer_blur(n).is_some() {
        format!(r#" filter="url(#blur-{})""#, svg_id(&n.id))
    } else {
        String::new()
    }
}

/// Defs for `n`'s effects, plus a warning for each one the file cannot
/// carry. `container` mirrors how the node is written: a container's shape
/// filter holds only its shadows and its blur filter sits on the group.
pub(super) fn emit_effect_defs(out: &mut String, n: &SceneNode, container: bool, w: &mut Writer) {
    report_unsupported(n, w);
    let id = svg_id(&n.id);
    let blur = layer_blur(n);
    let shape_blur = if container { None } else { blur };
    if has_shape_shadows(n) || shape_blur.is_some() {
        emit_filter(out, &format!("filter-{id}"), n, true, shape_blur, n.bounds);
    }
    if container {
        if let Some(radius) = blur {
            emit_filter(
                out,
                &format!("blur-{id}"),
                n,
                false,
                Some(radius),
                n.aggregate_bounds(),
            );
        }
    }
}

fn report_unsupported(n: &SceneNode, w: &mut Writer) {
    for effect in &n.effects {
        match effect {
            Effect::BackgroundBlur { radius } if *radius > 0.0 => {
                w.warn(n, "background blur has no SVG equivalent and is left out")
            }
            Effect::Shader(_) => w.warn(n, "shader effect (SkSL) is left out"),
            Effect::DropShadow(s) if visible(s) => {
                if s.blend_mode != ImageBlendMode::Normal && blend_name(s.blend_mode).is_none() {
                    w.warn(n, "shadow blend mode is exported as normal");
                }
            }
            _ => {}
        }
    }
    let skipped_outer = n
        .effects
        .iter()
        .any(|e| matches!(e, Effect::DropShadow(s) if !s.inner && visible(s)))
        && outer_shadows(n).next().is_none()
        && n.kind != NodeKind::Text;
    if skipped_outer {
        // The canvas does not cast these either; say so instead of
        // silently differing from the file's effect list.
        w.warn(
            n,
            "drop shadow on this shape kind is not painted, so it is left out",
        );
    }
}

fn emit_filter(
    out: &mut String,
    id: &str,
    n: &SceneNode,
    shadows: bool,
    blur: Option<f32>,
    area: Rect,
) {
    let region = filter_region(n, shadows, blur, area);
    let _ = write!(
        out,
        r#"<filter id="{id}" x="{}" y="{}" width="{}" height="{}" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">"#,
        region.origin.x, region.origin.y, region.size.x, region.size.y,
    );
    out.push_str(r#"<feFlood flood-opacity="0" result="BackgroundImageFix"/>"#);
    let mut previous = "BackgroundImageFix".to_string();
    let mut index = 1;
    if shadows {
        for shadow in outer_shadows(n) {
            emit_shadow_core(out, shadow);
            out.push_str(r#"<feComposite in2="hardAlpha" operator="out"/>"#);
            emit_shadow_color(out, shadow);
            let result = format!("effect{index}_dropShadow");
            let _ = write!(
                out,
                r#"<feBlend mode="{}" in2="{previous}" result="{result}"/>"#,
                blend_name(shadow.blend_mode).unwrap_or("normal")
            );
            previous = result;
            index += 1;
        }
    }
    let _ = write!(
        out,
        r#"<feBlend mode="normal" in="SourceGraphic" in2="{previous}" result="shape"/>"#
    );
    previous = "shape".into();
    if shadows {
        for shadow in inner_shadows(n) {
            emit_shadow_core(out, shadow);
            out.push_str(r#"<feComposite in2="hardAlpha" operator="arithmetic" k2="-1" k3="1"/>"#);
            emit_shadow_color(out, shadow);
            let result = format!("effect{index}_innerShadow");
            let _ = write!(
                out,
                r#"<feBlend mode="{}" in2="{previous}" result="{result}"/>"#,
                blend_name(shadow.blend_mode).unwrap_or("normal")
            );
            previous = result;
            index += 1;
        }
    }
    if let Some(radius) = blur {
        let _ = write!(
            out,
            r#"<feGaussianBlur in="{previous}" stdDeviation="{}" result="effect{index}_foregroundBlur"/>"#,
            radius * 0.5
        );
    }
    out.push_str("</filter>");
}

fn emit_shadow_core(out: &mut String, shadow: &DropShadow) {
    out.push_str(r#"<feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha"/>"#);
    let _ = write!(
        out,
        r#"<feOffset dx="{}" dy="{}"/>"#,
        shadow.offset_x, shadow.offset_y
    );
    if shadow.blur > 0.0 {
        let _ = write!(
            out,
            r#"<feGaussianBlur stdDeviation="{}"/>"#,
            shadow.blur * 0.5
        );
    }
}

fn emit_shadow_color(out: &mut String, shadow: &DropShadow) {
    let (r, g, b) = color_to_rgb_triplet(shadow.color);
    let _ = write!(
        out,
        r#"<feColorMatrix type="matrix" values="0 0 0 0 {r} 0 0 0 0 {g} 0 0 0 0 {b} 0 0 0 {} 0"/>"#,
        shadow.color.a.clamp(0.0, 1.0)
    );
}

/// The filter's paint area: the node's box grown by every shadow's reach
/// (offset plus three sigma) and the blur's spread.
fn filter_region(n: &SceneNode, shadows: bool, blur: Option<f32>, area: Rect) -> Rect {
    let r = normalize_rect(area);
    let (mut left, mut top, mut right, mut bottom) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    if shadows {
        for s in outer_shadows(n) {
            let spread = s.blur * 1.5;
            left = left.max(spread - s.offset_x);
            right = right.max(spread + s.offset_x);
            top = top.max(spread - s.offset_y);
            bottom = bottom.max(spread + s.offset_y);
        }
    }
    if let Some(radius) = blur {
        let spread = radius * 1.5;
        left = left.max(spread);
        right = right.max(spread);
        top = top.max(spread);
        bottom = bottom.max(spread);
    }
    Rect::xywh(
        r.origin.x - left,
        r.origin.y - top,
        r.size.x + left + right,
        r.size.y + top + bottom,
    )
}

/// The `feBlend` / CSS name of a blend mode; `None` where SVG filters have
/// no such mode (linear burn / dodge).
pub(super) fn blend_name(mode: ImageBlendMode) -> Option<&'static str> {
    Some(match mode {
        ImageBlendMode::Normal => "normal",
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
        ImageBlendMode::LinearBurn | ImageBlendMode::LinearDodge => return None,
    })
}
