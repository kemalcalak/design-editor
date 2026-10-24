//! Vector nodes (path, polygon, line) as inline SVG.
//!
//! The SVG element itself is the laid-out box; its `viewBox` is the path's
//! own coordinate span and `preserveAspectRatio="none"` stretches it to the
//! box, the way the canvas fits a path into its `width` / `height`.

use super::paint::fills_of;
use super::value::{color_alpha, round2};
use crate::{fmt_num, html_escape, node_stroke};
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
pub(crate) fn path_svg(node: &PenNode, p: &PathNode) -> Option<(String, String)> {
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
    let rule = match p.fill_rule {
        Some(jian_ops_schema::node::PathFillRule::Evenodd) => " fill-rule=\"evenodd\"",
        _ => "",
    };
    Some((
        format!("{} {} {} {}", n(x), n(y), n(w.max(1.0)), n(h.max(1.0))),
        format!(
            "<path d=\"{}\" fill=\"{}\"{rule}{}/>",
            html_escape(&d),
            html_escape(&svg_fill(node)),
            svg_stroke(node)
        ),
    ))
}

/// `(viewBox, inner markup)` for a regular polygon or star in a 100×100 box.
pub(crate) fn polygon_svg(node: &PenNode, p: &PolygonNode) -> (String, String) {
    let count = p.polygon_count.max(3) as usize;
    let inner = p.inner_radius.filter(|r| *r > 0.0 && *r < 1.0);
    let steps = if inner.is_some() { count * 2 } else { count };
    let points: Vec<String> = (0..steps)
        .map(|i| {
            let angle =
                -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / steps as f64;
            let r = match inner {
                Some(ir) if i % 2 == 1 => ir,
                _ => 1.0,
            };
            format!(
                "{},{}",
                n(50.0 + 50.0 * r * angle.cos()),
                n(50.0 + 50.0 * r * angle.sin())
            )
        })
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
