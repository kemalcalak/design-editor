//! Fills, strokes, effects and blend modes for the pen.dev conversion.
//!
//! The output is already canonical where the shape differs (stroke, effects,
//! image fills). Solid colours and `type: "gradient"` objects are left for
//! the loader's legacy repairs (`payload/legacy_normalize.rs`), which have
//! handled those Pencil spellings since the 2.8 documents.

use serde_json::{json, Map, Value};

use super::glsl::Stage;
use super::nodes::{Ctx, Role};
use super::variables::{self, Theme};
use super::{images, mesh, shader, PenDevReport};

/// Replace a node's pen.dev `fill` (one fill or a list, colours as strings)
/// with a canonical fill list; drop the key when nothing survives.
pub(super) fn convert_fill_key(map: &mut Map<String, Value>, theme: &Theme, ctx: &mut Ctx<'_>) {
    let Some(fills) = map.remove("fill") else {
        return;
    };
    let size = node_size(map);
    let fills = convert_fills(fills, size, theme, ctx);
    if !fills.is_empty() {
        map.insert("fill".to_owned(), Value::Array(fills));
    }
}

/// pen.dev stacks fills bottom-up (the last one paints on top); the canonical
/// stack is top-down like CSS backgrounds (`canvas_viewport_fill_layers.rs`
/// paints it in reverse), so the order flips here.
fn convert_fills(
    fills: Value,
    size: Option<[f64; 2]>,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) -> Vec<Value> {
    let items = match fills {
        Value::Array(items) => items,
        single => vec![single],
    };
    let mut fills: Vec<Value> = items
        .into_iter()
        .filter_map(|fill| convert_fill(fill, size, theme, ctx))
        .collect();
    fills.reverse();
    fills
}

/// The node's box when both sides are plain numbers (shader `size`).
fn node_size(map: &Map<String, Value>) -> Option<[f64; 2]> {
    let side = |key| map.get(key).and_then(Value::as_f64).filter(|v| *v > 0.0);
    Some([side("width")?, side("height")?])
}

fn convert_fill(
    fill: Value,
    size: Option<[f64; 2]>,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) -> Option<Value> {
    let mut obj = match fill {
        Value::String(color) => return Some(json!({ "type": "solid", "color": color })),
        Value::Object(obj) => obj,
        _ => return None,
    };
    if !take_enabled(&mut obj, ctx.report) {
        return None;
    }
    convert_blend_mode(&mut obj, ctx.report);
    match obj.get("type").and_then(Value::as_str) {
        Some("color") => {
            obj.insert("type".to_owned(), Value::String("solid".to_owned()));
        }
        // Linear, radial and angular all map onto canonical gradients in
        // the legacy repair (`normalize_gradient_fill`), which reads
        // `rotation` as a clockwise CSS angle.
        Some("gradient") => clockwise_rotation(&mut obj),
        Some("image") => return images::convert_image_fill(obj, size, ctx),
        Some("shader") => return shader::convert(obj, Stage::Fill, size, theme, ctx),
        Some("mesh_gradient") => return mesh::convert(obj, theme, ctx),
        _ => {}
    }
    Some(Value::Object(obj))
}

/// pen.dev turns a gradient counter-clockwise (0° up, 90° left); the
/// canonical linear `angle` and angular `rotation` turn clockwise from up
/// like CSS, so 90° becomes 270°.
fn clockwise_rotation(obj: &mut Map<String, Value>) {
    if let Some(degrees) = obj.get("rotation").and_then(Value::as_f64) {
        let clockwise = (360.0 - degrees.rem_euclid(360.0)).rem_euclid(360.0);
        obj.insert("rotation".to_owned(), json!(clockwise));
    }
}

/// pen.dev stroke keys, folded into one canonical `stroke` object.
pub(super) const STROKE_KEYS: &[&str] = &[
    "stroke",
    "strokeWidth",
    "strokeLinecap",
    "strokeLinejoin",
    "strokeAlignment",
];

/// Fold pen.dev's `stroke` paint and its sibling `strokeWidth` /
/// `strokeLinecap` / `strokeLinejoin` / `strokeAlignment` into one canonical
/// `stroke` object.
///
/// An instance patch merges key by key, so the canonical `stroke` object
/// replaces the component's whole stroke: a patch of only the width (or
/// only the paint) takes the missing keys from the node it patches. A
/// partial patch with no source stroke to complete it is reported.
pub(super) fn convert_stroke(
    map: &mut Map<String, Value>,
    role: &Role,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) {
    if let Role::Patch(path) = role {
        complete_stroke_patch(map, path, theme, ctx);
    }
    let width = map.remove("strokeWidth");
    let cap = map.remove("strokeLinecap");
    let join = map.remove("strokeLinejoin");
    let align = map.remove("strokeAlignment");
    let partial_patch = matches!(role, Role::Patch(_))
        && (width.is_none() || !map.contains_key("stroke"))
        && (map.contains_key("stroke")
            || width.is_some()
            || cap.is_some()
            || join.is_some()
            || align.is_some());
    if partial_patch {
        ctx.report.note("partial stroke override");
    }
    let Some(stroke) = map.remove("stroke") else {
        return;
    };
    let size = node_size(map);
    let fills = convert_fills(stroke, size, theme, ctx);
    if fills.is_empty() {
        return;
    }
    let mut out = Map::new();
    out.insert("thickness".to_owned(), thickness(width, ctx.report));
    out.insert("fill".to_owned(), Value::Array(fills));
    let cap = cap.as_ref().and_then(Value::as_str).map(|cap| match cap {
        "butt" => "none",
        other => other,
    });
    let align = align
        .as_ref()
        .and_then(Value::as_str)
        .map(|align| match align {
            "inner" => "inside",
            "outer" => "outside",
            other => other,
        });
    let join = join.as_ref().and_then(Value::as_str);
    for (key, value) in [("cap", cap), ("join", join), ("align", align)] {
        if let Some(value) = value {
            out.insert(key.to_owned(), Value::String(value.to_owned()));
        }
    }
    map.insert("stroke".to_owned(), Value::Object(out));
}

/// Fill a patch that names some stroke keys with the patched node's own
/// values for the others, resolved under the patch's theme.
fn complete_stroke_patch(
    map: &mut Map<String, Value>,
    path: &str,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) {
    if !STROKE_KEYS.iter().any(|key| map.contains_key(*key)) {
        return;
    }
    let Some(base) = ctx.instance_bases.stroke(path) else {
        return;
    };
    let mut missing: Map<String, Value> = base
        .iter()
        .filter(|(key, _)| !map.contains_key(key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    variables::resolve_node(&mut missing, &ctx.variables, theme, ctx.report);
    map.extend(missing);
}

/// pen.dev `strokeWidth`: a number, a `$variable`, or per-side
/// `{ top, right, bottom, left }` (the canonical `Sided` shape).
fn thickness(width: Option<Value>, report: &mut PenDevReport) -> Value {
    match width {
        None => json!(1),
        Some(Value::Number(width)) => Value::Number(width),
        Some(Value::Object(sides)) => {
            let mut out = Map::new();
            for (side, value) in sides {
                match value {
                    Value::Number(_) => {
                        out.insert(side, value);
                    }
                    _ => report.note("variable-bound stroke width"),
                }
            }
            Value::Object(out)
        }
        Some(_) => {
            report.note("variable-bound stroke width");
            json!(1)
        }
    }
}

/// pen.dev `effect` (one or a list) → canonical `effects`.
pub(super) fn convert_effects(map: &mut Map<String, Value>, theme: &Theme, ctx: &mut Ctx<'_>) {
    let Some(effect) = map.remove("effect") else {
        return;
    };
    let size = node_size(map);
    let items = match effect {
        Value::Array(items) => items,
        single => vec![single],
    };
    let effects: Vec<Value> = items
        .into_iter()
        .filter_map(|effect| convert_effect(effect, size, theme, ctx))
        .collect();
    if !effects.is_empty() {
        map.insert("effects".to_owned(), Value::Array(effects));
    }
}

fn convert_effect(
    effect: Value,
    size: Option<[f64; 2]>,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) -> Option<Value> {
    let Value::Object(mut obj) = effect else {
        return None;
    };
    let report = &mut *ctx.report;
    if !take_enabled(&mut obj, report) {
        return None;
    }
    match obj.get("type").and_then(Value::as_str) {
        Some("shadow") => {
            let offset = obj.get("offset");
            let offset_x = number(offset.and_then(|o| o.get("x")), 0.0, report);
            let offset_y = number(offset.and_then(|o| o.get("y")), 0.0, report);
            let blur = number(obj.get("blur"), 0.0, report);
            let color = obj
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or("#00000040");
            let mut shadow = json!({
                "type": "shadow",
                "offsetX": offset_x,
                "offsetY": offset_y,
                "blur": blur,
                "spread": 0,
                "color": color,
            });
            if obj.get("shadowType").and_then(Value::as_str) == Some("inner") {
                shadow["inner"] = Value::Bool(true);
            }
            Some(shadow)
        }
        Some(kind @ ("blur" | "background_blur")) => Some(json!({
            "type": kind,
            "radius": number(obj.get("radius"), 0.0, report),
        })),
        Some("shader") => shader::convert(obj, Stage::Effect, size, theme, ctx),
        _ => {
            report.note("unknown effect");
            None
        }
    }
}

/// Map pen.dev's camelCase `blendMode` onto the canonical snake_case set;
/// modes the canonical renderer lacks are dropped and reported.
pub(super) fn convert_blend_mode(map: &mut Map<String, Value>, report: &mut PenDevReport) {
    let Some(Value::String(mode)) = map.get("blendMode") else {
        return;
    };
    let canonical = match mode.as_str() {
        mode @ ("normal" | "darken" | "multiply" | "screen" | "overlay" | "difference"
        | "exclusion" | "hue" | "saturation" | "color" | "luminosity") => Some(mode),
        "light" => Some("lighten"),
        "colorBurn" => Some("color_burn"),
        "colorDodge" => Some("color_dodge"),
        "softLight" => Some("soft_light"),
        "hardLight" => Some("hard_light"),
        "linearBurn" => Some("linear_burn"),
        "linearDodge" => Some("linear_dodge"),
        _ => None,
    };
    match canonical.map(str::to_owned) {
        Some(canonical) => {
            map.insert("blendMode".to_owned(), Value::String(canonical));
        }
        None => {
            map.remove("blendMode");
            report.note("blend mode");
        }
    }
}

/// Remove a fill / effect `enabled` flag; `false` means drop the entry.
fn take_enabled(obj: &mut Map<String, Value>, report: &mut PenDevReport) -> bool {
    match obj.remove("enabled") {
        Some(Value::Bool(enabled)) => enabled,
        Some(_) => {
            report.note("variable-bound paint toggle");
            true
        }
        None => true,
    }
}

fn number(value: Option<&Value>, default: f64, report: &mut PenDevReport) -> f64 {
    match value {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(_) => {
            report.note("variable-bound effect value");
            default
        }
        None => default,
    }
}
