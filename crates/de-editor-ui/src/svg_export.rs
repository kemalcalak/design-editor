//! Platform-neutral SVG serializer for native and wasm hosts.
//!
//! The file keeps the editor's layer tree (`svg_export/node.rs`): frames,
//! groups and auto-layout containers are nested `<g>` elements, text stays
//! live `<text>`, images are `<image>` elements, effects are SVG filters.
//! [`SvgExportOptions`] adds layer names as element ids (what Figma's SVG
//! import shows as layer names) and embeds linked images; what the file
//! cannot carry is listed in [`SvgExport::warnings`].

use crate::layout_scene::{LayoutScene, NodeKind, SceneGradient, SceneNode, ScenePage};
use crate::{Color, Rect};
use std::collections::HashMap;
use std::fmt::Write as _;

mod angular;
mod effects;
mod fill_stack;
mod icon;
mod image;
mod image_metadata;
mod node;
mod outline;
#[path = "svg_export_rect.rs"]
mod rect;
mod shapes;
pub use shapes::write_stroke_geometry;
mod text;
mod text_outline;
mod transform;
mod writer;

use node::{
    box_clip, box_paint, clips_children, emit_node, is_box, is_container, is_written, BoxPaint,
};
use text::emit_text_fill_defs;
use transform::{
    apply_ancestor_clip_bounds, apply_clip_bounds, close_ancestor_clip_groups,
    emit_affine_group_start, emit_ancestor_clip_defs, emit_ancestor_clip_groups_start,
    find_node_with_ancestor_context, node_transform, svg_id, AncestorClip,
};
use writer::Writer;

const MARGIN: f32 = 16.0;

/// Layer names by node id, for [`SvgExportOptions::layer_names`].
pub type LayerNames = HashMap<String, String>;

/// How an SVG export is written. The default is what the desktop's
/// File ▸ Export has always written: node ids as element ids, image links
/// kept as they are.
#[derive(Debug, Clone, Copy, Default)]
pub struct SvgExportOptions<'a> {
    /// Element ids from these layer names (made attribute-safe and unique),
    /// with the name as authored in `data-name`; a node missing from the
    /// map keeps its node id.
    pub layer_names: Option<&'a LayerNames>,
    /// Turn linked images whose bytes the editor holds into `data:` URLs.
    pub embed_images: bool,
}

/// A serialized SVG file and what it could not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgExport {
    pub svg: String,
    /// One line per node feature the file leaves out or approximates
    /// (`"<layer name> (<node id>): <what>"`): first those found while
    /// writing the defs (effects, linked images), then the rest, each pass
    /// in document order.
    pub warnings: Vec<String>,
}

/// A failed SVG serialization.
///
/// `Display` reproduces the previous ad-hoc `String` messages byte for byte,
/// and `From<SvgExportError> for String` keeps the `?` call sites in hosts
/// that still return `Result<_, String>` compiling unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgExportError {
    /// The scene carries no active page.
    NoActivePage,
    /// The selected content has no positive-area painted bounds.
    NothingToExport,
    /// The requested node is not on the active page.
    NodeNotFound(String),
    /// The requested node exists but paints no geometry.
    NodePaintsNothing(String),
}

impl std::fmt::Display for SvgExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SvgExportError::NoActivePage => write!(f, "no active page"),
            SvgExportError::NothingToExport => write!(f, "nothing to export"),
            SvgExportError::NodeNotFound(id) => {
                write!(f, "node {id} not found on the active page")
            }
            SvgExportError::NodePaintsNothing(id) => write!(f, "node {id} paints nothing"),
        }
    }
}

impl std::error::Error for SvgExportError {}

impl From<SvgExportError> for String {
    fn from(error: SvgExportError) -> String {
        error.to_string()
    }
}

/// Serialize the active page in `scene` as an SVG string.
pub fn serialize_active_page_svg(scene: &LayoutScene) -> Result<String, SvgExportError> {
    let page = scene.active_page().ok_or(SvgExportError::NoActivePage)?;
    serialize_page_svg_with(page, &SvgExportOptions::default()).map(|export| export.svg)
}

/// Serialize one active-page node and subtree, tightly cropped to its painted bounds.
pub fn serialize_node_svg(scene: &LayoutScene, node_id: &str) -> Result<String, SvgExportError> {
    let page = scene.active_page().ok_or(SvgExportError::NoActivePage)?;
    serialize_node_on_page_svg_with(page, node_id, &SvgExportOptions::default())
        .map(|export| export.svg)
}

/// Serialize every top-level layer of `page` (with a 16 px margin).
pub fn serialize_page_svg_with(
    page: &ScenePage,
    options: &SvgExportOptions,
) -> Result<SvgExport, SvgExportError> {
    serialize_svg(
        &page.children,
        page_bounds(page).ok_or(SvgExportError::NothingToExport)?,
        MARGIN,
        glam::Affine2::IDENTITY,
        &[],
        options,
    )
}

/// Serialize node `node_id` of `page` and its subtree, cropped to its
/// painted bounds; ancestors' rotation, flip and clips still apply.
pub fn serialize_node_on_page_svg_with(
    page: &ScenePage,
    node_id: &str,
    options: &SvgExportOptions,
) -> Result<SvgExport, SvgExportError> {
    let (node, ancestor_xform, ancestor_clips) =
        find_node_with_ancestor_context(&page.children, node_id)
            .ok_or_else(|| SvgExportError::NodeNotFound(node_id.to_string()))?;
    let mut acc = BoundsAcc::new();
    collect_bounds(node, ancestor_xform, &mut acc);
    let painted_bounds = acc
        .into_rect()
        .ok_or_else(|| SvgExportError::NodePaintsNothing(node_id.to_string()))?;
    let bounds = apply_ancestor_clip_bounds(painted_bounds, &ancestor_clips)
        .ok_or_else(|| SvgExportError::NodePaintsNothing(node_id.to_string()))?;
    serialize_svg(
        std::slice::from_ref(node),
        bounds,
        0.0,
        ancestor_xform,
        &ancestor_clips,
        options,
    )
}

/// Serialize several nodes together (a multi-selection copy, #399),
/// cropped tight to their combined painted bounds. The nodes are taken as
/// given: no ancestor transform or clip applies.
pub fn serialize_nodes_svg_with(
    nodes: &[SceneNode],
    options: &SvgExportOptions,
) -> Result<SvgExport, SvgExportError> {
    let mut acc = BoundsAcc::new();
    for node in nodes {
        collect_bounds(node, glam::Affine2::IDENTITY, &mut acc);
    }
    let bounds = acc.into_rect().ok_or(SvgExportError::NothingToExport)?;
    serialize_svg(nodes, bounds, 0.0, glam::Affine2::IDENTITY, &[], options)
}

fn serialize_svg(
    nodes: &[SceneNode],
    bounds: Rect,
    margin: f32,
    root_xform: glam::Affine2,
    ancestor_clips: &[AncestorClip],
    options: &SvgExportOptions,
) -> Result<SvgExport, SvgExportError> {
    let view_x = bounds.origin.x - margin;
    let view_y = bounds.origin.y - margin;
    let view_w = bounds.size.x + margin * 2.0;
    let view_h = bounds.size.y + margin * 2.0;
    if view_w <= 0.0 || view_h <= 0.0 {
        return Err(SvgExportError::NothingToExport);
    }
    let mut w = Writer::new(options);
    let mut svg = String::with_capacity(4096);
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{view_x} {view_y} {view_w} {view_h}" width="{view_w}" height="{view_h}">"#
    );
    emit_defs(&mut svg, nodes, ancestor_clips, &mut w);
    emit_ancestor_clip_groups_start(&mut svg, ancestor_clips);
    let wraps_root = root_xform != glam::Affine2::IDENTITY;
    if wraps_root {
        emit_affine_group_start(&mut svg, root_xform);
    }
    for node in nodes {
        emit_node(&mut svg, node, &mut w);
    }
    if wraps_root {
        svg.push_str("</g>");
    }
    close_ancestor_clip_groups(&mut svg, ancestor_clips);
    svg.push_str("</svg>");
    Ok(SvgExport {
        svg,
        warnings: w.into_warnings(),
    })
}

fn emit_defs(
    out: &mut String,
    nodes: &[SceneNode],
    ancestor_clips: &[AncestorClip],
    w: &mut Writer,
) {
    // Written in place; an empty `<defs>` is taken back off.
    let start = out.len();
    out.push_str("<defs>");
    let body = out.len();
    emit_ancestor_clip_defs(out, ancestor_clips);
    for node in nodes {
        emit_node_defs(out, node, w);
    }
    if out.len() == body {
        out.truncate(start);
    } else {
        out.push_str("</defs>");
    }
}

/// Defs for `n` and its written descendants, decided by the same
/// predicates the body pass (`node.rs`) uses. Effect and linked-image
/// warnings are raised here; `Writer::warn` keeps each line once.
fn emit_node_defs(out: &mut String, n: &SceneNode, w: &mut Writer) {
    if !is_written(n) {
        return;
    }
    let id = svg_id(&n.id);
    if let Some(gradient) = &n.gradient {
        emit_gradient(
            out,
            &format!("gradient-{id}"),
            gradient,
            normalize_rect(n.bounds),
        );
    }
    if let Some(d) = (clips_children(n) || box_clip(n))
        .then(|| outline::node_outline(n))
        .flatten()
    {
        // Smoothed corners (#389) clip to the outline.
        let _ = write!(
            out,
            r#"<clipPath id="clip-{id}"><path d="{d}"/></clipPath>"#
        );
    } else if clips_children(n) || box_clip(n) {
        let r = normalize_rect(n.bounds);
        let _ = write!(
            out,
            r#"<clipPath id="clip-{id}"><rect x="{}" y="{}" width="{}" height="{}" rx="{}"/></clipPath>"#,
            r.origin.x,
            r.origin.y,
            r.size.x,
            r.size.y,
            n.corner_radius.max(0.0),
        );
    }
    if is_box(n) {
        match box_paint(n) {
            BoxPaint::Image(image) => image::emit_box_image_pattern(out, n, &image, w),
            BoxPaint::Stack => fill_stack::emit_fill_stack_defs(out, n, &id, w),
            BoxPaint::Plain => {}
        }
    }
    if n.kind == NodeKind::Text {
        emit_text_fill_defs(out, n, &id, w);
    }
    let container = is_container(n);
    let content = if container {
        content_bounds(n)
    } else {
        n.bounds
    };
    effects::emit_effect_defs(out, n, container, content, w);
    for child in &n.children {
        emit_node_defs(out, child, w);
    }
}

fn emit_gradient(out: &mut String, id: &str, gradient: &SceneGradient, bounds: Rect) {
    match gradient {
        SceneGradient::Linear {
            angle_deg,
            opacity,
            stops,
        } => {
            let angle = (angle_deg - 90.0).to_radians();
            let cx = bounds.origin.x + bounds.size.x * 0.5;
            let cy = bounds.origin.y + bounds.size.y * 0.5;
            let dx = angle.cos() * bounds.size.x * 0.5;
            let dy = angle.sin() * bounds.size.y * 0.5;
            let _ = write!(
                out,
                r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                cx - dx,
                cy - dy,
                cx + dx,
                cy + dy,
            );
            emit_stops(out, stops, *opacity);
            out.push_str("</linearGradient>");
        }
        SceneGradient::Radial {
            cx,
            cy,
            radius,
            opacity,
            stops,
        } => {
            let center_x = bounds.origin.x + bounds.size.x * cx.clamp(0.0, 1.0);
            let center_y = bounds.origin.y + bounds.size.y * cy.clamp(0.0, 1.0);
            let outer = (bounds.size.x.max(bounds.size.y) * radius.clamp(0.0, 1.0)).max(0.01);
            let _ = write!(
                out,
                r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{center_x}" cy="{center_y}" r="{outer}">"#
            );
            emit_stops(out, stops, *opacity);
            out.push_str("</radialGradient>");
        }
        SceneGradient::Angular {
            cx,
            cy,
            rotation_deg,
            opacity,
            stops,
        } => {
            let spec = angular::AngularSpec {
                cx: *cx,
                cy: *cy,
                rotation_deg: *rotation_deg,
                opacity: *opacity,
                stops,
            };
            angular::emit_angular_pattern(out, id, &spec, bounds);
        }
        SceneGradient::Mesh {
            colors, opacity, ..
        } => {
            if let Some(color) = colors.first() {
                let _ = write!(
                    out,
                    r#"<linearGradient id="{id}"><stop offset="0" stop-color="{}" stop-opacity="{}"/><stop offset="1" stop-color="{}" stop-opacity="{}"/></linearGradient>"#,
                    color_to_rgb(*color),
                    color.a * opacity,
                    color_to_rgb(*color),
                    color.a * opacity
                );
            }
        }
    }
}

fn emit_stops(out: &mut String, stops: &[crate::layout_scene::SceneGradientStop], opacity: f32) {
    // `SceneGradient::opacity` already includes the node's cumulative
    // layer opacity, matching the canvas shader path. Applying
    // `SceneNode::opacity` again here or on an enclosing group would
    // double-dim gradients and descendants.
    for stop in stops {
        let _ = write!(
            out,
            r#"<stop offset="{}" stop-color="{}" stop-opacity="{}"/>"#,
            stop.offset.clamp(0.0, 1.0),
            color_to_rgb(stop.color),
            stop.color.a * opacity.clamp(0.0, 1.0)
        );
    }
}

fn page_bounds(page: &ScenePage) -> Option<Rect> {
    let mut acc = BoundsAcc::new();
    for n in &page.children {
        collect_bounds(n, glam::Affine2::IDENTITY, &mut acc);
    }
    acc.into_rect()
}

use crate::scene_bounds::{normalize_rect, BoundsAcc, PaintCornerRules};

/// SVG export counts gradient and image fills as own paint.
const SVG_PAINT_RULES: PaintCornerRules = PaintCornerRules {
    gradient_paints: true,
    image_paints: true,
};

/// `n`'s box joined with everything its children paint, in `n`'s own user
/// space (inside its transform): what a layer blur on the group covers.
fn content_bounds(n: &SceneNode) -> Rect {
    let mut acc = BoundsAcc::new();
    let r = normalize_rect(n.bounds);
    acc.add(
        r.origin.x,
        r.origin.y,
        r.origin.x + r.size.x,
        r.origin.y + r.size.y,
    );
    for child in &n.children {
        collect_bounds(child, glam::Affine2::IDENTITY, &mut acc);
    }
    acc.into_rect().unwrap_or(r)
}

fn collect_bounds(n: &SceneNode, parent_xform: glam::Affine2, acc: &mut BoundsAcc) {
    if !is_written(n) {
        return;
    }
    let local_xform = parent_xform * node_transform(n);
    // Shadows and blur paint past the box; the view box keeps them.
    let [left, top, right, bottom] = effects::paint_outsets(n);
    if left + top + right + bottom > 0.0 {
        let r = normalize_rect(n.bounds);
        for p in [
            glam::Vec2::new(r.origin.x - left, r.origin.y - top),
            glam::Vec2::new(r.origin.x + r.size.x + right, r.origin.y - top),
            glam::Vec2::new(r.origin.x - left, r.origin.y + r.size.y + bottom),
            glam::Vec2::new(
                r.origin.x + r.size.x + right,
                r.origin.y + r.size.y + bottom,
            ),
        ] {
            let w = local_xform.transform_point2(p);
            acc.add(w.x, w.y, w.x, w.y);
        }
    }
    if let Some(local_corners) = own_paint_corners(n) {
        for p in local_corners {
            let w = local_xform.transform_point2(p);
            acc.add(w.x, w.y, w.x, w.y);
        }
    }
    if n.clip_content && !n.children.is_empty() && n.bounds.size.x > 0.0 && n.bounds.size.y > 0.0 {
        let mut child_acc = BoundsAcc::new();
        for child in &n.children {
            collect_bounds(child, local_xform, &mut child_acc);
        }
        if let Some(visible) = child_acc
            .into_rect()
            .and_then(|bounds| apply_clip_bounds(bounds, n.bounds, local_xform))
        {
            acc.add(
                visible.origin.x,
                visible.origin.y,
                visible.origin.x + visible.size.x,
                visible.origin.y + visible.size.y,
            );
        }
        return;
    }
    for child in &n.children {
        collect_bounds(child, local_xform, acc);
    }
}

fn own_paint_corners(n: &SceneNode) -> Option<Vec<glam::Vec2>> {
    crate::scene_bounds::own_paint_corners(n, SVG_PAINT_RULES)
}

/// An attribute value escaped, borrowed when it has nothing to escape
/// (a long `data:` URL, most names).
fn escape_attr(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains(['&', '<', '>', '"', '\'']) {
        std::borrow::Cow::Owned(xml_escape(text))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

fn xml_escape(text: &str) -> String {
    de_util::xml_escape::escape_xml(text)
}

fn fill_stroke_attrs(n: &SceneNode) -> String {
    let mut s = fill_attrs(n);
    if let Some(stroke) = n.stroke {
        s.push_str(&stroke_attrs(stroke.color, stroke.width));
        s.push_str(&stroke_style_attrs(n));
    }
    s
}

/// `stroke-linecap` / `stroke-linejoin` / `stroke-dasharray` /
/// `stroke-dashoffset` for a node's authored stroke style (#89); empty
/// when it has none.
pub(crate) fn stroke_style_attrs(n: &SceneNode) -> String {
    n.stroke
        .map(|stroke| style_attrs(&stroke.style))
        .unwrap_or_default()
}

/// [`stroke_style_attrs`] of one render-side style; also writes
/// `stroke-miterlimit` (#389).
pub(crate) fn style_attrs(style: &crate::layout_scene::StrokeStyle) -> String {
    use crate::layout_scene::{StrokeCap as LineCap, StrokeJoin as LineJoin};
    let mut s = String::new();
    if let Some(cap) = style.cap {
        let cap = match cap {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        };
        let _ = write!(s, r#" stroke-linecap="{cap}""#);
    }
    if let Some(join) = style.join {
        let join = match join {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        };
        let _ = write!(s, r#" stroke-linejoin="{join}""#);
    }
    if let Some(limit) = style.miter_limit {
        let _ = write!(s, r#" stroke-miterlimit="{limit}""#);
    }
    if let Some(dash) = style.dash {
        let lengths: Vec<String> = dash.lengths().iter().map(|len| len.to_string()).collect();
        let _ = write!(s, r#" stroke-dasharray="{}""#, lengths.join(" "));
        if dash.offset() != 0.0 {
            let _ = write!(s, r#" stroke-dashoffset="{}""#, dash.offset());
        }
    }
    s
}

fn fill_attrs(n: &SceneNode) -> String {
    let mut s = String::new();
    if n.gradient.is_some() {
        let _ = write!(s, r#" fill="url(#gradient-{})""#, svg_id(&n.id));
    } else if let Some(fill) = n.fill {
        let _ = write!(s, r#" fill="{}""#, color_to_rgb(fill));
        if fill.a < 0.999 {
            let _ = write!(s, r#" fill-opacity="{}""#, fill.a);
        }
    } else {
        s.push_str(r#" fill="none""#);
    }
    s
}

fn stroke_attrs(color: Color, width: f32) -> String {
    let mut s = format!(
        r#" stroke="{}" stroke-width="{}""#,
        color_to_rgb(color),
        width
    );
    if color.a < 0.999 {
        let _ = write!(s, r#" stroke-opacity="{}""#, color.a);
    }
    s
}

/// A colour's channels as 0..=1 numbers, for filter colour matrices.
fn color_to_rgb_triplet(c: Color) -> (f32, f32, f32) {
    (
        c.r.clamp(0.0, 1.0),
        c.g.clamp(0.0, 1.0),
        c.b.clamp(0.0, 1.0),
    )
}

fn color_to_rgb(c: Color) -> String {
    format!(
        "rgb({},{},{})",
        (c.r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (c.b.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

#[cfg(test)]
#[path = "svg_export_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_export_layer_tests.rs"]
mod layer_tests;

#[cfg(test)]
#[path = "svg_export_stroke_ends_tests.rs"]
mod stroke_ends_tests;

#[cfg(test)]
#[path = "svg_export_corner_tests.rs"]
mod corner_tests;
