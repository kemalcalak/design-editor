//! Image paints in SVG export: an image node, or a rect / frame whose fill
//! is one image, is an `<image>` (embedded as a `data:` URL when the
//! export asks for it), clipped to the node's rounded box; a tiled image is
//! a repeating `<pattern>`.

use super::image_metadata;
use super::writer::Writer;
use super::{normalize_rect, svg_id, xml_escape};
use crate::layout_scene::{SceneFillLayer, SceneImageFit, SceneNode};
use crate::ImageAdjustments;
use std::fmt::Write as _;

/// One image painting a node's box.
pub(super) struct BoxImage<'a> {
    pub src: &'a str,
    pub src_id: u64,
    pub fit: SceneImageFit,
    pub opacity: f32,
    pub cropped: bool,
    pub adjusted: bool,
}

impl<'a> BoxImage<'a> {
    /// The image of an image node (`image_src`).
    pub(super) fn of_node(n: &'a SceneNode) -> Option<Self> {
        Some(Self {
            src: n.image_src.as_deref()?,
            src_id: n.image_src_id,
            fit: n.image_fit,
            opacity: n.opacity,
            cropped: n.image_transform.is_some(),
            adjusted: n.image_adjustments != ImageAdjustments::default(),
        })
    }

    /// The image of a one-layer image fill.
    pub(super) fn of_layer(layer: &'a SceneFillLayer) -> Option<Self> {
        let SceneFillLayer::Image {
            src,
            src_id,
            fit,
            transform,
            adjustments,
            opacity,
            ..
        } = layer
        else {
            return None;
        };
        Some(Self {
            src,
            src_id: *src_id,
            fit: *fit,
            opacity: *opacity,
            cropped: transform.is_some(),
            adjusted: *adjustments != ImageAdjustments::default(),
        })
    }
}

/// Write `image` over `n`'s box with `attrs` (`opacity` is the image's own
/// and is folded into `attrs` by the caller). Rounded boxes reference the
/// node's `clip-…` def.
pub(super) fn emit_box_image(
    out: &mut String,
    n: &SceneNode,
    image: &BoxImage,
    attrs: &str,
    w: &mut Writer,
) {
    if image.cropped && image.fit == SceneImageFit::Crop {
        w.warn(
            n,
            "image crop position is not kept; the image fills its box",
        );
    }
    if image.adjusted {
        w.warn(n, "image colour adjustments are left out");
    }
    let r = normalize_rect(n.bounds);
    let id = svg_id(&n.id);
    let clip = if n.corner_radius > 0.0 {
        format!(r#" clip-path="url(#clip-{id})""#)
    } else {
        String::new()
    };
    if image.fit == SceneImageFit::Tile {
        let _ = write!(
            out,
            r#"<rect{attrs} x="{}" y="{}" width="{}" height="{}" fill="url(#image-pattern-{id})"{clip}/>"#,
            r.origin.x, r.origin.y, r.size.x, r.size.y,
        );
        return;
    }
    let href = w.image_href(n, image.src, image.src_id);
    let _ = write!(
        out,
        r#"<image{attrs} x="{}" y="{}" width="{}" height="{}" xlink:href="{}" preserveAspectRatio="{}"{clip}/>"#,
        r.origin.x,
        r.origin.y,
        r.size.x,
        r.size.y,
        xml_escape(&href),
        preserve_aspect_ratio(image.fit),
    );
}

/// The tile pattern def a tiled box image references.
pub(super) fn emit_box_image_pattern(
    out: &mut String,
    n: &SceneNode,
    image: &BoxImage,
    w: &mut Writer,
) {
    if image.fit != SceneImageFit::Tile {
        return;
    }
    let href = w.image_href(n, image.src, image.src_id);
    let mut cell = SceneNode::leaf(n.id.as_str(), n.kind.clone());
    cell.bounds = n.bounds;
    cell.image_src = Some(image.src.into());
    cell.image_src_id = image.src_id;
    emit_image_pattern(out, &cell, &href, &svg_id(&n.id));
}

/// `preserveAspectRatio` for an image fit. Tile never reaches it (it writes
/// a pattern); it maps like Fill only to keep the match total.
pub(super) fn preserve_aspect_ratio(fit: SceneImageFit) -> &'static str {
    match fit {
        SceneImageFit::Fit => "xMidYMid meet",
        SceneImageFit::Crop | SceneImageFit::Fill | SceneImageFit::Tile => "xMidYMid slice",
        SceneImageFit::Stretch => "none",
    }
}

/// A repeating pattern of `n.image_src` (its intrinsic size sets the cell)
/// written with `href`, under `image-pattern-{id}`.
pub(super) fn emit_image_pattern(out: &mut String, n: &SceneNode, href: &str, id: &str) {
    let r = normalize_rect(n.bounds);
    // If source bytes are unavailable (for example an uncached remote URL),
    // fall back to one bounds-sized cell instead of inventing a repeat scale.
    let (tile_w, tile_h) = n
        .image_src
        .as_deref()
        .and_then(|src| image_metadata::intrinsic_dimensions(n, src))
        .unwrap_or((r.size.x.max(1.0), r.size.y.max(1.0)));
    let start_x = r.origin.x + (r.size.x - tile_w) * 0.5;
    let start_y = r.origin.y + (r.size.y - tile_h) * 0.5;
    let _ = write!(
        out,
        r#"<pattern id="image-pattern-{id}" patternUnits="userSpaceOnUse" x="{start_x}" y="{start_y}" width="{tile_w}" height="{tile_h}" viewBox="0 0 {tile_w} {tile_h}"><image width="{tile_w}" height="{tile_h}" xlink:href="{}" preserveAspectRatio="none"/></pattern>"#,
        xml_escape(href),
    );
}
