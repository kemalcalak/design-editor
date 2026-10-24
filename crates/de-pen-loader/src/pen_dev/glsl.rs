//! pen.dev GLSL ES 1.0 fragment shader → SkSL runtime effect.
//!
//! pen.dev shader fills and effects run WebGL 1.0 fragment shaders:
//! `void main()` writes `gl_FragColor`, `gl_FragCoord` has GL's bottom-left
//! origin, and doc-comment annotations bind the special uniforms
//! (`/** @resolution */ uniform vec2 u_resolution;`, `@time`, `@mouse`) and
//! describe user ones (`@color`, `@default`, `@range`, `@label`). A
//! `sampler2D` reads an image file or, with the special sources
//! `pencil:object` / `pencil:background`, the node's own rendering or its
//! backdrop.
//!
//! Design Editor runs SkSL (`half4 main(float2 fragCoord)`, top-left origin)
//! and binds `size` (the node box), `time` and `mouse` itself
//! (`canvas_shader_clock`, `canvas_shader_pointer`). The two languages share
//! nearly all syntax, so [`translate`] rewrites the token stream:
//!
//! - `precision` statements, precision qualifiers and the harmless
//!   directives are dropped; object-like `#define`s are substituted;
//! - GLSL type names become SkSL ones (`vec3` → `float3`, `mat2` →
//!   `float2x2`);
//! - `main` takes the fragment coordinate and returns the colour, with
//!   `gl_FragCoord` flipped to GL's origin (`size.y - y`), as globals so
//!   helper functions can read them too;
//! - `@resolution` reads `size`, `@time` reads `time`, `@mouse` reads the
//!   canvas pointer flipped like `gl_FragCoord`;
//! - a colour `vec3` is bound as a `float4` colour uniform and read
//!   through `.rgb`, so the document keeps a hex colour for it;
//! - `int` / `ivec` uniforms are bound as floats and read through `int()`;
//! - `texture2D(tex, uv)` becomes `tex.eval(...)` in node pixels.
//!
//! Anything else (other textures, derivatives, `discard`, …) is left to the
//! SkSL compiler, which the caller runs to decide whether to keep the paint.

use std::collections::HashMap;

use serde_json::{Map, Value};

use super::glsl_bindings::{rename, Bindings, Use};
use super::glsl_error::GlslError;
use super::glsl_lex::{lex, next_significant, prev_significant, text, Kind, Token};

/// Where the shader runs: a fill paints the node, an effect post-processes
/// its rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Fill,
    Effect,
}

/// What a `sampler2D` uniform reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Texture {
    /// `pencil:object`: the node's own rendering (effects only).
    Object,
    /// `pencil:background`: what is painted behind the node (effects only).
    Background,
    /// An image file or URL (fills only).
    Image(String),
}

/// How a user uniform is bound in the SkSL program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// One float (also an `int` uniform).
    Float,
    /// A float vector of this length (also an `ivec`).
    Vector(usize),
    /// A colour (`vec3` / `vec4`), bound as a hex string.
    Colour,
    /// A `uniform shader` child.
    Texture(Texture),
}

/// A user uniform of the translated program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UserUniform {
    /// Name in the GLSL source (and the pen.dev `uniforms` map).
    pub(crate) glsl_name: String,
    /// Name in the SkSL program (and the canonical `uniforms` map).
    pub(crate) sksl_name: String,
    pub(crate) slot: Slot,
    /// The raw `@default` annotation text, if any.
    pub(crate) default: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Translation {
    pub(crate) sksl: String,
    pub(crate) uniforms: Vec<UserUniform>,
}

/// GLSL type name → SkSL type name.
fn sksl_type(glsl: &str) -> Option<&'static str> {
    Some(match glsl {
        "vec2" => "float2",
        "vec3" => "float3",
        "vec4" => "float4",
        "ivec2" => "int2",
        "ivec3" => "int3",
        "ivec4" => "int4",
        "bvec2" => "bool2",
        "bvec3" => "bool3",
        "bvec4" => "bool4",
        "mat2" => "float2x2",
        "mat3" => "float3x3",
        "mat4" => "float4x4",
        _ => return None,
    })
}

const PRECISION_QUALIFIERS: &[&str] = &["lowp", "mediump", "highp"];

/// Translate `glsl` for `stage`. `values` is the pen.dev `uniforms` map
/// with `$variable` references already resolved: it decides whether a
/// vector is a colour and what a texture reads.
pub(crate) fn translate(
    glsl: &str,
    stage: Stage,
    values: &Map<String, Value>,
) -> Result<Translation, GlslError> {
    let tokens = preprocess(lex(glsl))?;
    let program = scan(&tokens)?;
    let mut bindings = Bindings::default();
    for decl in &program.uniforms {
        bindings.declare(decl, stage, values)?;
    }
    bindings.check_textures(stage)?;
    let mut out = Emitter {
        tokens: &tokens,
        program: &program,
        bindings: &bindings,
        sksl: String::new(),
    };
    out.sksl.push_str(&bindings.prelude());
    out.emit(0, tokens.len())?;
    Ok(Translation {
        sksl: out.sksl,
        uniforms: bindings.users,
    })
}

/// Drop the directives SkSL has no use for and substitute object-like
/// `#define`s. Everything else a preprocessor would do fails the shader.
fn preprocess(tokens: Vec<Token>) -> Result<Vec<Token>, GlslError> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut defines: HashMap<String, Vec<Token>> = HashMap::new();
    let mut gl_es_depth = 0usize;
    for token in tokens {
        if token.kind == Kind::Directive {
            let line = token.text.trim();
            let mut words = line.trim_start_matches('#').split_whitespace();
            match (words.next(), words.next()) {
                (Some("version" | "extension" | "pragma"), _) => {}
                (Some("ifdef"), Some("GL_ES")) => gl_es_depth += 1,
                (Some("endif"), None) if gl_es_depth > 0 => gl_es_depth -= 1,
                (Some("define"), Some(_)) => {
                    let (name, body) =
                        define(line).ok_or_else(|| GlslError::Directive(line.to_owned()))?;
                    let body = substitute(lex(body), &defines);
                    defines.insert(name.to_owned(), body);
                }
                _ => return Err(GlslError::Directive(line.to_owned())),
            }
            // Keep the line count, so compile errors point at the line.
            out.push(Token::new(Kind::Space, "\n"));
            continue;
        }
        out.push(token);
    }
    if gl_es_depth > 0 {
        return Err(GlslError::Directive("#ifdef GL_ES".to_owned()));
    }
    Ok(substitute(out, &defines))
}

/// `#define NAME body` → `(NAME, body)`; `None` for a function-like macro.
fn define(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim_start_matches('#').trim_start();
    let rest = rest.strip_prefix("define")?.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let (name, body) = rest.split_at(end);
    if name.is_empty() || body.starts_with('(') || body.contains('\\') {
        return None;
    }
    Some((name, body.trim()))
}

fn substitute(tokens: Vec<Token>, defines: &HashMap<String, Vec<Token>>) -> Vec<Token> {
    if defines.is_empty() {
        return tokens;
    }
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        match defines
            .get(&token.text)
            .filter(|_| token.kind == Kind::Ident)
        {
            Some(body) => {
                out.push(Token::new(Kind::Punct, "("));
                out.extend(body.iter().cloned());
                out.push(Token::new(Kind::Punct, ")"));
            }
            None => out.push(token),
        }
    }
    out
}

/// One `uniform` statement.
pub(super) struct UniformDecl {
    /// Token range of the statement, `;` included.
    pub(super) range: (usize, usize),
    pub(super) ty: String,
    pub(super) names: Vec<String>,
    pub(super) notes: Annotations,
}

/// The parsed doc comment in front of a uniform.
#[derive(Default)]
pub(super) struct Annotations {
    pub(super) resolution: bool,
    pub(super) time: bool,
    pub(super) mouse: bool,
    pub(super) color: bool,
    pub(super) sdf: bool,
    pub(super) default: Option<String>,
}

impl Annotations {
    fn parse(comment: &str) -> Self {
        let mut notes = Self::default();
        let body = comment
            .trim_start_matches("/**")
            .trim_end_matches("*/")
            .lines()
            .map(|line| line.trim().trim_start_matches('*').trim());
        for line in body {
            for (at, _) in line.match_indices('@') {
                let tag = &line[at + 1..];
                let word_end = tag
                    .find(|c: char| !c.is_ascii_alphanumeric())
                    .unwrap_or(tag.len());
                let (word, rest) = tag.split_at(word_end);
                match word {
                    "resolution" => notes.resolution = true,
                    "time" => notes.time = true,
                    "mouse" => notes.mouse = true,
                    "color" => notes.color = true,
                    "sdf" => notes.sdf = true,
                    "default" => {
                        let value = rest.split('@').next().unwrap_or_default().trim();
                        notes.default = (!value.is_empty()).then(|| value.to_owned());
                    }
                    _ => {}
                }
            }
        }
        notes
    }
}

/// The top-level structure the emitter needs.
struct Program {
    uniforms: Vec<UniformDecl>,
    /// Start → end of the `precision` and `uniform` statements the emitter
    /// replaces.
    skips: HashMap<usize, usize>,
    /// `void main(…)` header range (up to, not including, `{`).
    main_header: (usize, usize),
    /// Indices of main's `{` and `}`.
    main_body: (usize, usize),
}

fn scan(tokens: &[Token]) -> Result<Program, GlslError> {
    let mut uniforms = Vec::new();
    let mut precision = Vec::new();
    let mut main = None;
    let mut depth = 0i32;
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        if token.is_punct('{') {
            depth += 1;
        } else if token.is_punct('}') {
            depth -= 1;
            if depth < 0 {
                return Err(GlslError::Unbalanced);
            }
        } else if depth == 0 && token.kind == Kind::Ident {
            match token.text.as_str() {
                "precision" => {
                    let end = statement_end(tokens, i)?;
                    precision.push((i, end));
                    i = end;
                    continue;
                }
                "uniform" => {
                    let end = statement_end(tokens, i)?;
                    uniforms.push(uniform_decl(tokens, i, end)?);
                    i = end;
                    continue;
                }
                "attribute" | "varying" => {
                    return Err(GlslError::Unsupported("vertex inputs"));
                }
                "void" => {
                    if let Some(found) = main_header(tokens, i) {
                        main = Some(found);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    if depth != 0 {
        return Err(GlslError::Unbalanced);
    }
    let ((header_start, open), close) = main
        .map(|(start, open)| matching(tokens, open, '{', '}').map(|close| ((start, open), close)))
        .ok_or(GlslError::NoMain)??;
    let skips = precision
        .into_iter()
        .chain(uniforms.iter().map(|decl: &UniformDecl| decl.range))
        .collect();
    Ok(Program {
        uniforms,
        skips,
        main_header: (header_start, open),
        main_body: (open, close),
    })
}

/// End (exclusive) of the statement starting at `i`: one past its `;`.
fn statement_end(tokens: &[Token], i: usize) -> Result<usize, GlslError> {
    (i..tokens.len())
        .find(|&j| tokens[j].is_punct(';'))
        .map(|j| j + 1)
        .ok_or(GlslError::Unbalanced)
}

fn uniform_decl(tokens: &[Token], start: usize, end: usize) -> Result<UniformDecl, GlslError> {
    let words: Vec<&Token> = tokens[start + 1..end - 1]
        .iter()
        .filter(|token| !token.is_trivia())
        .filter(|token| !PRECISION_QUALIFIERS.contains(&token.text.as_str()))
        .collect();
    let Some((ty, rest)) = words.split_first() else {
        return Err(GlslError::Unsupported("empty uniform declaration"));
    };
    let mut names = Vec::new();
    for (index, word) in rest.iter().enumerate() {
        let expect_name = index % 2 == 0;
        match (expect_name, word.kind) {
            (true, Kind::Ident) => names.push(word.text.clone()),
            (false, _) if word.is_punct(',') => {}
            _ => {
                return Err(GlslError::Uniform {
                    name: rest.first().map_or_else(String::new, |w| w.text.clone()),
                    reason: "arrays and initialisers are not supported",
                })
            }
        }
    }
    if names.is_empty() {
        return Err(GlslError::Unsupported("uniform without a name"));
    }
    // The doc comment right in front of the statement (only spaces between).
    let notes = (0..start)
        .rev()
        .find(|&j| tokens[j].kind != Kind::Space)
        .filter(|&j| tokens[j].kind == Kind::Comment && tokens[j].text.starts_with("/**"))
        .map(|j| Annotations::parse(&tokens[j].text))
        .unwrap_or_default();
    Ok(UniformDecl {
        range: (start, end),
        ty: ty.text.clone(),
        names,
        notes,
    })
}

/// `void main ( [void] ) {` at `i` → `(start, index of '{')`.
fn main_header(tokens: &[Token], i: usize) -> Option<(usize, usize)> {
    let name = next_significant(tokens, i + 1)?;
    if !tokens[name].is_ident("main") {
        return None;
    }
    let open = next_significant(tokens, name + 1)?;
    if !tokens[open].is_punct('(') {
        return None;
    }
    let mut close = next_significant(tokens, open + 1)?;
    if tokens[close].is_ident("void") {
        close = next_significant(tokens, close + 1)?;
    }
    if !tokens[close].is_punct(')') {
        return None;
    }
    let brace = next_significant(tokens, close + 1)?;
    tokens[brace].is_punct('{').then_some((i, brace))
}

/// Index of the bracket closing the one at `open`.
fn matching(tokens: &[Token], open: usize, left: char, right: char) -> Result<usize, GlslError> {
    let mut depth = 0i32;
    for (j, token) in tokens.iter().enumerate().skip(open) {
        if token.is_punct(left) {
            depth += 1;
        } else if token.is_punct(right) {
            depth -= 1;
            if depth == 0 {
                return Ok(j);
            }
        }
    }
    Err(GlslError::Unbalanced)
}

/// A `texture2D` / `textureSize` call: its sampler, the token ranges of
/// its arguments and the index after its `)`.
struct SamplerCall<'a> {
    sampler: &'a Use,
    args: Vec<(usize, usize)>,
    after: usize,
}

struct Emitter<'a> {
    tokens: &'a [Token],
    program: &'a Program,
    bindings: &'a Bindings,
    sksl: String,
}

impl Emitter<'_> {
    /// Write the SkSL for tokens `lo..hi`.
    fn emit(&mut self, lo: usize, hi: usize) -> Result<(), GlslError> {
        let (main_open, main_close) = self.program.main_body;
        let mut i = lo;
        while i < hi {
            if let Some(&end) = self.program.skips.get(&i) {
                if let Some(declared) = self.bindings.declarations.get(&i) {
                    self.sksl.push_str(declared);
                }
                i = end;
                continue;
            }
            if i == self.program.main_header.0 {
                self.sksl.push_str(
                    "half4 main(float2 pen_fragCoord) {\n    pen_FragCoord = float4(pen_fragCoord.x, size.y - pen_fragCoord.y, 0.0, 1.0);\n    pen_FragColor = float4(0.0);",
                );
                i = main_open + 1;
                continue;
            }
            if i == main_close {
                self.sksl.push_str("\n    return half4(pen_FragColor);\n}");
                i += 1;
                continue;
            }
            let token = &self.tokens[i];
            let in_main = i > main_open && i < main_close;
            if token.kind != Kind::Ident {
                self.sksl.push_str(&token.text);
                i += 1;
                continue;
            }
            let after_dot =
                prev_significant(self.tokens, i).is_some_and(|j| self.tokens[j].is_punct('.'));
            if after_dot {
                self.sksl.push_str(&token.text);
                i += 1;
                continue;
            }
            match token.text.as_str() {
                q if PRECISION_QUALIFIERS.contains(&q) => {}
                "return" if in_main => {
                    let next = next_significant(self.tokens, i + 1);
                    if next.is_some_and(|j| self.tokens[j].is_punct(';')) {
                        self.sksl.push_str("return half4(pen_FragColor)");
                    } else {
                        self.sksl.push_str("return");
                    }
                }
                "gl_FragCoord" => self.sksl.push_str("pen_FragCoord"),
                "gl_FragColor" => self.sksl.push_str("pen_FragColor"),
                "discard" => return Err(GlslError::Unsupported("discard")),
                "sampler2D" | "samplerCube" => {
                    return Err(GlslError::Unsupported("textures passed to functions"))
                }
                "texture2D" | "texture2DLod" | "texture" => {
                    i = self.texture_call(i)?;
                    continue;
                }
                "textureSize" => {
                    i = self.texture_size(i)?;
                    continue;
                }
                name if name.starts_with("gl_") => {
                    return Err(GlslError::Unsupported(
                        "GLSL built-in other than gl_FragCoord / gl_FragColor",
                    ))
                }
                name => match self.bindings.uses.get(name) {
                    Some(Use::Expr(expr)) => {
                        let wrap = expr.contains(' ');
                        if wrap {
                            self.sksl.push('(');
                        }
                        self.sksl.push_str(expr);
                        if wrap {
                            self.sksl.push(')');
                        }
                    }
                    Some(Use::Sampler { .. }) => {
                        return Err(GlslError::Unsupported("textures passed to functions"))
                    }
                    None => match sksl_type(name) {
                        Some(ty) => self.sksl.push_str(ty),
                        None => self.sksl.push_str(&rename(name)),
                    },
                },
            }
            i += 1;
        }
        Ok(())
    }

    /// The sampler named by the first argument of the call at `i`, the
    /// argument ranges and the index after the call's `)`.
    fn call_args(&self, i: usize) -> Result<SamplerCall<'_>, GlslError> {
        let open = next_significant(self.tokens, i + 1)
            .filter(|&j| self.tokens[j].is_punct('('))
            .ok_or(GlslError::Unbalanced)?;
        let close = matching(self.tokens, open, '(', ')')?;
        let mut args = Vec::new();
        let mut depth = 0i32;
        let mut start = open + 1;
        for j in open + 1..close {
            let token = &self.tokens[j];
            if token.is_punct('(') || token.is_punct('[') {
                depth += 1;
            } else if token.is_punct(')') || token.is_punct(']') {
                depth -= 1;
            } else if depth == 0 && token.is_punct(',') {
                args.push((start, j));
                start = j + 1;
            }
        }
        args.push((start, close));
        let sampler = text(&self.tokens[args[0].0..args[0].1]);
        let sampler = self
            .bindings
            .uses
            .get(sampler.trim())
            .filter(|u| matches!(u, Use::Sampler { .. }))
            .ok_or(GlslError::Unsupported(
                "texture lookup on a non-uniform sampler",
            ))?;
        Ok(SamplerCall {
            sampler,
            args,
            after: close + 1,
        })
    }

    /// `texture2D(tex, uv[, bias])` → `tex.eval(pen_texel(uv))`.
    fn texture_call(&mut self, i: usize) -> Result<usize, GlslError> {
        let SamplerCall {
            sampler,
            args,
            after,
        } = self.call_args(i)?;
        let Use::Sampler { child, .. } = sampler else {
            unreachable!("call_args returns samplers only");
        };
        let Some(&(uv_lo, uv_hi)) = args.get(1) else {
            return Err(GlslError::Unsupported("texture lookup without coordinates"));
        };
        let uv_lo = next_significant(self.tokens, uv_lo).map_or(uv_hi, |j| j.min(uv_hi));
        self.sksl.push_str(&format!("{child}.eval(pen_texel("));
        self.emit(uv_lo, uv_hi)?;
        self.sksl.push_str("))");
        Ok(after)
    }

    /// `textureSize(tex, lod)` of the node or its backdrop → the node size.
    fn texture_size(&mut self, i: usize) -> Result<usize, GlslError> {
        let SamplerCall { sampler, after, .. } = self.call_args(i)?;
        match sampler {
            Use::Sampler {
                texture: Texture::Object | Texture::Background,
                ..
            } => {
                self.sksl.push_str("int2(size)");
                Ok(after)
            }
            _ => Err(GlslError::Unsupported("textureSize of an image texture")),
        }
    }
}

#[cfg(test)]
#[path = "glsl_tests.rs"]
mod tests;
