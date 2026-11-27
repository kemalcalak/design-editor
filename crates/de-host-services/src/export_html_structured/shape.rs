//! Box emission for the container / rectangle / ellipse family.
//!
//! These become `<div>`s. The div is opened before the node's children
//! and closed after them, so the DOM nests the way the scene nests and
//! `overflow` / `opacity` / `transform` scope over the subtree the way
//! the painter's save-layers do.
//!
//! Nesting has one hazard worth stating: an absolutely positioned child
//! resolves against its ancestor's PADDING box, so a border on a
//! container would shift every descendant inward by the border width.
//! Strokes are therefore drawn as a dedicated overlay element appended
//! after the children rather than as a border on the container itself —
//! which also lets the overlay honour all three stroke alignments by
//! insetting or outsetting itself, something a CSS border (always
//! inside) cannot do.

use de_editor_ui::layout_scene::{
    SceneFillType, SceneImageFit, SceneNode, SceneStroke, SceneStrokeAlign,
};
use de_editor_ui::Rect;
use de_util::xml_escape::escape_html;
use std::fmt::Write as _;

use super::css;

/// Open the node's box. `ellipse` rounds it to a full oval.
pub fn open(out: &mut String, n: &SceneNode, rect: Rect, ellipse: bool) {
    // Smoothed corners (#389): the background paints in its own layer
    // clipped to the outline, and a clipping container clips its
    // children in an inner wrapper (`clip_wrapper`), so the box's
    // shadow, filters and an outside / centre stroke stay unclipped.
    // The box keeps the circular radius only so a box-shadow follows it.
    let outline = outline_clip(n, ellipse);
    let mut style = String::new();
    css::place(&mut style, rect);
    if outline.is_none() {
        write_background(&mut style, n, rect);
    }
    if ellipse {
        css::decl(&mut style, "border-radius", "50%");
    } else if let Some(radius) = css::corner_radius(n) {
        css::decl(&mut style, "border-radius", &radius);
    }
    if outline.is_none() && n.clip_content {
        css::decl(&mut style, "overflow", "hidden");
    }
    // `SceneNode::opacity` is already baked into every resolved fill,
    // stroke and shadow alpha at scene-build time. Only
    // `composite_opacity` — the alpha applied once to an isolated
    // subtree — belongs on the element, or translucency would be
    // applied twice.
    if n.composite_opacity < 1.0 && n.composite_opacity.is_finite() {
        css::decl(&mut style, "opacity", &css::ratio(n.composite_opacity));
    }
    if let Some(transform) = css::transform(n) {
        css::decl(&mut style, "transform", &transform);
        // Rotation is about the bounds centre in the scene model.
        css::decl(&mut style, "transform-origin", "50% 50%");
    }
    if let Some(blend) = css::blend_mode(n.blend_mode) {
        css::decl(&mut style, "mix-blend-mode", blend);
    }
    let effects = css::effects(&n.effects);
    if !effects.box_shadow.is_empty() {
        css::decl(&mut style, "box-shadow", &effects.box_shadow);
    }
    if !effects.filter.is_empty() {
        css::decl(&mut style, "filter", &effects.filter);
    }
    if !effects.backdrop_filter.is_empty() {
        css::decl(&mut style, "backdrop-filter", &effects.backdrop_filter);
    }

    let _ = write!(out, r#"<div class="n" style="{style}">"#);
    if let Some(clip) = &outline {
        let mut layer = String::new();
        css::place(&mut layer, Rect::xywh(0.0, 0.0, rect.size.x, rect.size.y));
        write_background(&mut layer, n, rect);
        css::decl(&mut layer, "clip-path", clip);
        let _ = write!(out, r#"<div class="n" style="{layer}"></div>"#);
        // The same predicate `close` reads, so the two never drift.
        if clip_wrapper(n, ellipse) {
            // The wrapper sits on the box's own origin, so children keep
            // the coordinates they resolve against the box.
            let mut wrapper = String::new();
            css::place(&mut wrapper, Rect::xywh(0.0, 0.0, rect.size.x, rect.size.y));
            css::decl(&mut wrapper, "clip-path", clip);
            let _ = write!(out, r#"<div class="n" style="{wrapper}">"#);
        }
    }
    super::video::emit(out, n);
}

/// Whether `open` left a children clip wrapper open inside the box.
fn clip_wrapper(n: &SceneNode, ellipse: bool) -> bool {
    n.clip_content && !ellipse && n.corner_path.is_some()
}

/// The CSS `clip-path` of a box with a corner outline (node-local, which
/// is the element's border box), or `None` for a plain or round box.
fn outline_clip(n: &SceneNode, ellipse: bool) -> Option<String> {
    let outline = n.corner_path.as_deref().filter(|_| !ellipse)?;
    Some(format!("path('{}')", outline.d))
}

/// Close the node's box, laying the stroke overlay over the children
/// first so a container's outline is not painted under its own content.
pub fn close(out: &mut String, n: &SceneNode, rect: Rect, ellipse: bool) {
    if clip_wrapper(n, ellipse) {
        out.push_str("</div>");
    }
    match (n.stroke, n.corner_path.as_deref().filter(|_| !ellipse)) {
        (Some(stroke), Some(outline)) if stroke.sides.is_none() => {
            emit_outline_stroke(out, stroke, rect, outline)
        }
        (Some(stroke), _) => emit_stroke_overlay(out, stroke, rect, n, ellipse),
        (None, _) => {}
    }
    out.push_str("</div>");
}

/// Write the node's fill as background declarations.
///
/// The caller has already decided this node is expressible, so a fill
/// reaching here is one CSS can state. A bitmap rides as
/// `background-image` rather than a sibling `<img>` so that the node's
/// corner radius, `clip_content` and children all nest against the same
/// box — an image node in the canonical schema is a rectangle that can
/// carry a subtree, not a leaf.
fn write_background(style: &mut String, n: &SceneNode, rect: Rect) {
    // The loader parks a neutral grey under every image node so a
    // failed decode reads as a placeholder. Keeping it as the
    // background COLOUR under the bitmap reproduces that.
    if let Some(fill) = n.fill {
        css::decl(style, "background-color", &css::color(fill));
    }
    if let (
        SceneFillType::LinearGradient
        | SceneFillType::RadialGradient
        | SceneFillType::AngularGradient,
        Some(gradient),
    ) = (n.fill_type, n.gradient.as_ref())
    {
        if let Some(value) = css::gradient(gradient, rect) {
            css::decl(style, "background-image", &value);
        }
        return;
    }
    let Some(src) = n.image_src.as_deref() else {
        return;
    };
    if src.trim().is_empty() {
        return;
    }
    css::decl(
        style,
        "background-image",
        &format!("url(\"{}\")", escape_html(src)),
    );
    css::decl(style, "background-position", "center");
    match n.image_fit {
        // `Fill` and `Crop` both cover the box and centre the overflow;
        // the crop's own affine transform is not expressible here, and a
        // node carrying one never reaches this function.
        SceneImageFit::Fill | SceneImageFit::Crop => {
            css::decl(style, "background-size", "cover");
            css::decl(style, "background-repeat", "no-repeat");
        }
        SceneImageFit::Fit => {
            css::decl(style, "background-size", "contain");
            css::decl(style, "background-repeat", "no-repeat");
        }
        SceneImageFit::Stretch => {
            css::decl(style, "background-size", "100% 100%");
            css::decl(style, "background-repeat", "no-repeat");
        }
        SceneImageFit::Tile => {
            // Guaranteed present by the caller's expressibility check —
            // a tile whose cell size is unknown cannot keep its repeat
            // frequency and takes the raster path instead.
            if let Some([w, h]) = n.image_original_size {
                let scale = if n.image_tile_scale > 0.0 {
                    n.image_tile_scale
                } else {
                    1.0
                };
                css::decl(
                    style,
                    "background-size",
                    &format!("{}px {}px", css::num(w * scale), css::num(h * scale)),
                );
            }
            css::decl(style, "background-repeat", "repeat");
        }
    }
    if let Some(blend) = css::blend_mode(n.image_blend_mode) {
        css::decl(style, "background-blend-mode", blend);
    }
}

/// Draw the stroke as an overlay box.
///
/// The overlay is inset or outset per [`SceneStrokeAlign`] so the band
/// straddles the node edge the way the scene says it does: `Inside`
/// sits flush, `Center` pulls out by half the width, `Outside` by the
/// full width. `pointer-events:none` keeps the overlay from swallowing
/// a click meant for the text underneath it.
fn emit_stroke_overlay(
    out: &mut String,
    stroke: SceneStroke,
    rect: Rect,
    n: &SceneNode,
    ellipse: bool,
) {
    let outset = match stroke.align {
        SceneStrokeAlign::Inside => 0.0,
        SceneStrokeAlign::Center => stroke.width * 0.5,
        SceneStrokeAlign::Outside => stroke.width,
    };
    let mut style = String::new();
    css::decl(&mut style, "left", &format!("{}px", css::num(-outset)));
    css::decl(&mut style, "top", &format!("{}px", css::num(-outset)));
    css::decl(
        &mut style,
        "width",
        &format!("{}px", css::num(rect.size.x + outset * 2.0)),
    );
    css::decl(
        &mut style,
        "height",
        &format!("{}px", css::num(rect.size.y + outset * 2.0)),
    );
    css::decl(&mut style, "border-style", "solid");
    css::decl(&mut style, "border-color", &css::color(stroke.color));
    match stroke.sides {
        Some([top, right, bottom, left]) => {
            css::decl(
                &mut style,
                "border-width",
                &format!(
                    "{}px {}px {}px {}px",
                    css::num(top.max(0.0)),
                    css::num(right.max(0.0)),
                    css::num(bottom.max(0.0)),
                    css::num(left.max(0.0))
                ),
            );
        }
        None => css::decl(
            &mut style,
            "border-width",
            &format!("{}px", css::num(stroke.width.max(0.0))),
        ),
    }
    if ellipse {
        css::decl(&mut style, "border-radius", "50%");
    } else if let Some(radius) = css::corner_radius(n) {
        css::decl(&mut style, "border-radius", &radius);
    }
    let _ = write!(out, r#"<div class="n k" style="{style}"></div>"#);
}

/// A smoothed box's stroke (#389): an inline SVG over the box, in its local
/// coordinates, stroking the aligned centre line (or the outline itself
/// for a centred stroke).
fn emit_outline_stroke(
    out: &mut String,
    stroke: SceneStroke,
    rect: Rect,
    outline: &de_editor_ui::layout_scene::SceneCornerPath,
) {
    let d = if outline.stroke_d.is_empty() {
        &outline.d
    } else {
        &outline.stroke_d
    };
    let pad = stroke.width.max(0.0);
    let view = Rect::xywh(-pad, -pad, rect.size.x + pad * 2.0, rect.size.y + pad * 2.0);
    let mut style = String::new();
    css::place(&mut style, view);
    css::decl(&mut style, "overflow", "visible");
    css::decl(&mut style, "pointer-events", "none");
    let _ = write!(
        out,
        r#"<svg class="n k" style="{style}" viewBox="{} {} {} {}"><path d="{}" fill="none" stroke="{}" stroke-width="{}"/></svg>"#,
        css::num(view.origin.x),
        css::num(view.origin.y),
        css::num(view.size.x),
        css::num(view.size.y),
        de_util::xml_escape::escape_xml(d),
        css::color(stroke.color),
        css::num(stroke.width.max(0.0)),
    );
}
