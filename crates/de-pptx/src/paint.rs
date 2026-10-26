//! Fills and outlines: DrawingML `solidFill` / `gradFill` / `blipFill` /
//! `pattFill` / `noFill` and `a:ln` as the schema's fill and stroke JSON.

use roxmltree::Node;
use serde_json::{json, Value};

use crate::color::{ColorContext, Rgba};
use crate::context::{round, Ctx, PX_PER_EMU};
use crate::media::MediaMiss;
use crate::package::Relationships;
use crate::xml;

/// What a property list says about a fill.
#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    /// Nothing authored here: keep looking (placeholder, style).
    Unset,
    /// `noFill`.
    None,
    /// `grpFill`: the enclosing group's fill.
    Group,
    Paint(Value),
}

const FILL_TAGS: [&str; 6] = [
    "noFill",
    "solidFill",
    "gradFill",
    "blipFill",
    "pattFill",
    "grpFill",
];

/// The fill authored directly in `parent` (an `spPr`, `bgPr`, `tcPr`, …).
pub fn read_fill(ctx: &mut Ctx<'_, '_, '_>, rels: &Relationships, parent: Node<'_, '_>) -> Fill {
    let Some(el) = xml::elements(parent).find(|n| FILL_TAGS.contains(&n.tag_name().name())) else {
        return Fill::Unset;
    };
    let colors = ctx.colors;
    match el.tag_name().name() {
        "noFill" => Fill::None,
        "grpFill" => Fill::Group,
        "solidFill" => colors
            .color_in(el)
            .map_or(Fill::Unset, |c| Fill::Paint(solid(c))),
        "gradFill" => gradient(el, &colors).map_or(Fill::Unset, Fill::Paint),
        "pattFill" => {
            ctx.warnings
                .add("pattern fill (shown as its foreground colour)");
            xml::child(el, "fgClr")
                .and_then(|fg| colors.color_in(fg))
                .map_or(Fill::Unset, |c| Fill::Paint(solid(c)))
        }
        "blipFill" => image_fill(ctx, rels, el).map_or(Fill::Unset, Fill::Paint),
        _ => Fill::Unset,
    }
}

pub fn solid(c: Rgba) -> Value {
    let mut fill = json!({ "type": "solid", "color": c.hex() });
    if c.a < 0.999 {
        fill["opacity"] = json!(round(c.a));
    }
    fill
}

fn gradient(el: Node<'_, '_>, colors: &ColorContext<'_>) -> Option<Value> {
    let mut stops: Vec<(f64, Rgba)> = xml::child(el, "gsLst")
        .into_iter()
        .flat_map(|list| xml::children(list, "gs"))
        .filter_map(|gs| Some((xml::attr_f64(gs, "pos")? / 100_000.0, colors.color_in(gs)?)))
        .collect();
    if stops.is_empty() {
        return None;
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    if stops.len() == 1 {
        return Some(solid(stops[0].1));
    }
    let stops: Vec<Value> = stops
        .iter()
        .map(|(pos, c)| json!({ "offset": round(*pos), "color": c.hex_with_alpha() }))
        .collect();
    if let Some(path) = xml::child(el, "path") {
        // The centre of `fillToRect` is the focus PowerPoint grows the
        // ramp from; the export writes the same rect back.
        let (cx, cy) = xml::child(path, "fillToRect").map_or((0.5, 0.5), |r| {
            let side = |name: &str| xml::attr_f64(r, name).unwrap_or(0.0) / 100_000.0;
            (
                (side("l") + 1.0 - side("r")) / 2.0,
                (side("t") + 1.0 - side("b")) / 2.0,
            )
        });
        return Some(json!({
            "type": "radial_gradient", "cx": round(cx), "cy": round(cy), "stops": stops,
        }));
    }
    // DrawingML measures clockwise from due east; `.de` follows CSS
    // (0° points up), so the turn is +90°, the export's inverse.
    let angle = xml::child(el, "lin")
        .and_then(|lin| xml::attr_f64(lin, "ang"))
        .map_or(0.0, |a| a / 60_000.0);
    Some(json!({
        "type": "linear_gradient",
        "angle": round((angle + 90.0).rem_euclid(360.0)),
        "stops": stops,
    }))
}

/// A picture fill. `stretch` fills the box like the slide shows it,
/// `tile` repeats.
fn image_fill(ctx: &mut Ctx<'_, '_, '_>, rels: &Relationships, el: Node<'_, '_>) -> Option<Value> {
    let url = image_url(ctx, rels, el)?;
    let mode = if xml::child(el, "tile").is_some() {
        "tile"
    } else {
        "stretch"
    };
    Some(json!({ "type": "image", "url": url, "mode": mode }))
}

/// The `data:` URL of a `blipFill`'s `a:blip`, warning when it is missing
/// or in a format the canvas cannot decode.
pub fn image_url(
    ctx: &mut Ctx<'_, '_, '_>,
    rels: &Relationships,
    blip_fill: Node<'_, '_>,
) -> Option<String> {
    let blip = xml::child(blip_fill, "blip")?;
    let rel_id = xml::rel_attr(blip, "embed")?;
    match ctx.media.data_url(rels, rel_id) {
        Ok(url) => Some(url),
        Err(MediaMiss::Missing) => {
            ctx.warnings.add("missing image");
            None
        }
        Err(MediaMiss::Unsupported(ext)) => {
            ctx.warnings
                .add(format!("{} image (not supported)", ext.to_uppercase()));
            None
        }
    }
}

/// What a property list says about an outline.
pub enum Line {
    Unset,
    None,
    Stroke(Value),
}

/// `a:ln` inside `sp_pr`. A line with a width but no fill of its own
/// takes `fallback` (the style's `lnRef` colour).
pub fn read_line(
    sp_pr: Option<Node<'_, '_>>,
    colors: &ColorContext<'_>,
    fallback: Option<Rgba>,
) -> Line {
    let Some(ln) = sp_pr.and_then(|p| xml::child(p, "ln")) else {
        return Line::Unset;
    };
    if xml::child(ln, "noFill").is_some() {
        return Line::None;
    }
    let color = xml::child(ln, "solidFill")
        .and_then(|f| colors.color_in(f))
        .or_else(|| xml::path(ln, &["gradFill", "gsLst", "gs"]).and_then(|gs| colors.color_in(gs)))
        .or(fallback);
    let Some(color) = color else {
        return Line::Unset;
    };
    // An outline without `w` is PowerPoint's default 0.75 pt (9525 EMU).
    let width = xml::attr_f64(ln, "w").unwrap_or(9525.0) * PX_PER_EMU;
    Line::Stroke(stroke(ln, color, width))
}

/// The `lnRef` index of the theme's thinnest line.
pub const THIN_LINE: i64 = 1;

/// A stroke from the style's `lnRef` alone: its colour, and the theme's
/// thin / medium / thick widths (6350 / 12700 / 19050 EMU by default).
pub fn style_line(idx: i64, color: Rgba) -> Option<Value> {
    if idx <= 0 {
        return None;
    }
    let width = [6350.0, 12_700.0, 19_050.0][(idx as usize - 1).min(2)] * PX_PER_EMU;
    Some(stroke_json(color, width))
}

fn stroke_json(color: Rgba, width: f64) -> Value {
    json!({
        "thickness": round(width.max(0.1)),
        "align": "center",
        "fill": [solid(color)],
    })
}

fn stroke(ln: Node<'_, '_>, color: Rgba, width: f64) -> Value {
    let mut out = stroke_json(color, width);
    match ln.attribute("cap") {
        Some("rnd") => out["cap"] = json!("round"),
        Some("sq") => out["cap"] = json!("square"),
        _ => {}
    }
    if xml::child(ln, "round").is_some() {
        out["join"] = json!("round");
    } else if xml::child(ln, "bevel").is_some() {
        out["join"] = json!("bevel");
    } else if xml::child(ln, "miter").is_some() {
        out["join"] = json!("miter");
    }
    let dash = xml::child(ln, "prstDash").and_then(|d| d.attribute("val"));
    // Preset dashes are in multiples of the line width.
    let pattern: Option<&[f64]> = match dash {
        Some("dash") => Some(&[4.0, 3.0]),
        Some("sysDash") => Some(&[3.0, 1.0]),
        Some("dot") | Some("sysDot") => Some(&[1.0, 1.0]),
        Some("lgDash") => Some(&[8.0, 3.0]),
        Some("dashDot") | Some("sysDashDot") => Some(&[4.0, 3.0, 1.0, 3.0]),
        Some("lgDashDot") => Some(&[8.0, 3.0, 1.0, 3.0]),
        Some("lgDashDotDot") | Some("sysDashDotDot") => Some(&[8.0, 3.0, 1.0, 3.0, 1.0, 3.0]),
        _ => None,
    };
    if let Some(pattern) = pattern {
        let w = width.max(1.0);
        out["dashPattern"] = json!(pattern.iter().map(|p| round(p * w)).collect::<Vec<_>>());
    }
    out
}

/// The colour of a style reference (`fillRef`, `lnRef`, `fontRef`,
/// `effectRef`), and its index.
pub fn style_ref(
    shape: Node<'_, '_>,
    name: &str,
    colors: &ColorContext<'_>,
) -> Option<(i64, Option<Rgba>)> {
    let r = xml::path(shape, &["style", name])?;
    let idx = xml::attr_i64(r, "idx").unwrap_or(0);
    Some((idx, colors.color_in(r)))
}
