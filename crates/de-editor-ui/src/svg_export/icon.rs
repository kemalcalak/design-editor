//! `icon_font` nodes as vector paths, so an icon arrives in Figma as an
//! editable vector named after the layer instead of a filled box.
//!
//! The glyph sits centred in a square of `min(w, h)`, scaled from its
//! catalog viewBox, stroked at `viewbox / 12` user units (the canvas's
//! `(size / 24) * 2`), with the same lookup ladder as the canvas and the
//! HTML export: the Iconify catalog first, then the built-in lucide table,
//! then lucide's dot for an unknown name (reported).

use super::node::LayerAttrs;
use super::writer::Writer;
use super::{color_to_rgb, normalize_rect, xml_escape};
use crate::layout_scene::SceneNode;
use crate::widgets::icon_catalog::IconRenderStyle;
use crate::widgets::icon_family::{canonical_family, lookup_icon_weighted};
use crate::widgets::icons::Icon;
use crate::Color;
use std::fmt::Write as _;

/// Lucide's dot, what the canvas strokes for an unknown glyph name.
const FALLBACK_ICON_D: &str = "M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0";
const LUCIDE_VIEWBOX: f32 = 24.0;
/// Painter default icon tint when the node authored no fill.
const ICON_DEFAULT_FILL: Color = Color {
    r: 0.39,
    g: 0.45,
    b: 0.55,
    a: 1.0,
};

pub(super) fn is_icon(n: &SceneNode) -> bool {
    matches!(&n.kind, crate::layout_scene::NodeKind::Other(tag) if tag == "icon_font")
}

pub(super) fn emit_icon(out: &mut String, n: &SceneNode, attrs: &LayerAttrs, w: &mut Writer) {
    let rect = normalize_rect(n.bounds);
    let size = rect.size.x.min(rect.size.y);
    if size <= 0.0 {
        return;
    }
    let name = n.text.as_deref().unwrap_or("");
    let family = canonical_family(&n.font_family);
    let Some((viewbox, paths, outline_units)) = resolve_glyph(&family, name, n.font_weight) else {
        w.warn(
            n,
            &format!("icon {name:?} is not in the icon catalog; exported as a dot"),
        );
        return emit_paths(
            out,
            n,
            attrs,
            rect,
            size,
            LUCIDE_VIEWBOX,
            &[(FALLBACK_ICON_D.to_string(), IconRenderStyle::Stroke)],
            0.0,
        );
    };
    emit_paths(out, n, attrs, rect, size, viewbox, &paths, outline_units);
}

#[allow(clippy::too_many_arguments)]
fn emit_paths(
    out: &mut String,
    n: &SceneNode,
    attrs: &LayerAttrs,
    rect: crate::Rect,
    size: f32,
    viewbox: f32,
    paths: &[(String, IconRenderStyle)],
    outline_units: f32,
) {
    let scale = size / viewbox;
    let fit = glam::Affine2::from_scale_angle_translation(
        glam::Vec2::splat(scale),
        0.0,
        glam::Vec2::new(
            rect.origin.x + (rect.size.x - size) * 0.5,
            rect.origin.y + (rect.size.y - size) * 0.5,
        ),
    );
    let tint = n.fill.unwrap_or(ICON_DEFAULT_FILL);
    // The tint's alpha rides on the layer opacity, so paths stay opaque.
    let alpha = tint.a.clamp(0.0, 1.0);
    let paint_of = |style: IconRenderStyle| -> String {
        let mut s = String::new();
        match style {
            IconRenderStyle::Fill if outline_units > 0.0 => {
                let _ = write!(
                    s,
                    r#" fill="{0}" stroke="{0}" stroke-width="{1}" stroke-linejoin="round""#,
                    color_to_rgb(tint),
                    outline_units * viewbox / 24.0
                );
            }
            IconRenderStyle::Fill => {
                let _ = write!(s, r#" fill="{}""#, color_to_rgb(tint));
            }
            IconRenderStyle::Stroke => {
                let _ = write!(
                    s,
                    r#" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#,
                    color_to_rgb(tint),
                    viewbox / 12.0
                );
            }
        }
        s
    };
    if let [(d, style)] = paths {
        // One path: the path itself is the layer.
        let _ = write!(
            out,
            r#"<path{} d="{}"{}/>"#,
            attrs.render(fit, alpha),
            xml_escape(d),
            paint_of(*style)
        );
        return;
    }
    let _ = write!(out, "<g{}>", attrs.render(fit, alpha));
    for (d, style) in paths {
        let _ = write!(out, r#"<path d="{}"{}/>"#, xml_escape(d), paint_of(*style));
    }
    out.push_str("</g>");
}

/// The glyph's viewBox extent, its `d` strings and the weight outline;
/// `None` for a name no table knows.
fn resolve_glyph(
    family: &str,
    name: &str,
    weight: u16,
) -> Option<(f32, Vec<(String, IconRenderStyle)>, f32)> {
    if let Some(icon) = lookup_icon_weighted(family, name, weight) {
        let entry = icon.entry;
        let viewbox = entry.width.max(entry.height).max(1.0);
        return Some((
            viewbox,
            vec![(entry.d.clone(), entry.style)],
            icon.outline_units,
        ));
    }
    if family == "lucide" {
        if let Some(icon) = Icon::from_name(name) {
            let paths = icon
                .paths()
                .iter()
                .map(|d| ((*d).to_string(), IconRenderStyle::Stroke))
                .collect();
            return Some((LUCIDE_VIEWBOX, paths, 0.0));
        }
    }
    None
}
