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
use crate::widgets::icon_family::{
    resolve_icon_glyph, IconGlyph, FALLBACK_ICON_D, ICON_DEFAULT_TINT, ICON_GLYPH_UNITS,
};
use std::fmt::Write as _;

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
    let glyph = resolve_icon_glyph(&n.font_family, name, n.font_weight).unwrap_or_else(|| {
        w.warn(
            n,
            &format!("icon {name:?} is not in the icon catalog; exported as a dot"),
        );
        IconGlyph {
            viewbox: ICON_GLYPH_UNITS,
            paths: vec![(FALLBACK_ICON_D.to_string(), IconRenderStyle::Stroke)],
            outline_units: 0.0,
        }
    });
    let IconGlyph {
        viewbox,
        paths,
        outline_units,
    } = glyph;
    let fit = glam::Affine2::from_scale_angle_translation(
        glam::Vec2::splat(size / viewbox),
        0.0,
        glam::Vec2::new(
            rect.origin.x + (rect.size.x - size) * 0.5,
            rect.origin.y + (rect.size.y - size) * 0.5,
        ),
    );
    let tint = n.fill.unwrap_or(ICON_DEFAULT_TINT);
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
                    outline_units * viewbox / ICON_GLYPH_UNITS
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
    // A filter (layer blur) reads its region in the element's user space,
    // so with one the fit goes on a child rather than on the layer.
    if let ([(d, style)], true) = (paths.as_slice(), attrs.filter.is_empty()) {
        // One path: the path itself is the layer.
        let _ = write!(
            out,
            r#"<path{} d="{}"{}/>"#,
            attrs.attrs_with(fit, alpha),
            xml_escape(d),
            paint_of(*style)
        );
        return;
    }
    let (outer, inner) = if attrs.filter.is_empty() {
        (attrs.attrs_with(fit, alpha), String::new())
    } else {
        (
            attrs.attrs_with(glam::Affine2::IDENTITY, alpha),
            format!(r#" transform="{}""#, super::transform::matrix_value(fit)),
        )
    };
    let _ = write!(out, "<g{outer}>");
    if !inner.is_empty() {
        let _ = write!(out, "<g{inner}>");
    }
    for (d, style) in &paths {
        let _ = write!(out, r#"<path d="{}"{}/>"#, xml_escape(d), paint_of(*style));
    }
    if !inner.is_empty() {
        out.push_str("</g>");
    }
    out.push_str("</g>");
}
