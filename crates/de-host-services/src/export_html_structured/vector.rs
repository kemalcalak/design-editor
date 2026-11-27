//! Emitters for the node kinds a `<div>` cannot describe: icons, and
//! the line / polygon / path family.
//!
//! All three write an inline `<svg>`. Inline means the glyph data ships
//! as markup inside the one file — no icon font to download, no sprite
//! sheet, no request. The `<svg>` box is pinned by the same absolute
//! positioning every other node uses, and its `viewBox` is stated in
//! ABSOLUTE doc coordinates so the geometry the scene already resolved
//! (which is absolute) drops in without a second coordinate change.

use de_editor_ui::layout_scene::SceneNode;
use de_editor_ui::polygon_geometry::polygon_node_points;
use de_editor_ui::widgets::icon_catalog::IconRenderStyle;
use de_editor_ui::widgets::icon_family::{
    resolve_icon_glyph, IconGlyph, FALLBACK_ICON_D, ICON_DEFAULT_TINT, ICON_GLYPH_UNITS,
};
use de_editor_ui::{Color, Point2D, Rect};
use de_util::xml_escape::escape_xml;
use std::fmt::Write as _;

use super::css;

/// Emit an `icon_font` node as an inline SVG.
///
/// `rect` is already board-local. The glyph is centred in a square of
/// `min(w, h)`, matching `icons::paint_icon_font_node`, so an icon in a
/// non-square box sits where the canvas puts it rather than stretching.
pub fn emit_icon(out: &mut String, n: &SceneNode, rect: Rect) {
    let size = rect.size.x.min(rect.size.y);
    if size <= 0.0 {
        return;
    }
    let square = Rect {
        origin: Point2D::new(
            rect.origin.x + (rect.size.x - size) / 2.0,
            rect.origin.y + (rect.size.y - size) / 2.0,
        ),
        size: Point2D::new(size, size),
    };
    let name = n.text.as_deref().unwrap_or("");
    let tint = css::color(n.fill.unwrap_or(ICON_DEFAULT_TINT));

    let IconGlyph {
        viewbox,
        paths: paints,
        outline_units,
    } = resolve_icon_glyph(&n.font_family, name, n.font_weight).unwrap_or_else(|| IconGlyph {
        // The honest dot the canvas draws for an unknown name.
        viewbox: ICON_GLYPH_UNITS,
        paths: vec![(FALLBACK_ICON_D.to_string(), IconRenderStyle::Stroke)],
        outline_units: 0.0,
    });
    let mut style = String::new();
    css::place(&mut style, square);

    // The canvas strokes at `(size / 24) * 2` doc px against a 24-unit
    // viewBox. In user units that is `viewbox / 12`, independent of the
    // rendered size — which is what makes the icon scale like the
    // canvas does instead of thickening as the slide is projected up.
    let stroke_width = css::num(viewbox / 12.0);
    let _ = write!(
        out,
        r#"<svg class="n" style="{style}" viewBox="0 0 {vb} {vb}" fill="none" stroke-width="{stroke_width}" stroke-linecap="round" stroke-linejoin="round">"#,
        vb = css::num(viewbox)
    );
    for (d, painted) in paints {
        match painted {
            // A Material Symbols weight above 400 is the canvas's
            // same-colour outline over the fill (`icons_font_node`).
            IconRenderStyle::Fill if outline_units > 0.0 => {
                let _ = write!(
                    out,
                    r#"<path d="{}" fill="{tint}" stroke="{tint}" stroke-width="{}" stroke-linejoin="round"/>"#,
                    escape_xml(&d),
                    css::num(outline_units * viewbox / ICON_GLYPH_UNITS)
                );
            }
            IconRenderStyle::Fill => {
                let _ = write!(
                    out,
                    r#"<path d="{}" fill="{tint}" stroke="none"/>"#,
                    escape_xml(&d)
                );
            }
            IconRenderStyle::Stroke => {
                let _ = write!(out, r#"<path d="{}" stroke="{tint}"/>"#, escape_xml(&d));
            }
        }
    }
    out.push_str("</svg>");
}

/// Emit a Line node as an inline SVG segment.
///
/// The segment runs from the line's start to its end
/// (`SceneNode::line_endpoints`: the scene keeps a normalised box plus the
/// direction it was drawn in, #389), so the start stays the start. The SVG
/// box is the segment's normalised rect padded out by the stroke, which
/// also keeps a perfectly horizontal line (zero height) from collapsing to
/// an unrenderable zero-extent viewport.
pub fn emit_line(out: &mut String, n: &SceneNode, origin: Point2D) {
    if emit_stroke_geometry(out, n, origin) {
        return;
    }
    let (color, width) = match n.stroke {
        Some(s) => (s.color, s.width),
        None => (n.fill.unwrap_or(Color::BLACK), 1.5),
    };
    let (a, b) = n.line_endpoints();
    let doc_box = padded_box(
        Rect {
            origin: Point2D::new(a.x.min(b.x), a.y.min(b.y)),
            size: Point2D::new((b.x - a.x).abs(), (b.y - a.y).abs()),
        },
        width * 0.5,
    );
    open_svg(out, doc_box, origin);
    let _ = write!(
        out,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
        css::num(a.x),
        css::num(a.y),
        css::num(b.x),
        css::num(b.y),
        css::color(color),
        css::num(width)
    );
    out.push_str("</svg>");
}

/// Emit a Polygon node as an inline SVG polygon over its regular-N-gon
/// or star vertices — the same `polygon_node_points` the canvas paints.
pub fn emit_polygon(out: &mut String, n: &SceneNode, origin: Point2D) {
    let rect = normalize(n.bounds);
    if rect.size.x <= 0.0 || rect.size.y <= 0.0 {
        return;
    }
    let stroke_pad = n.stroke.map_or(0.0, |s| s.width * 0.5);
    open_svg(out, padded_box(rect, stroke_pad), origin);
    let mut points = String::new();
    for (i, p) in polygon_node_points(n, rect).iter().enumerate() {
        if i > 0 {
            points.push(' ');
        }
        let _ = write!(points, "{},{}", css::num(p.x), css::num(p.y));
    }
    let _ = write!(out, r#"<polygon points="{points}"{}/>"#, paint_attrs(n));
    out.push_str("</svg>");
}

/// Emit a Path node — either an imported SVG `d` fitted into its box,
/// or the pen tool's polyline through `points`.
pub fn emit_path(out: &mut String, n: &SceneNode, origin: Point2D) {
    if emit_stroke_geometry(out, n, origin) {
        return;
    }
    let rect = normalize(n.bounds);
    let stroke_pad = n.stroke.map_or(0.0, |s| s.width * 0.5);
    let (d, transform) = match &n.svg_path {
        Some(authored) => (authored.clone(), fit_transform(authored, rect)),
        None => {
            if n.points.len() < 2 {
                return;
            }
            let mut d = String::with_capacity(n.points.len() * 16);
            let _ = write!(
                d,
                "M{} {}",
                css::num(n.points[0].x),
                css::num(n.points[0].y)
            );
            for p in &n.points[1..] {
                let _ = write!(d, " L{} {}", css::num(p.x), css::num(p.y));
            }
            if n.path_closed {
                d.push_str(" Z");
            }
            (d, String::new())
        }
    };
    let doc_box = if n.svg_path.is_some() {
        padded_box(rect, stroke_pad)
    } else {
        padded_box(points_bounds(&n.points), stroke_pad)
    };
    open_svg(out, doc_box, origin);
    // An open pen stroke with no authored stroke still has to be
    // visible; the canvas hairlines it in the fill colour, so the fill
    // is redirected to the stroke channel rather than flooding the
    // polyline's implied interior.
    let hairline_only = n.svg_path.is_none() && !n.path_closed && n.stroke.is_none();
    let attrs = if hairline_only {
        format!(
            r#" fill="none" stroke="{}" stroke-width="1.5""#,
            css::color(n.fill.unwrap_or(Color::BLACK))
        )
    } else {
        paint_attrs(n)
    };
    let fill_rule = if n.even_odd_fill {
        r#" fill-rule="evenodd""#
    } else {
        ""
    };
    let _ = write!(
        out,
        r#"<path d="{}"{transform}{fill_rule}{attrs}/>"#,
        escape_xml(&d)
    );
    out.push_str("</svg>");
}

/// Open an `<svg>` whose viewBox is stated in absolute doc coordinates,
/// positioned by subtracting the board origin. `overflow:visible` keeps
/// a stroke that straddles the box edge from being clipped.
fn open_svg(out: &mut String, doc_box: Rect, origin: Point2D) {
    let mut style = String::new();
    css::place(
        &mut style,
        Rect {
            origin: Point2D::new(doc_box.origin.x - origin.x, doc_box.origin.y - origin.y),
            size: doc_box.size,
        },
    );
    css::decl(&mut style, "overflow", "visible");
    let _ = write!(
        out,
        r#"<svg class="n" style="{style}" viewBox="{} {} {} {}">"#,
        css::num(doc_box.origin.x),
        css::num(doc_box.origin.y),
        css::num(doc_box.size.x),
        css::num(doc_box.size.y)
    );
}

/// Grow a rect by `pad` on every side, and guarantee a positive extent
/// so the resulting `<svg>` has a renderable viewport.
fn padded_box(rect: Rect, pad: f32) -> Rect {
    let pad = pad.max(0.0);
    Rect {
        origin: Point2D::new(rect.origin.x - pad, rect.origin.y - pad),
        size: Point2D::new(
            (rect.size.x + pad * 2.0).max(1.0),
            (rect.size.y + pad * 2.0).max(1.0),
        ),
    }
}

fn points_bounds(points: &[Point2D]) -> Rect {
    let Some(first) = points.first() else {
        return Rect::ZERO;
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.x, first.y, first.x, first.y);
    for p in &points[1..] {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    Rect {
        origin: Point2D::new(min_x, min_y),
        size: Point2D::new(max_x - min_x, max_y - min_y),
    }
}

/// `fill` / `stroke` attributes for an SVG shape.
///
/// An absent fill is written as an explicit `fill="none"` rather than
/// left off: SVG's initial `fill` is opaque black, so omitting the
/// attribute would flood an unfilled shape solid.
fn paint_attrs(n: &SceneNode) -> String {
    let mut attrs = String::new();
    match n.fill {
        Some(fill) => {
            let _ = write!(attrs, r#" fill="{}""#, css::color(fill));
        }
        None => attrs.push_str(r#" fill="none""#),
    }
    if let Some(stroke) = n.stroke {
        let _ = write!(
            attrs,
            r#" stroke="{}" stroke-width="{}""#,
            css::color(stroke.color),
            css::num(stroke.width)
        );
        // The authored join and miter limit (#389): a polygon or path
        // corner bevels / rounds / mitres as on the canvas.
        use de_editor_ui::layout_scene::StrokeJoin;
        if let Some(join) = stroke.style.join {
            let join = match join {
                StrokeJoin::Miter => "miter",
                StrokeJoin::Round => "round",
                StrokeJoin::Bevel => "bevel",
            };
            let _ = write!(attrs, r#" stroke-linejoin="{join}""#);
        }
        if let Some(limit) = stroke.style.miter_limit {
            let _ = write!(attrs, r#" stroke-miterlimit="{}""#, css::num(limit));
        }
    }
    attrs
}

/// Scale + translate an imported `d` (authored in its own coordinate
/// space) into the node's resolved box — the same fit
/// `svg_export::svg_path_fit_transform` applies.
fn fit_transform(d: &str, rect: Rect) -> String {
    let Some((source_x, source_y, source_w, source_h)) = de_editor_core::svg_path_data_bounds(d)
    else {
        return format!(
            r#" transform="translate({} {})""#,
            css::num(rect.origin.x),
            css::num(rect.origin.y)
        );
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
    format!(
        r#" transform="matrix({} 0 0 {} {} {})""#,
        css::num(sx),
        css::num(sy),
        css::num(rect.origin.x - source_x * sx),
        css::num(rect.origin.y - source_y * sy)
    )
}

fn normalize(r: Rect) -> Rect {
    de_editor_ui::scene_bounds::normalize_rect(r)
}

#[cfg(test)]
#[path = "vector_tests.rs"]
mod tests;

/// A line or open path with end shapes (#389): its precomputed stroke
/// geometry as inline SVG, the same paths the SVG export writes, placed in
/// the node box with the node's flips. Returns `false` when it has none.
fn emit_stroke_geometry(out: &mut String, n: &SceneNode, origin: Point2D) -> bool {
    let (Some(stroke), Some(geometry)) = (n.stroke, n.stroke_geometry.as_deref()) else {
        return false;
    };
    let rect = normalize(n.bounds);
    let reach =
        stroke.width + de_editor_core::stroke_geometry::head_length(f64::from(stroke.width)) as f32;
    open_svg(out, padded_box(rect, reach), origin);
    let (sx, tx) = if n.flip_x {
        (-1.0, rect.origin.x + rect.size.x)
    } else {
        (1.0, rect.origin.x)
    };
    let (sy, ty) = if n.flip_y {
        (-1.0, rect.origin.y + rect.size.y)
    } else {
        (1.0, rect.origin.y)
    };
    let place = format!(
        r#" transform="matrix({sx} 0 0 {sy} {} {})""#,
        css::num(tx),
        css::num(ty)
    );
    if let (Some(fill), Some(d)) = (n.fill, n.svg_path.as_deref()) {
        // An imported open path's fill under its stroke, as the canvas
        // paints it (a pen path's open outline paints no fill).
        let rule = if n.even_odd_fill {
            r#" fill-rule="evenodd""#
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<path d="{}"{}{rule} fill="{}"/>"#,
            de_util::xml_escape::escape_xml(d),
            fit_transform(d, rect),
            css::color(fill)
        );
    }
    de_editor_ui::svg_export::write_stroke_geometry(out, n, geometry, &place);
    out.push_str("</svg>");
    true
}
