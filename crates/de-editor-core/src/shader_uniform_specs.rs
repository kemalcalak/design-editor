//! The user uniforms a shader program declares, with their annotations
//! (#164).
//!
//! The panel shows one control per user uniform. What a control looks like
//! comes from the doc comment right in front of the declaration, the
//! pen.dev convention the `.pen` import keeps in the translated SkSL (#163):
//!
//! ```text
//! /**
//!  * @label Speed
//!  * @range 0, 2
//!  * @default 0.5
//!  */
//! uniform float u_speed;
//! ```
//!
//! - `@label` names the row (else the uniform name);
//! - `@range a, b` (or `@min a` / `@max b`) gives a float a slider;
//! - `@color` (or SkSL's `layout(color)`) makes a `float3` / `float4` a
//!   colour;
//! - `@default` is the value a missing uniform takes;
//! - `@time` / `@mouse` / `@resolution` mark canvas-bound uniforms, which
//!   get no row (nor do `size`, `time`, `mouse` and `uniform shader`
//!   children, which the canvas binds itself).
//!
//! A block comment (`/** */` or `/* */`) or a run of `//` lines counts when
//! only whitespace separates it from the `uniform` statement. The schema
//! has no field for the annotations, so they are read from the source every
//! time; the source stays the single source of truth.

use std::collections::BTreeMap;

use jian_ops_schema::style::ShaderUniformValue;

/// Uniform names the canvas binds itself (the layout scene and the shader
/// clock / pointer), never a panel row.
pub const CANVAS_BOUND_UNIFORMS: [&str; 3] = ["size", "time", "mouse"];

/// What control a user uniform gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniformKind {
    /// One float: a slider plus a number with a range, else a number.
    Float,
    /// A float vector of this many components (2–4): one number each.
    Vector(usize),
    /// A colour bound as `arity` floats (3 or 4). A 4-component colour is
    /// stored as a hex string (the backends expand it); a 3-component one
    /// as three floats, which is what a `float3` uniform binds.
    Color { arity: usize },
}

impl UniformKind {
    /// Number fields the row edits: one per vector component, one for a
    /// float, none for a colour (it edits through the picker).
    pub fn number_fields(self) -> usize {
        match self {
            Self::Float => 1,
            Self::Vector(n) => n,
            Self::Color { .. } => 0,
        }
    }
}

/// One user uniform of a shader program.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderUniformSpec {
    pub name: String,
    pub kind: UniformKind,
    /// `@label` text, if any.
    pub label: Option<String>,
    /// `@range a, b` (or `@min` / `@max`), ordered low → high.
    pub range: Option<(f32, f32)>,
    /// Raw `@default` text, if any.
    pub default: Option<String>,
}

impl ShaderUniformSpec {
    /// The row label: `@label`, else the uniform name.
    pub fn display_label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    /// The uniform's value: the stored one when it fits the kind, else the
    /// `@default`, else zero / black.
    pub fn value_in(
        &self,
        values: Option<&BTreeMap<String, ShaderUniformValue>>,
    ) -> ShaderUniformValue {
        values
            .and_then(|map| map.get(&self.name))
            .and_then(|value| self.coerce(value))
            .or_else(|| {
                self.default
                    .as_deref()
                    .and_then(|text| self.parse_default(text))
            })
            .unwrap_or_else(|| self.zero())
    }

    /// `value` in this uniform's stored form, or `None` when it cannot be
    /// one (wrong arity, not a colour, non-finite).
    pub fn coerce(&self, value: &ShaderUniformValue) -> Option<ShaderUniformValue> {
        match (self.kind, value) {
            (UniformKind::Float, ShaderUniformValue::Float(v)) if v.is_finite() => {
                Some(ShaderUniformValue::Float(*v))
            }
            (UniformKind::Float, ShaderUniformValue::Vec(v))
                if v.len() == 1 && v[0].is_finite() =>
            {
                Some(ShaderUniformValue::Float(v[0]))
            }
            (UniformKind::Vector(n), ShaderUniformValue::Vec(v))
                if v.len() == n && v.iter().all(|c| c.is_finite()) =>
            {
                Some(ShaderUniformValue::Vec(v.clone()))
            }
            (UniformKind::Color { arity: 4 }, ShaderUniformValue::Color(hex)) => {
                rgba(hex).map(|_| ShaderUniformValue::Color(hex.clone()))
            }
            (UniformKind::Color { arity: 4 }, ShaderUniformValue::Vec(v)) if v.len() == 4 => {
                Some(ShaderUniformValue::Color(hex_of(v)))
            }
            (UniformKind::Color { arity: 3 }, ShaderUniformValue::Vec(v))
                if v.len() == 3 && v.iter().all(|c| c.is_finite()) =>
            {
                Some(ShaderUniformValue::Vec(v.clone()))
            }
            (UniformKind::Color { arity: 3 }, ShaderUniformValue::Color(hex)) => {
                rgba(hex).map(|[r, g, b, _]| ShaderUniformValue::Vec(vec![r, g, b]))
            }
            _ => None,
        }
    }

    /// The colour this uniform holds as `#RRGGBB[AA]`, when it is one.
    pub fn hex_in(&self, values: Option<&BTreeMap<String, ShaderUniformValue>>) -> Option<String> {
        if !matches!(self.kind, UniformKind::Color { .. }) {
            return None;
        }
        Some(match self.value_in(values) {
            ShaderUniformValue::Color(hex) => hex,
            ShaderUniformValue::Vec(v) => hex_of(&v),
            ShaderUniformValue::Float(_) => return None,
        })
    }

    /// The numbers the row's fields show: one for a float, one per
    /// component for a vector, none for a colour.
    pub fn numbers_in(&self, values: Option<&BTreeMap<String, ShaderUniformValue>>) -> Vec<f32> {
        match (self.kind, self.value_in(values)) {
            (UniformKind::Float, ShaderUniformValue::Float(v)) => vec![v],
            (UniformKind::Vector(_), ShaderUniformValue::Vec(v)) => v,
            _ => Vec::new(),
        }
    }

    fn parse_default(&self, text: &str) -> Option<ShaderUniformValue> {
        let text = text.trim();
        if text.starts_with('#') {
            return self.coerce(&ShaderUniformValue::Color(text.to_owned()));
        }
        let numbers: Vec<f32> = text
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<f32>().ok())
            .collect::<Option<_>>()?;
        match self.kind {
            UniformKind::Float => numbers.first().map(|v| ShaderUniformValue::Float(*v)),
            UniformKind::Vector(n) | UniformKind::Color { arity: n } => {
                // `@default 1` on a vector fills every component.
                let filled = match numbers.as_slice() {
                    [one] => vec![*one; n],
                    _ if numbers.len() == n => numbers,
                    _ => return None,
                };
                self.coerce(&ShaderUniformValue::Vec(filled))
            }
        }
    }

    fn zero(&self) -> ShaderUniformValue {
        match self.kind {
            UniformKind::Float => ShaderUniformValue::Float(0.0),
            UniformKind::Vector(n) => ShaderUniformValue::Vec(vec![0.0; n]),
            UniformKind::Color { arity: 4 } => ShaderUniformValue::Color("#000000".to_owned()),
            UniformKind::Color { arity } => ShaderUniformValue::Vec(vec![0.0; arity]),
        }
    }
}

/// `#RRGGBB[AA]` → 0..1 RGBA (sRGB bytes / 255).
pub(crate) fn rgba(hex: &str) -> Option<[f32; 4]> {
    de_util::hex_color::parse_hex_rgba_f32(hex, de_util::hex_color::HexOptions::LENIENT)
}

/// 0..1 components → `#RRGGBB`, plus `AA` when a fourth component is
/// below one.
pub(crate) fn hex_of(components: &[f32]) -> String {
    let byte = |i: usize| {
        let v = components.get(i).copied().unwrap_or(1.0);
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    let mut hex = format!("#{:02x}{:02x}{:02x}", byte(0), byte(1), byte(2));
    if components.len() > 3 && byte(3) < 255 {
        hex.push_str(&format!("{:02x}", byte(3)));
    }
    hex
}

/// The user uniforms `sksl` declares, in declaration order. `values` (the
/// body's uniform map) decides whether an unannotated `float4` holding a
/// hex string is a colour (the panel presets declare their colours so).
pub fn uniform_specs(
    sksl: &str,
    values: Option<&BTreeMap<String, ShaderUniformValue>>,
) -> Vec<ShaderUniformSpec> {
    let mut out = Vec::new();
    for statement in uniform_statements(sksl) {
        let notes = Notes::parse(statement.comment);
        if notes.canvas_bound {
            continue;
        }
        let Some(kind) = kind_of(statement.ty) else {
            continue;
        };
        for name in statement.names {
            if CANVAS_BOUND_UNIFORMS.contains(&name) {
                continue;
            }
            let stored_colour = matches!(
                values.and_then(|map| map.get(name)),
                Some(ShaderUniformValue::Color(_))
            );
            let kind = match kind {
                UniformKind::Vector(n @ (3 | 4))
                    if notes.color || statement.layout_color || (n == 4 && stored_colour) =>
                {
                    UniformKind::Color { arity: n }
                }
                other => other,
            };
            out.push(ShaderUniformSpec {
                name: name.to_owned(),
                kind,
                label: notes.label.clone(),
                range: if kind == UniformKind::Float {
                    notes.range()
                } else {
                    None
                },
                default: notes.default.clone(),
            });
        }
    }
    out
}

/// Whether `sksl` declares a `uniform float2 size;` (or `half2`): the
/// layout scene binds the node size only into an existing `size` entry.
pub fn declares_size(sksl: &str) -> bool {
    uniform_statements(sksl)
        .iter()
        .any(|s| matches!(s.ty, "float2" | "half2") && s.names.contains(&"size"))
}

/// Rows for a body with no source (the loader-expanded `turbulence`
/// preset): one per stored value, its kind read off the value.
pub fn stored_value_specs(values: &BTreeMap<String, ShaderUniformValue>) -> Vec<ShaderUniformSpec> {
    values
        .iter()
        .filter(|(name, _)| !CANVAS_BOUND_UNIFORMS.contains(&name.as_str()))
        .filter_map(|(name, value)| {
            let kind = match value {
                ShaderUniformValue::Float(_) => UniformKind::Float,
                ShaderUniformValue::Vec(v) if (2..=4).contains(&v.len()) => {
                    UniformKind::Vector(v.len())
                }
                ShaderUniformValue::Vec(_) => return None,
                ShaderUniformValue::Color(_) => UniformKind::Color { arity: 4 },
            };
            Some(ShaderUniformSpec {
                name: name.clone(),
                kind,
                label: None,
                range: None,
                default: None,
            })
        })
        .collect()
}

/// SkSL type name → control kind; `None` for what the panel cannot edit
/// (children, ints — the uniform block skips them — matrices, bools).
fn kind_of(ty: &str) -> Option<UniformKind> {
    Some(match ty {
        "float" | "half" => UniformKind::Float,
        "float2" | "half2" | "vec2" => UniformKind::Vector(2),
        "float3" | "half3" | "vec3" => UniformKind::Vector(3),
        "float4" | "half4" | "vec4" => UniformKind::Vector(4),
        _ => return None,
    })
}

/// The parsed doc comment in front of a uniform.
#[derive(Default)]
struct Notes {
    label: Option<String>,
    min: Option<f32>,
    max: Option<f32>,
    color: bool,
    canvas_bound: bool,
    default: Option<String>,
}

impl Notes {
    fn parse(comment: &str) -> Self {
        let mut notes = Self::default();
        for line in comment.lines() {
            let line = line
                .trim()
                .trim_start_matches("/**")
                .trim_start_matches("/*")
                .trim_start_matches("//")
                .trim_end_matches("*/")
                .trim()
                .trim_start_matches('*')
                .trim();
            let mut rest = line;
            while let Some(at) = rest.find('@') {
                let tag = &rest[at + 1..];
                let word_end = tag
                    .find(|c: char| !c.is_ascii_alphanumeric())
                    .unwrap_or(tag.len());
                let (word, after) = tag.split_at(word_end);
                let value_end = after.find('@').unwrap_or(after.len());
                let value = after[..value_end].trim();
                match word {
                    "label" if !value.is_empty() => notes.label = Some(value.to_owned()),
                    "range" => {
                        let mut parts = value
                            .split(|c: char| c == ',' || c.is_whitespace())
                            .filter(|part| !part.is_empty())
                            .filter_map(|part| part.parse::<f32>().ok());
                        notes.min = parts.next().or(notes.min);
                        notes.max = parts.next().or(notes.max);
                    }
                    "min" => notes.min = value.parse().ok().or(notes.min),
                    "max" => notes.max = value.parse().ok().or(notes.max),
                    "color" | "colour" => notes.color = true,
                    "time" | "mouse" | "resolution" => notes.canvas_bound = true,
                    "default" if !value.is_empty() => notes.default = Some(value.to_owned()),
                    _ => {}
                }
                rest = &after[value_end..];
            }
        }
        notes
    }

    fn range(&self) -> Option<(f32, f32)> {
        let (a, b) = (self.min?, self.max?);
        (a.is_finite() && b.is_finite() && a != b).then(|| (a.min(b), a.max(b)))
    }
}

/// One top-level `uniform` statement.
struct UniformStatement<'a> {
    /// The comment right in front of it (empty when none).
    comment: &'a str,
    layout_color: bool,
    ty: &'a str,
    names: Vec<&'a str>,
}

/// Top-level `[layout(…)] uniform <type> <name>[, <name>…];` statements.
/// Comments are skipped except as the doc comment of the next statement;
/// anything inside braces is a function body or struct, never a uniform.
fn uniform_statements(sksl: &str) -> Vec<UniformStatement<'_>> {
    let bytes = sksl.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    // Comment run since the last code token: (start, end).
    let mut comment: Option<(usize, usize)> = None;
    while i < bytes.len() {
        let rest = &sksl[i..];
        if rest.starts_with("//") {
            let end = rest.find('\n').map_or(sksl.len(), |n| i + n);
            comment = Some((comment.map_or(i, |(start, _)| start), end));
            i = end;
            continue;
        }
        if let Some(body) = rest.strip_prefix("/*") {
            let end = body.find("*/").map_or(sksl.len(), |n| i + 2 + n + 2);
            // A block comment starts a new doc comment.
            comment = Some((i, end));
            i = end;
            continue;
        }
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            // A blank line between a `//` run and the next one keeps the
            // run; only code ends it.
            i += 1;
            continue;
        }
        match c {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && (starts_word(rest, "uniform") || starts_word(rest, "layout")) {
            let end = rest.find([';', '{', '}']).map_or(sksl.len(), |n| i + n);
            if let Some(statement) = parse_statement(&sksl[i..end]) {
                out.push(UniformStatement {
                    comment: comment.map_or("", |(start, stop)| &sksl[start..stop]),
                    ..statement
                });
            }
            comment = None;
            i = end.max(i + 1);
            continue;
        }
        comment = None;
        // Skip the rest of this identifier / number so `myuniform` is not
        // read as `uniform`.
        if c.is_ascii_alphanumeric() || c == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
        } else {
            // Step a whole char: `i` must stay on a char boundary.
            i += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn starts_word(text: &str, word: &str) -> bool {
    text.starts_with(word)
        && !text[word.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `[layout(…)] uniform <type> a, b` (no `;`) → its parts.
fn parse_statement(text: &str) -> Option<UniformStatement<'_>> {
    let mut rest = text.trim();
    let mut layout_color = false;
    if starts_word(rest, "layout") {
        let open = rest.find('(')?;
        let close = rest[open..].find(')')? + open;
        layout_color = rest[open + 1..close]
            .split(',')
            .any(|part| part.trim() == "color");
        rest = rest[close + 1..].trim_start();
    }
    rest = rest.strip_prefix("uniform")?;
    let mut words = rest
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .filter(|word| !matches!(*word, "lowp" | "mediump" | "highp"));
    let ty = words.next()?;
    let names: Vec<&str> = words.collect();
    let valid = !names.is_empty()
        && names.iter().all(|name| {
            name.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    valid.then_some(UniformStatement {
        comment: "",
        layout_color,
        ty,
        names,
    })
}

#[cfg(test)]
#[path = "shader_uniform_specs_tests.rs"]
mod tests;
