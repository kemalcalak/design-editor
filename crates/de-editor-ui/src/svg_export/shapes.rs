//! Ellipse, polygon, line and path primitives of the SVG export. Each takes
//! the layer attributes of the node it stands for (`node.rs`).

use super::node::LayerAttrs;
use super::{fill_stroke_attrs, normalize_rect, stroke_attrs, stroke_style_attrs, xml_escape};
use crate::layout_scene::{SceneNode, SceneStrokeAlign};
use crate::{Color, Rect};
use std::fmt::Write as _;

/// `rect` moved so a centred SVG stroke lands where the node's inside /
/// outside stroke does: inset or outset by half the width. Under an opaque
/// stroke the fill edge it moves is covered, so one shape still carries
/// fill and stroke (Figma's own export does the same).
pub(super) fn stroke_aligned(n: &SceneNode, rect: Rect, radius: f32) -> (Rect, f32) {
    let Some(stroke) = n.stroke.filter(|s| s.width > 0.0) else {
        return (rect, radius);
    };
    let shift = match stroke.align {
        SceneStrokeAlign::Center => return (rect, radius),
        SceneStrokeAlign::Inside => -stroke.width * 0.5,
        SceneStrokeAlign::Outside => stroke.width * 0.5,
    };
    let w = (rect.size.x + shift * 2.0).max(0.0);
    let h = (rect.size.y + shift * 2.0).max(0.0);
    let radius = if radius > 0.0 {
        (radius + shift).max(0.0)
    } else {
        radius
    };
    (
        Rect::xywh(rect.origin.x - shift, rect.origin.y - shift, w, h),
        radius,
    )
}

pub(super) fn emit_ellipse(out: &mut String, n: &SceneNode, attrs: &str) {
    let box_rect = normalize_rect(n.bounds);
    if box_rect.size.x == 0.0 || box_rect.size.y == 0.0 {
        return;
    }
    let (r, _) = stroke_aligned(n, box_rect, 0.0);
    let cx = r.origin.x + r.size.x * 0.5;
    let cy = r.origin.y + r.size.y * 0.5;
    let _ = write!(
        out,
        r#"<ellipse{attrs} cx="{cx}" cy="{cy}" rx="{}" ry="{}"{}/>"#,
        r.size.x * 0.5,
        r.size.y * 0.5,
        fill_stroke_attrs(n),
    );
}

pub(super) fn emit_polygon(out: &mut String, n: &SceneNode, attrs: &str) {
    let r = normalize_rect(n.bounds);
    if r.size.x == 0.0 || r.size.y == 0.0 {
        return;
    }
    if let Some(d) = super::outline::node_outline(n) {
        // Rounded corners (#389): the precomputed outline.
        let _ = write!(out, r#"<path{attrs} d="{d}"{}/>"#, fill_stroke_attrs(n));
        return;
    }
    let points = crate::polygon_geometry::polygon_node_points(n, r);
    let mut point_attr = String::new();
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            point_attr.push(' ');
        }
        let _ = write!(point_attr, "{},{}", p.x, p.y);
    }
    let _ = write!(
        out,
        r#"<polygon{attrs} points="{point_attr}"{}/>"#,
        fill_stroke_attrs(n),
    );
}

pub(super) fn emit_line(out: &mut String, n: &SceneNode, attrs: &str) {
    if let Some(geometry) = n.stroke_geometry.as_deref() {
        let _ = write!(out, "<g{attrs}>");
        write_stroke_geometry(out, n, geometry, &geometry_place(n));
        out.push_str("</g>");
        return;
    }
    let (color, width) = match n.stroke {
        Some(s) => (s.color, s.width),
        None => (n.fill.unwrap_or(Color::BLACK), 1.5),
    };
    // Start to end the way the line was drawn (#389), not always the
    // box's top-left → bottom-right diagonal.
    let (from, to) = n.line_endpoints();
    let _ = write!(
        out,
        r#"<line{attrs} x1="{}" y1="{}" x2="{}" y2="{}"{}/>"#,
        from.x,
        from.y,
        to.x,
        to.y,
        stroke_attrs(color, width) + &stroke_style_attrs(n),
    );
}

pub(super) fn emit_path(out: &mut String, n: &SceneNode, attrs: &LayerAttrs) {
    let outline = super::outline::node_outline(n);
    let (d, fit) = if let Some(d) = &outline {
        // Rounded vertices (#389): the outline, already in place (escaped).
        (d.clone(), glam::Affine2::IDENTITY)
    } else if let Some(d) = &n.svg_path {
        (d.clone(), svg_path_fit(d, n.bounds))
    } else {
        if n.points.len() < 2 {
            return;
        }
        let mut d = String::with_capacity(n.points.len() * 16);
        let _ = write!(d, "M{} {}", n.points[0].x, n.points[0].y);
        for p in &n.points[1..] {
            let _ = write!(d, " L{} {}", p.x, p.y);
        }
        if n.path_closed {
            d.push_str(" Z");
        }
        (d, glam::Affine2::IDENTITY)
    };
    let paint = if n.svg_path.is_none() && !n.path_closed && n.stroke.is_none() {
        let color = n.fill.unwrap_or(Color::BLACK);
        format!(r#" fill="none"{}"#, stroke_attrs(color, 1.5))
    } else {
        fill_stroke_attrs(n)
    };
    let vector_effect = if n.svg_path.is_some() && n.stroke.is_some() && outline.is_none() {
        r#" vector-effect="non-scaling-stroke""#
    } else {
        ""
    };
    let fill_rule = if n.even_odd_fill {
        r#" fill-rule="evenodd""#
    } else {
        ""
    };
    let d = if outline.is_some() { d } else { xml_escape(&d) };
    if let Some(geometry) = n.stroke_geometry.as_deref() {
        // End shapes (#389): the fill (if any) on the path, then the
        // stroke geometry in node-local coordinates.
        let _ = write!(out, "<g{}>", attrs.attrs());
        let fill = fill_attrs_only(n);
        if !fill.is_empty() {
            let _ = write!(
                out,
                r#"<path d="{d}" transform="{}"{fill_rule}{fill}/>"#,
                super::transform::matrix_value(fit),
            );
        }
        write_stroke_geometry(out, n, geometry, &geometry_place(n));
        out.push_str("</g>");
        return;
    }
    if !attrs.filter.is_empty() && fit != glam::Affine2::IDENTITY {
        // A filter reads its region in the element's own user space; with
        // the fit on the same element that is the `d` string's space, not
        // the node box the region is written in. The fit goes inside.
        let _ = write!(
            out,
            r#"<g{}><path d="{d}" transform="{}"{vector_effect}{fill_rule}{paint}/></g>"#,
            attrs.attrs(),
            super::transform::matrix_value(fit),
        );
        return;
    }
    let _ = write!(
        out,
        r#"<path{} d="{d}"{vector_effect}{fill_rule}{paint}/>"#,
        attrs.attrs_with(fit, 1.0),
    );
}

/// The scale + translate that fits an imported `d` string's own bounds
/// into the node box, as the canvas paints it.
fn svg_path_fit(d: &str, bounds: Rect) -> glam::Affine2 {
    let rect = normalize_rect(bounds);
    let Some((source_x, source_y, source_w, source_h)) = de_editor_core::svg_path_data_bounds(d)
    else {
        return glam::Affine2::from_translation(glam::Vec2::new(rect.origin.x, rect.origin.y));
    };
    let sx = if source_w.abs() > 0.01 {
        rect.size.x / source_w
    } else {
        1.0
    };
    let sy = if source_h.abs() > 0.01 {
        rect.size.y / source_h
    } else {
        1.0
    };
    let tx = rect.origin.x - source_x * sx;
    let ty = rect.origin.y - source_y * sy;
    glam::Affine2::from_scale_angle_translation(
        glam::Vec2::new(sx, sy),
        0.0,
        glam::Vec2::new(tx, ty),
    )
}

/// The node's fill attributes when it paints one, else empty.
fn fill_attrs_only(n: &SceneNode) -> String {
    let fill = super::fill_attrs(n);
    if fill == r#" fill="none""# {
        String::new()
    } else {
        fill
    }
}

/// A line's or open path's precomputed stroke geometry (#389): the centre
/// line (styled like the canvas, `centre_line_style`), the filled end
/// shapes (nonzero) and the line-arrow arms, as plain paths placed in the
/// node box.
/// Written as paths rather than markers so Figma's SVG import, which
/// drops markers, keeps the arrowheads. `place` is the attribute text
/// (e.g. a `transform`) that puts node-local coordinates where they go.
/// Shared with the structured HTML export, which inlines the same SVG.
pub fn write_stroke_geometry(
    out: &mut String,
    n: &SceneNode,
    geometry: &crate::layout_scene::SceneStrokeGeometry,
    place: &str,
) {
    use crate::layout_scene::{StrokeCap, StrokeJoin, StrokeStyle};
    let Some(stroke) = n.stroke else {
        return;
    };
    let line = stroke_attrs(stroke.color, stroke.width);
    if !geometry.stroke_d.is_empty() {
        let style = crate::stroke_centre_line::centre_line_style(stroke.style);
        let _ = write!(
            out,
            r#"<path d="{}"{place} fill="none"{line}{}/>"#,
            xml_escape(&geometry.stroke_d),
            super::style_attrs(&style),
        );
    }
    if !geometry.fill_d.is_empty() {
        let mut fill = format!(
            r#" fill="{}" fill-rule="nonzero""#,
            super::color_to_rgb(stroke.color)
        );
        if stroke.color.a < 0.999 {
            let _ = write!(fill, r#" fill-opacity="{}""#, stroke.color.a);
        }
        let _ = write!(
            out,
            r#"<path d="{}"{place}{fill}/>"#,
            xml_escape(&geometry.fill_d),
        );
    }
    if !geometry.arms_d.is_empty() {
        let style = StrokeStyle {
            cap: Some(StrokeCap::Butt),
            join: Some(StrokeJoin::Miter),
            ..StrokeStyle::default()
        };
        let _ = write!(
            out,
            r#"<path d="{}"{place} fill="none"{line}{}/>"#,
            xml_escape(&geometry.arms_d),
            super::style_attrs(&style),
        );
    }
}

/// ` transform="translate(x y)"`: node-local geometry into the node box
/// (the layer attributes already carry rotation and flips).
fn geometry_place(n: &SceneNode) -> String {
    let origin = normalize_rect(n.bounds).origin;
    format!(r#" transform="translate({} {})""#, origin.x, origin.y)
}
