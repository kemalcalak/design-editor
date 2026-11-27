//! Vector nodes (path, polygon, line) as inline SVG.
//!
//! The SVG element itself is the laid-out box; its `viewBox` is the path's
//! own coordinate span and `preserveAspectRatio="none"` stretches it to the
//! box, the way the canvas fits a path into its `width` / `height`.

use super::paint::fills_of;
use super::value::{color_alpha, round2};
use crate::{fmt_num, html_escape, node_stroke};
use de_editor_core::corner_geometry;
use jian_ops_schema::node::{LineNode, PathNode, PenNode, PenPathAnchor, PolygonNode};
use jian_ops_schema::style::{PenFill, StrokeThickness};

fn n(v: f64) -> String {
    fmt_num(round2(v))
}

/// The SVG `fill` of a node: its first solid fill, a gradient's first stop,
/// else none.
fn svg_fill(node: &PenNode) -> String {
    fills_of(node)
        .iter()
        .find_map(|f| match f {
            PenFill::Solid(b) => Some(color_alpha(&b.color, f64::from(b.opacity.unwrap_or(1.0)))),
            PenFill::LinearGradient(b) => b.stops.first().map(|s| color_alpha(&s.color, 1.0)),
            PenFill::RadialGradient(b) => b.stops.first().map(|s| color_alpha(&s.color, 1.0)),
            PenFill::AngularGradient(b) => b.stops.first().map(|s| color_alpha(&s.color, 1.0)),
            _ => None,
        })
        .unwrap_or_else(|| "none".into())
}

/// `stroke` / `stroke-width` attributes (non-scaling, so a stretched viewBox
/// keeps the authored width).
fn svg_stroke(node: &PenNode) -> String {
    let Some(stroke) = node_stroke(node) else {
        return String::new();
    };
    let width = match &stroke.thickness {
        StrokeThickness::Uniform(w) => *w,
        StrokeThickness::PerSide(a) => a[0],
        StrokeThickness::Sided(s) => s.top.or(s.right).or(s.bottom).or(s.left).unwrap_or(0.0),
    };
    if width <= 0.0 {
        return String::new();
    }
    let c = stroke
        .fill
        .as_deref()
        .and_then(|fills| {
            fills.iter().find_map(|f| match f {
                PenFill::Solid(b) => {
                    Some(color_alpha(&b.color, f64::from(b.opacity.unwrap_or(1.0))))
                }
                _ => None,
            })
        })
        .unwrap_or_else(|| "rgb(0,0,0)".into());
    format!(
        " stroke=\"{}\" stroke-width=\"{}\" vector-effect=\"non-scaling-stroke\"",
        html_escape(&c),
        n(f64::from(width))
    )
}

/// SVG path data for anchors (handles are anchor-relative deltas).
pub(crate) fn anchors_d(anchors: &[PenPathAnchor], closed: bool) -> String {
    let mut d = String::new();
    let Some(first) = anchors.first() else {
        return d;
    };
    d.push_str(&format!("M{} {}", n(first.x), n(first.y)));
    let segment = |d: &mut String, a: &PenPathAnchor, b: &PenPathAnchor| {
        if a.handle_out.is_none() && b.handle_in.is_none() {
            d.push_str(&format!(" L{} {}", n(b.x), n(b.y)));
            return;
        }
        let (ox, oy) = a.handle_out.as_ref().map_or((0.0, 0.0), |h| (h.x, h.y));
        let (ix, iy) = b.handle_in.as_ref().map_or((0.0, 0.0), |h| (h.x, h.y));
        d.push_str(&format!(
            " C{} {} {} {} {} {}",
            n(a.x + ox),
            n(a.y + oy),
            n(b.x + ix),
            n(b.y + iy),
            n(b.x),
            n(b.y)
        ));
    };
    for pair in anchors.windows(2) {
        segment(&mut d, &pair[0], &pair[1]);
    }
    if closed && anchors.len() > 2 {
        segment(&mut d, &anchors[anchors.len() - 1], first);
        d.push_str(" Z");
    }
    d
}

/// `(viewBox, inner markup)` for a path node, or `None` without geometry.
/// `size` is the node's literal box, which a rounded path (#389) needs.
pub(crate) fn path_svg(
    node: &PenNode,
    p: &PathNode,
    size: Option<(f64, f64)>,
) -> Option<(String, String)> {
    let rule = match p.fill_rule {
        Some(jian_ops_schema::node::PathFillRule::Evenodd) => " fill-rule=\"evenodd\"",
        _ => "",
    };
    let markup = |d: &str| {
        format!(
            "<path d=\"{}\" fill=\"{}\"{rule}{}/>",
            html_escape(d),
            html_escape(&svg_fill(node)),
            svg_stroke(node)
        )
    };
    if let Some((view_box, d)) = rounded_path(p, size) {
        return Some((view_box, markup(&d)));
    }
    let (d, (x, y, w, h)) = if let Some(d) = p.d.as_deref().filter(|d| !d.trim().is_empty()) {
        let (x, y, w, h) = de_editor_core::svg_path_data_bounds(d)?;
        (
            d.to_string(),
            (f64::from(x), f64::from(y), f64::from(w), f64::from(h)),
        )
    } else {
        let anchors = p.anchors.as_deref().filter(|a| !a.is_empty())?;
        let closed = p.closed.unwrap_or(false);
        (
            anchors_d(anchors, closed),
            de_editor_core::path_bounds::path_bounds_from_anchors(anchors, closed),
        )
    };
    Some((
        format!("{} {} {} {}", n(x), n(y), n(w.max(1.0)), n(h.max(1.0))),
        markup(&d),
    ))
}

/// The vertices of a polygon or star fitted into `w × h`: jian's
/// `polygon_points`, the geometry the canvas paints (a star's tips touch
/// every edge).
fn polygon_vertices(p: &PolygonNode, w: f64, h: f64) -> Vec<(f64, f64)> {
    let inner = p.inner_radius.map(|r| r as f32).filter(|r| r.is_finite());
    jian_scene::layout_scene::polygon_points(
        de_editor_core::render_backend::Rect::xywh(0.0, 0.0, w as f32, h as f32),
        p.polygon_count,
        inner,
    )
    .iter()
    .map(|pt| (f64::from(pt.x), f64::from(pt.y)))
    .collect()
}

/// `(viewBox, inner markup)` for a regular polygon or star in a 100×100 box,
/// or, rounded (#389), its outline in the node's own px so the radius is
/// not stretched with the box.
pub(crate) fn polygon_svg(
    node: &PenNode,
    p: &PolygonNode,
    size: Option<(f64, f64)>,
) -> (String, String) {
    if let Some(rounded) = rounded_polygon_svg(node, p, size) {
        return rounded;
    }
    let points: Vec<String> = polygon_vertices(p, 100.0, 100.0)
        .iter()
        .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
        .collect();
    (
        "0 0 100 100".into(),
        format!(
            "<polygon points=\"{}\" fill=\"{}\"{}/>",
            points.join(" "),
            html_escape(&svg_fill(node)),
            svg_stroke(node)
        ),
    )
}

/// A polygon or star with a corner radius as its rounded (and smoothed)
/// outline, the editor's own geometry (`corner_geometry`), in node px.
fn rounded_polygon_svg(
    node: &PenNode,
    p: &PolygonNode,
    size: Option<(f64, f64)>,
) -> Option<(String, String)> {
    let radius = p.corner_radius.filter(|r| r.is_finite() && *r > 0.0)?;
    let (w, h) = size.filter(|(w, h)| *w > 0.0 && *h > 0.0)?;
    let points = polygon_vertices(p, w, h);
    let smoothing = p.corner_smoothing.unwrap_or(0.0);
    let outline = corner_geometry::polygon_outline(&points, radius, smoothing)?;
    let d = corner_geometry::outline_path_data(&[outline]);
    Some((
        format!("0 0 {} {}", n(w), n(h)),
        format!(
            "<path d=\"{}\" fill=\"{}\"{}/>",
            html_escape(&d),
            html_escape(&svg_fill(node)),
            svg_stroke(node)
        ),
    ))
}

/// A path with a `vertexRadius` (#389) as `(viewBox, d)` in node px: like
/// the canvas, the geometry is fitted to the node's box first and then
/// rounded, so a stretched path keeps the radius it shows in the editor.
/// `None` without a radius, a size or parseable geometry.
fn rounded_path(p: &PathNode, size: Option<(f64, f64)>) -> Option<(String, String)> {
    use de_editor_core::vector_edit::{fit_subpaths_to_box, parse_path_data, Subpath};
    let radius = p.vertex_radius.filter(|r| r.is_finite() && *r > 0.0)?;
    let (w, h) = size.filter(|(w, h)| *w > 0.0 && *h > 0.0)?;
    // `d` first, then the anchors, as the scene builder reads them.
    let mut subs = match p.d.as_deref().filter(|d| !d.trim().is_empty()) {
        Some(d) => parse_path_data(d)?,
        None => vec![Subpath {
            anchors: p.anchors.clone().filter(|a| !a.is_empty())?,
            closed: p.closed.unwrap_or(false),
        }],
    };
    fit_subpaths_to_box(&mut subs, w, h);
    let smoothing = p.corner_smoothing.unwrap_or(0.0);
    for sub in &mut subs {
        *sub = corner_geometry::round_corners(sub, &vec![radius; sub.anchors.len()], smoothing);
    }
    Some((
        format!("0 0 {} {}", n(w), n(h)),
        corner_geometry::outline_path_data(&subs),
    ))
}

/// `(width, height, viewBox, inner markup)` for a line: `x2` / `y2` are the
/// end point relative to the start, as the layout engine reads them.
pub(crate) fn line_svg(node: &PenNode, l: &LineNode) -> (f64, f64, String, String) {
    let dx = l.x2.unwrap_or(0.0);
    let dy = l.y2.unwrap_or(0.0);
    let (w, h) = (dx.abs().max(1.0), dy.abs().max(1.0));
    let (sx, sy) = ((-dx).max(0.0), (-dy).max(0.0));
    let stroke = match svg_stroke(node) {
        s if s.is_empty() => " stroke=\"rgb(0,0,0)\" stroke-width=\"1\"".to_string(),
        s => s,
    };
    (
        w,
        h,
        format!("0 0 {} {}", n(w), n(h)),
        format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{stroke}/>",
            n(sx),
            n(sy),
            n(sx + dx),
            n(sy + dy)
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor(x: f64, y: f64) -> PenPathAnchor {
        serde_json::from_value(serde_json::json!({ "x": x, "y": y })).unwrap()
    }

    #[test]
    fn anchors_make_lines_and_close() {
        let a = [anchor(0.0, 0.0), anchor(10.0, 0.0), anchor(10.0, 10.0)];
        assert_eq!(anchors_d(&a, true), "M0 0 L10 0 L10 10 L0 0 Z");
        assert_eq!(anchors_d(&a, false), "M0 0 L10 0 L10 10");
    }
}
