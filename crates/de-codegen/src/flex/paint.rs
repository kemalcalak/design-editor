//! Fills, strokes, corner radii, effects, blend modes, opacity and
//! transforms as CSS declarations.
//!
//! Conventions match the canvas renderer:
//! - fills are front-to-back, so the first fill is CSS's first (topmost)
//!   background layer;
//! - a linear gradient's `angle` is CSS's `<angle>` (0deg = bottom to top);
//!   radial / angular centres are fractions of the box;
//! - a stroke without `align` is centred on the edge. Strokes are drawn as an
//!   `outline` (or inset shadows for per-side widths), so they never change
//!   the box the layout computed;
//! - a shadow's blur passes through (CSS blur radius = 2 sigma, the canvas
//!   uses sigma = blur / 2); a layer blur's radius is halved for `blur()`.

use super::assets::Assets;
use super::ir::Decls;
use super::value::{color, color_alpha, px, round2};
use crate::{fmt_num, node_fills, node_stroke};
use jian_ops_schema::node::{CornerRadius, NumberOrExpression, PenNode, ScriptNode};
use jian_ops_schema::style::{
    BlendMode, GradientStop, ImageFillMode, PenEffect, PenFill, ShaderUniformValue, StrokeAlign,
    StrokeThickness,
};

/// The node's effect list, when its variant carries one.
pub(crate) fn node_effects(node: &PenNode) -> &[PenEffect] {
    let effects = match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => {
            n.container.effects.as_ref()
        }
        PenNode::Group(n) => n.container.effects.as_ref(),
        PenNode::Rectangle(n) => n.container.effects.as_ref(),
        PenNode::Ellipse(n) => n.effects.as_ref(),
        PenNode::Polygon(n) => n.effects.as_ref(),
        PenNode::Path(n) => n.effects.as_ref(),
        PenNode::Line(n) => n.effects.as_ref(),
        PenNode::Text(n) => n.effects.as_ref(),
        PenNode::Image(n) => n.effects.as_ref(),
        PenNode::TextInput(n) => n.effects.as_ref(),
        PenNode::TextArea(n) => n.effects.as_ref(),
        PenNode::Select(n) => n.effects.as_ref(),
        PenNode::Switch(n) => n.effects.as_ref(),
        PenNode::Checkbox(n) => n.effects.as_ref(),
        PenNode::Slider(n) => n.effects.as_ref(),
        PenNode::RadioGroup(n) => n.effects.as_ref(),
        PenNode::NumberInput(n) => n.effects.as_ref(),
        PenNode::Progress(n) => n.effects.as_ref(),
        PenNode::Tabs(n) => n.effects.as_ref(),
        PenNode::IconFont(_)
        | PenNode::Ref(_)
        | PenNode::Note(_)
        | PenNode::Context(_)
        | PenNode::Prompt(_)
        | PenNode::Browser(_) => None,
    };
    effects.map(Vec::as_slice).unwrap_or(&[])
}

/// CSS keyword for a blend mode; `None` for normal.
pub(crate) fn blend_keyword(mode: &BlendMode) -> Option<&'static str> {
    Some(match mode {
        BlendMode::Normal => return None,
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::Lighten => "lighten",
        BlendMode::Difference => "difference",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
        BlendMode::SoftLight => "soft-light",
        BlendMode::ColorDodge => "color-dodge",
        BlendMode::ColorBurn => "color-burn",
        BlendMode::HardLight => "hard-light",
        BlendMode::Exclusion => "exclusion",
        // WebKit-only keywords; other browsers ignore them and paint
        // source-over, which is the canvas's own fallback (#14).
        BlendMode::LinearBurn => "plus-darker",
        BlendMode::LinearDodge => "plus-lighter",
    })
}

fn stops(stops: &[GradientStop], opacity: f64) -> String {
    stops
        .iter()
        .map(|s| {
            format!(
                "{} {}%",
                color_alpha(&s.color, opacity),
                fmt_num(round2(f64::from(s.offset.clamp(0.0, 1.0)) * 100.0))
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn pct(v: Option<f32>) -> String {
    format!(
        "{}%",
        fmt_num(round2(f64::from(v.unwrap_or(0.5).clamp(0.0, 1.0)) * 100.0))
    )
}

/// One fill as a CSS background layer (an `<image>` or, for a lone solid,
/// a colour) plus its blend mode. `size` is the node's literal box when known
/// (a radial radius is a fraction of its longer side).
fn layer(
    fill: &PenFill,
    lone: bool,
    size: Option<(f64, f64)>,
    assets: &mut Assets,
    hint: &str,
) -> Option<(String, Option<&'static str>)> {
    let opacity = |o: Option<f32>| f64::from(o.unwrap_or(1.0));
    let (css, blend) = match fill {
        PenFill::Solid(b) => {
            let c = color_alpha(&b.color, opacity(b.opacity));
            let css = if lone {
                c
            } else {
                format!("linear-gradient({c}, {c})")
            };
            (css, b.blend_mode.as_ref())
        }
        PenFill::LinearGradient(b) => (
            format!(
                "linear-gradient({}deg, {})",
                fmt_num(round2(f64::from(b.angle.unwrap_or(0.0)))),
                stops(&b.stops, opacity(b.opacity))
            ),
            b.blend_mode.as_ref(),
        ),
        PenFill::RadialGradient(b) => {
            let r = f64::from(b.radius.unwrap_or(0.5).clamp(0.0, 1.0));
            let shape = match size {
                Some((w, h)) => format!("circle {}", px((w.max(h) * r).max(0.01))),
                None => "circle closest-side".into(),
            };
            (
                format!(
                    "radial-gradient({shape} at {} {}, {})",
                    pct(b.cx),
                    pct(b.cy),
                    stops(&b.stops, opacity(b.opacity))
                ),
                b.blend_mode.as_ref(),
            )
        }
        PenFill::AngularGradient(b) => (
            format!(
                "conic-gradient(from {}deg at {} {}, {})",
                fmt_num(round2(f64::from(b.rotation.unwrap_or(0.0)))),
                pct(b.cx),
                pct(b.cy),
                stops(&b.stops, opacity(b.opacity))
            ),
            b.blend_mode.as_ref(),
        ),
        // A Gouraud mesh has no CSS spelling: its first vertex colour stands
        // in, like the other flat exports do.
        PenFill::MeshGradient(b) => {
            let c = color_alpha(&b.stops.first()?.color, opacity(b.opacity));
            (
                if lone {
                    c
                } else {
                    format!("linear-gradient({c}, {c})")
                },
                b.blend_mode.as_ref(),
            )
        }
        // An SkSL program cannot run in CSS: its first colour uniform stands in.
        PenFill::Shader(b) => {
            let c = b.uniforms.as_ref()?.values().find_map(|v| match v {
                ShaderUniformValue::Color(c) if c.starts_with('#') || c.starts_with('$') => {
                    Some(color(c))
                }
                _ => None,
            })?;
            (
                if lone {
                    c
                } else {
                    format!("linear-gradient({c}, {c})")
                },
                b.blend_mode.as_ref(),
            )
        }
        PenFill::Image(b) => {
            let src = b.url.as_str();
            if src.trim().is_empty() {
                return None;
            }
            let url = assets.url(src, hint);
            let placement = match b.mode.as_ref() {
                Some(ImageFillMode::Fit) => "center / contain no-repeat".to_string(),
                Some(ImageFillMode::Stretch) => "0 0 / 100% 100% no-repeat".to_string(),
                Some(ImageFillMode::Tile) => match &b.original_size {
                    Some(o) => {
                        let s = f64::from(b.tile_scale.unwrap_or(1.0));
                        format!(
                            "0 0 / {} {} repeat",
                            px(f64::from(o.width) * s),
                            px(f64::from(o.height) * s)
                        )
                    }
                    None => "0 0 repeat".to_string(),
                },
                Some(ImageFillMode::Fill | ImageFillMode::Crop) | None => {
                    "center / cover no-repeat".to_string()
                }
            };
            (
                format!("url(\"{}\") {placement}", css_url(&url)),
                b.blend_mode.as_ref(),
            )
        }
    };
    Some((css, blend.and_then(blend_keyword)))
}

/// A URL safe inside a double-quoted CSS string.
fn css_url(url: &str) -> String {
    url.replace('\\', "%5C")
        .replace('"', "%22")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// `background` (+ `background-blend-mode`) for a fill stack.
pub(crate) fn apply_background(
    decls: &mut Decls,
    fills: &[PenFill],
    size: Option<(f64, f64)>,
    assets: &mut Assets,
    hint: &str,
) {
    let lone = fills.len() == 1;
    let layers: Vec<_> = fills
        .iter()
        .filter_map(|f| layer(f, lone, size, assets, hint))
        .collect();
    if layers.is_empty() {
        return;
    }
    decls.set(
        "background",
        layers
            .iter()
            .map(|(css, _)| css.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    );
    if layers.iter().any(|(_, b)| b.is_some()) {
        decls.set(
            "background-blend-mode",
            layers
                .iter()
                .map(|(_, b)| b.unwrap_or("normal"))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
}

/// Text paint: a solid fill is the text colour; a gradient or image stack is
/// clipped to the glyphs.
pub(crate) fn apply_text_fill(
    decls: &mut Decls,
    fills: &[PenFill],
    assets: &mut Assets,
    hint: &str,
) {
    match fills {
        [] => {}
        [PenFill::Solid(b)] => {
            decls.set(
                "color",
                color_alpha(&b.color, f64::from(b.opacity.unwrap_or(1.0))),
            );
        }
        _ => {
            let mut layers = Decls::default();
            apply_background(&mut layers, fills, None, assets, hint);
            if let Some(bg) = layers.get("background") {
                decls.set("background", bg.to_string());
                decls.set("-webkit-background-clip", "text");
                decls.set("background-clip", "text");
                decls.set("color", "transparent");
            }
        }
    }
}

/// The stroke as an `outline` (uniform) or inset shadows (per side), which
/// leave the layout box alone. Per-side shadows are returned for the caller
/// to merge into `box-shadow`.
pub(crate) fn apply_stroke(decls: &mut Decls, node: &PenNode) -> Vec<String> {
    let Some(stroke) = node_stroke(node) else {
        return Vec::new();
    };
    let stroke_color = stroke
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
    let sides = match &stroke.thickness {
        StrokeThickness::Uniform(w) => {
            let w = f64::from(*w);
            if w <= 0.0 {
                return Vec::new();
            }
            let style = match stroke.dash_pattern.as_deref() {
                Some([d, g, ..]) if (f64::from(*d) - w).abs() < 0.5 && d == g => "dotted",
                Some(p) if !p.is_empty() => "dashed",
                _ => "solid",
            };
            decls.set("outline", format!("{} {style} {stroke_color}", px(w)));
            let offset = match stroke.align {
                Some(StrokeAlign::Inside) => -w,
                Some(StrokeAlign::Outside) => 0.0,
                Some(StrokeAlign::Center) | None => -w / 2.0,
            };
            decls.set("outline-offset", px(offset));
            return Vec::new();
        }
        StrokeThickness::PerSide([t, r, b, l]) => [*t, *r, *b, *l],
        StrokeThickness::Sided(s) => [
            s.top.unwrap_or(0.0),
            s.right.unwrap_or(0.0),
            s.bottom.unwrap_or(0.0),
            s.left.unwrap_or(0.0),
        ],
    };
    let [t, r, b, l] = sides.map(f64::from);
    let mut shadows = Vec::new();
    for (w, x, y) in [(t, 0.0, t), (r, -r, 0.0), (b, 0.0, -b), (l, l, 0.0)] {
        if w > 0.0 {
            shadows.push(format!("inset {} {} 0 0 {stroke_color}", px(x), px(y)));
        }
    }
    shadows
}

/// `border-radius` from a uniform or per-corner radius (TL, TR, BR, BL —
/// CSS's own order).
pub(crate) fn radius_css(radius: &Option<CornerRadius>) -> Option<String> {
    match radius {
        Some(CornerRadius::Uniform(r)) if *r > 0.0 => Some(px(*r)),
        Some(CornerRadius::PerCorner(c)) if c.iter().any(|r| *r > 0.0) => Some(
            c.iter()
                .map(|r| px(r.max(0.0)))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

/// Shadows, blurs and background blurs. Text takes `text-shadow` (no spread,
/// no inset). `extra_shadows` are per-side stroke shadows to keep.
pub(crate) fn apply_effects(
    decls: &mut Decls,
    effects: &[PenEffect],
    text: bool,
    extra_shadows: Vec<String>,
) {
    let mut shadows = extra_shadows;
    let mut filter = Vec::new();
    let mut backdrop = Vec::new();
    for effect in effects {
        match effect {
            PenEffect::Shadow(s) if s.visible != Some(false) => {
                let c = color(&s.color);
                if text {
                    if s.inner != Some(true) {
                        shadows.push(format!(
                            "{} {} {} {c}",
                            px(f64::from(s.offset_x)),
                            px(f64::from(s.offset_y)),
                            px(f64::from(s.blur.max(0.0)))
                        ));
                    }
                } else {
                    shadows.push(format!(
                        "{}{} {} {} {} {c}",
                        if s.inner == Some(true) { "inset " } else { "" },
                        px(f64::from(s.offset_x)),
                        px(f64::from(s.offset_y)),
                        px(f64::from(s.blur.max(0.0))),
                        px(f64::from(s.spread))
                    ));
                }
            }
            PenEffect::Blur(b) if b.visible != Some(false) && b.radius > 0.0 => {
                filter.push(format!("blur({})", px(f64::from(b.radius) * 0.5)));
            }
            PenEffect::BackgroundBlur(b) if b.visible != Some(false) && b.radius > 0.0 => {
                backdrop.push(format!("blur({})", px(f64::from(b.radius) * 0.5)));
            }
            // Shader effects run SkSL; CSS has no equivalent.
            _ => {}
        }
    }
    if !shadows.is_empty() {
        decls.set(
            if text { "text-shadow" } else { "box-shadow" },
            shadows.join(", "),
        );
    }
    if !filter.is_empty() {
        decls.set("filter", filter.join(" "));
    }
    if !backdrop.is_empty() {
        let v = backdrop.join(" ");
        decls.set("-webkit-backdrop-filter", v.clone());
        decls.set("backdrop-filter", v);
    }
}

/// Opacity, blend mode, rotation and flips from the node base.
pub(crate) fn apply_compositing(decls: &mut Decls, node: &PenNode) {
    use de_editor_core::pen_node_ext::PenNodeExt;
    let base = node.base();
    match &base.opacity {
        Some(NumberOrExpression::Number(o)) if *o >= 1.0 => {}
        Some(o) => {
            if let Some(v) = super::value::number_or_expr_unitless(o) {
                decls.set("opacity", v);
            }
        }
        None => {}
    }
    if let Some(mode) = base.blend_mode.as_ref().and_then(blend_keyword) {
        decls.set("mix-blend-mode", mode);
    }
    let mut transform = Vec::new();
    let rotation = base.rotation.unwrap_or(0.0);
    if rotation.abs() > f64::EPSILON && rotation.is_finite() {
        transform.push(format!("rotate({}deg)", fmt_num(round2(rotation))));
    }
    if base.flip_x == Some(true) {
        transform.push("scaleX(-1)".into());
    }
    if base.flip_y == Some(true) {
        transform.push("scaleY(-1)".into());
    }
    if !transform.is_empty() {
        decls.set("transform", transform.join(" "));
    }
}

/// The fill stack of a node, empty when it carries none.
pub(crate) fn fills_of(node: &PenNode) -> &[PenFill] {
    node_fills(node).map(Vec::as_slice).unwrap_or(&[])
}
