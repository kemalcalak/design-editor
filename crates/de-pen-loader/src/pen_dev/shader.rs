//! pen.dev `shader` fills and effects → canonical SkSL shader paints.
//!
//! A pen.dev shader paint names a GLSL file next to the document
//! (`"url": "shaders/scene.glsl?v=6"`) and its uniform values. The file is
//! read once per document, translated per paint ([`super::glsl`]: the
//! uniform values decide which vectors are colours and what a texture
//! reads), and the SkSL is compile-checked where Skia is linked. A shader
//! that cannot be read, translated or compiled is dropped and reported, as
//! before this conversion existed.
//!
//! Uniform values: numbers and arrays as written, colours as hex strings,
//! a `$variable` kept as a reference the canonical resolver reads per theme
//! (a number or boolean variable on a scalar, a colour variable on a
//! colour; any other reference, or a chain, is resolved for the node's
//! theme and reported), a missing value from the GLSL `@default`, else zero
//! / black.

use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{json, Map, Value};

use super::glsl::{self, Slot, Stage, Texture, UserUniform};
use super::images;
use super::nodes::Ctx;
use super::variables::Theme;

/// GLSL sources and compile results already seen in this conversion.
#[derive(Default)]
pub(super) struct ShaderCache {
    /// Relative path → source text (`None` once known missing).
    sources: HashMap<String, Option<Rc<str>>>,
    /// Image texture URL → `data:` URL (`None` once known missing).
    images: HashMap<String, Option<String>>,
    /// SkSL → whether it compiled.
    #[cfg(feature = "sksl-check")]
    compiled: HashMap<String, bool>,
}

/// Convert one enabled pen.dev shader paint, or `None` (reported) when it
/// cannot be carried over. `size` is the node's numeric box, if known.
pub(super) fn convert(
    mut obj: Map<String, Value>,
    stage: Stage,
    size: Option<[f64; 2]>,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) -> Option<Value> {
    let (missing, compile_failed) = match stage {
        Stage::Fill => (
            "shader fill (GLSL file missing)",
            "shader fill (SkSL compile failed)",
        ),
        Stage::Effect => (
            "shader effect (GLSL file missing)",
            "shader effect (SkSL compile failed)",
        ),
    };
    let Some(source) = obj
        .remove("url")
        .and_then(|url| url.as_str().and_then(|url| read_source(url, ctx)))
    else {
        ctx.report.note(missing);
        return None;
    };
    let (values, references) = resolve_values(obj.remove("uniforms"), theme, ctx);
    let translation = match glsl::translate(&source, stage, &values) {
        Ok(translation) => translation,
        Err(error) => {
            ctx.report.note(error.label(stage));
            return None;
        }
    };
    if !compiles(&translation.sksl, ctx) {
        ctx.report.note(compile_failed);
        return None;
    }
    let mut uniforms = Map::new();
    let [width, height] = size.unwrap_or([1.0, 1.0]);
    // Bound to the node box at render time; written so `fragCoord` maps
    // onto it (`runtime_shader_local_matrix`).
    uniforms.insert("size".to_owned(), json!([width, height]));
    for user in &translation.uniforms {
        let value = values.get(&user.glsl_name);
        let live = references
            .get(&user.glsl_name)
            .filter(|_| binds_live(&user.slot, value));
        let canonical = match live {
            // The canonical resolver reads it per theme at render time.
            Some(reference) => Some(Value::String(format!("${reference}"))),
            None => {
                if references.contains_key(&user.glsl_name) {
                    ctx.report.note("shader uniform variable (fixed value)");
                }
                uniform_value(user, value, ctx)
            }
        };
        if let Some(value) = canonical {
            uniforms.insert(user.sksl_name.clone(), value);
        }
        if let Slot::Texture(Texture::Image(url)) = &user.slot {
            if translation.sized_images.contains(&user.sksl_name) {
                // `textureSize` of the image; 1 x 1 when it cannot be read.
                let [w, h] = image_size(url, ctx).unwrap_or([1, 1]);
                uniforms.insert(
                    glsl::image_size_uniform(&user.sksl_name),
                    json!([w as f64, h as f64]),
                );
            }
        }
    }
    let mut out = Map::new();
    out.insert("type".to_owned(), Value::String("shader".to_owned()));
    out.insert("sksl".to_owned(), Value::String(translation.sksl));
    out.insert("uniforms".to_owned(), Value::Object(uniforms));
    if stage == Stage::Fill {
        for key in ["opacity", "blendMode"] {
            if let Some(value) = obj.remove(key) {
                out.insert(key.to_owned(), value);
            }
        }
    }
    Some(Value::Object(out))
}

/// The GLSL text at `url` (query and fragment ignored), read once.
fn read_source(url: &str, ctx: &mut Ctx<'_>) -> Option<Rc<str>> {
    let relative = images::relative_path(url);
    if let Some(cached) = ctx.shaders.sources.get(relative) {
        return cached.clone();
    }
    let source: Option<Rc<str>> = images::is_inside_document_folder(relative)
        .then(|| (ctx.assets)(relative))
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(Rc::from);
    ctx.shaders
        .sources
        .insert(relative.to_owned(), source.clone());
    source
}

/// The pen.dev `uniforms` map with every `$variable` replaced by its value
/// for `theme` (the translation needs values: they decide colours and
/// textures), plus the variable each single-hop reference named. An
/// unknown variable is removed so the `@default` applies.
fn resolve_values(
    uniforms: Option<Value>,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) -> (Map<String, Value>, HashMap<String, String>) {
    let Some(Value::Object(mut uniforms)) = uniforms else {
        return (Map::new(), HashMap::new());
    };
    let mut unresolved = Vec::new();
    let mut references = HashMap::new();
    for (name, value) in uniforms.iter_mut() {
        let first = value
            .as_str()
            .and_then(|s| s.strip_prefix('$'))
            .map(str::to_owned);
        // A variable may name another one; follow a short chain.
        let mut hops = 0;
        while let Some(reference) = value.as_str().and_then(|s| s.strip_prefix('$')) {
            match ctx.variables.value(reference, theme).filter(|_| hops < 8) {
                Some(resolved) => {
                    *value = resolved;
                    hops += 1;
                }
                None => {
                    unresolved.push(name.clone());
                    break;
                }
            }
        }
        if let Some(first) = first.filter(|_| !unresolved.contains(name)) {
            if hops == 1 {
                references.insert(name.clone(), first);
            } else {
                // The render-time resolver follows no chains.
                ctx.report.note("shader uniform variable (fixed value)");
            }
        }
    }
    for name in unresolved {
        uniforms.remove(&name);
        ctx.report.note("unresolved variable");
    }
    (uniforms, references)
}

/// Whether a uniform of `slot` can keep a variable whose value under the
/// node's theme is `value`: a number (or boolean) variable on a scalar, a
/// colour variable on a colour. The render-time resolver writes the same
/// kinds (`variables_resolve`, shader uniforms).
fn binds_live(slot: &Slot, value: Option<&Value>) -> bool {
    match (slot, value) {
        (Slot::Float | Slot::Int(1) | Slot::Bool(1), Some(Value::Number(_) | Value::Bool(_))) => {
            true
        }
        (Slot::Colour, Some(text @ Value::String(_))) => colour(text).is_some(),
        _ => false,
    }
}

#[cfg(feature = "sksl-check")]
fn compiles(sksl: &str, ctx: &mut Ctx<'_>) -> bool {
    if let Some(&compiled) = ctx.shaders.compiled.get(sksl) {
        return compiled;
    }
    let compiled = skia_safe::RuntimeEffect::make_for_shader(sksl, None).is_ok();
    ctx.shaders.compiled.insert(sksl.to_owned(), compiled);
    compiled
}

/// Without Skia (the CanvasKit web build, the CLI) the translation is
/// trusted; a program the renderer then rejects paints its fallback colour.
#[cfg(not(feature = "sksl-check"))]
fn compiles(_sksl: &str, _ctx: &mut Ctx<'_>) -> bool {
    true
}

/// The canonical value for `user`: the paint's value, else the GLSL
/// `@default`, else zero / black. `None` leaves the uniform unbound (an
/// image texture that cannot be read, a node / backdrop texture).
fn uniform_value(user: &UserUniform, value: Option<&Value>, ctx: &mut Ctx<'_>) -> Option<Value> {
    let default = user.default.as_deref();
    match &user.slot {
        Slot::Float => {
            let n = value
                .and_then(number)
                .or_else(|| default.and_then(|d| d.trim().parse::<f64>().ok()))
                .unwrap_or(0.0);
            Some(json!(n))
        }
        Slot::Vector(len) => Some(json!(vector_value(value, default, *len))),
        Slot::Int(len) | Slot::Bool(len) => {
            let whole: Vec<f64> = vector_value(value, default, *len)
                .into_iter()
                .map(|n| match &user.slot {
                    Slot::Bool(_) => f64::from(u8::from(n != 0.0)),
                    _ => n.round(),
                })
                .collect();
            match whole.as_slice() {
                [single] => Some(json!(single)),
                _ => Some(json!(whole)),
            }
        }
        Slot::Colour => {
            let colour = value
                .and_then(colour)
                .or_else(|| default.and_then(|d| colour(&Value::String(d.trim().to_owned()))))
                .unwrap_or_else(|| "#000000".to_owned());
            Some(Value::String(colour))
        }
        Slot::Texture(Texture::Image(url)) => image_source(url, ctx).map(Value::String),
        Slot::Texture(Texture::Object | Texture::Background | Texture::Sdf) => None,
    }
}

/// `len` numbers from the paint's value, else the `@default`, else zeros;
/// a single number fills every component.
fn vector_value(value: Option<&Value>, default: Option<&str>, len: usize) -> Vec<f64> {
    let numbers = value
        .and_then(numbers)
        .or_else(|| default.map(default_numbers))
        .unwrap_or_default();
    let mut numbers: Vec<f64> = match numbers.as_slice() {
        [single] => vec![*single; len],
        _ => numbers,
    };
    numbers.resize(len, 0.0);
    numbers
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn numbers(value: &Value) -> Option<Vec<f64>> {
    match value {
        Value::Array(items) => items.iter().map(number).collect(),
        Value::String(s) => Some(default_numbers(s)),
        other => number(other).map(|n| vec![n]),
    }
}

/// `@default 0.5, 1` → `[0.5, 1.0]`; unparsable parts read as 0.
fn default_numbers(text: &str) -> Vec<f64> {
    text.trim()
        .trim_start_matches(['[', '('])
        .trim_end_matches([']', ')'])
        .split([',', ' '])
        .filter(|part| !part.trim().is_empty())
        .map(|part| match part.trim() {
            "true" => 1.0,
            "false" => 0.0,
            part => part.parse().unwrap_or(0.0),
        })
        .collect()
}

/// A colour uniform value as a hex string: a colour string as written
/// when the loader can parse it, a `[r, g, b(, a)]` array in 0..1 as hex.
fn colour(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => crate::style_payload::parse_color(s).map(|_| s.trim().to_owned()),
        Value::Array(_) => {
            let parts = numbers(value)?;
            if !(3..=4).contains(&parts.len()) {
                return None;
            }
            let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let mut hex = String::from("#");
            for part in &parts {
                hex.push_str(&format!("{:02x}", byte(*part)));
            }
            Some(hex)
        }
        _ => None,
    }
}

/// An image texture's source for a shader image child: URLs as written, a
/// relative file inlined as a `data:` URL (shader image uniforms take no
/// `op-image:` table reference).
fn image_source(url: &str, ctx: &mut Ctx<'_>) -> Option<String> {
    if url.starts_with("data:") || url.starts_with("http://") || url.starts_with("https://") {
        return Some(url.to_owned());
    }
    if let Some(cached) = ctx.shaders.images.get(url) {
        return cached.clone();
    }
    let source = images::file_data_url(url, ctx.assets);
    if source.is_none() {
        ctx.report.note_missing_image(url);
    }
    ctx.shaders.images.insert(url.to_owned(), source.clone());
    source
}

/// The pixel size of the image at `url`: a `data:` URL or a file next to
/// the document (a remote image's size is unknown at import).
fn image_size(url: &str, ctx: &mut Ctx<'_>) -> Option<[u32; 2]> {
    let bytes = if let Some(data) = url.strip_prefix("data:") {
        let (_, encoded) = data.split_once(";base64,")?;
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .ok()?
    } else {
        let relative = images::relative_path(url);
        if !images::is_inside_document_folder(relative) {
            return None;
        }
        (ctx.assets)(relative)?
    };
    let (w, h) = de_util::image_dimensions::encoded_image_dimensions(&bytes)?;
    Some([w, h])
}

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
