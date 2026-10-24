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
//!   directives are dropped;
//! - GLSL type names become SkSL ones (`vec3` → `float3`, `mat2` →
//!   `float2x2`);
//! - `main` takes the fragment coordinate and returns the colour, with
//!   `gl_FragCoord` flipped to GL's origin (`size.y - y`), as globals so
//!   helper functions can read them too;
//! - `@resolution` reads `size`, `@time` reads `time`, `@mouse` reads the
//!   canvas pointer flipped like `gl_FragCoord`;
//! - a colour `vec3` is bound as a `float4` colour uniform and read
//!   through `.rgb`, so the document keeps a hex colour for it;
//! - `int` / `ivec` uniforms stay integers; a `bool` / `bvec` one (SkSL has
//!   no boolean uniforms) is bound as an `int` read through `bool()`;
//! - `texture2D(tex, uv)` becomes `tex.eval(...)` in node pixels;
//! - `discard` returns transparent (from a helper function through a
//!   flag `main` checks), and the other WebGL 1.0 built-ins read their
//!   fragment-shader values (`gl_FragData[0]`, `gl_FrontFacing`,
//!   `gl_PointCoord`, `gl_DepthRange`, the `gl_Max*` minimums);
//! - `texture2DProj` divides by the last coordinate first.
//!
//! The preprocessor runs first ([`super::glsl_preprocess`]). Anything else
//! (derivatives, …) is left to the SkSL compiler, which the caller runs to
//! decide whether to keep the paint.

use std::collections::HashMap;

use serde_json::{Map, Value};

use super::glsl_bindings::{rename, Bindings, Use};
use super::glsl_error::GlslError;
use super::glsl_lex::{lex, next_significant, prev_significant, Kind, Token};
use super::glsl_preprocess::preprocess;

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
    /// `pencil:object`: the node's own rendering (in a fill, the fills
    /// under it).
    Object,
    /// `pencil:background`: what is painted behind the node.
    Background,
    /// `@sdf`: the signed distance field of the node's shape (in an
    /// effect, of the rendered content).
    Sdf,
    /// An image file or URL.
    Image(String),
}

/// How a user uniform is bound in the SkSL program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// One float.
    Float,
    /// A float vector of this length.
    Vector(usize),
    /// An `int` (1) or `ivecN` (N) uniform, stored as whole numbers.
    Int(usize),
    /// A `bool` (1) or `bvecN` (N) uniform, stored as 0 / 1.
    Bool(usize),
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
    /// Image children whose size the program reads (`textureSize`); the
    /// import binds [`image_size_uniform`] for each.
    pub(crate) sized_images: Vec<String>,
}

/// The uniform holding image child `child`'s pixel size.
pub(crate) fn image_size_uniform(child: &str) -> String {
    format!("pen_size_{child}")
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
        bindings.declare(decl, values)?;
    }
    bindings.finish_children(stage);
    let mut out = Emitter {
        tokens: &tokens,
        program: &program,
        bindings: &bindings,
        sksl: String::new(),
        sized: Vec::new(),
    };
    out.sksl.push_str(&bindings.prelude());
    let prelude_end = out.sksl.len();
    if program.helper_discards {
        out.sksl.push_str("bool pen_Discard;\n");
    }
    if tokens
        .iter()
        .any(|token| token.is_ident("texture2DProj") || token.is_ident("texture2DProjLod"))
    {
        out.sksl.push_str(
            "float2 pen_project(float3 p) { return p.xy / p.z; }\nfloat2 pen_project(float4 p) { return p.xy / p.w; }\n",
        );
    }
    out.emit(0, tokens.len())?;
    // `textureSize` of an image reads a hidden uniform the import fills
    // with the image's size.
    let mut sized = out.sized;
    sized.sort();
    sized.dedup();
    let declarations: String = sized
        .iter()
        .map(|child| {
            format!(
                "/** @internal */ uniform float2 {};\n",
                image_size_uniform(child)
            )
        })
        .collect();
    out.sksl.insert_str(prelude_end, &declarations);
    Ok(Translation {
        sksl: out.sksl,
        uniforms: bindings.users,
        sized_images: sized,
    })
}

/// One `uniform` statement.
pub(super) struct UniformDecl {
    /// Token range of the statement, `;` included.
    pub(super) range: (usize, usize),
    pub(super) ty: String,
    pub(super) names: Vec<String>,
    pub(super) notes: Annotations,
    /// Token index of the doc comment in front of the statement.
    pub(super) doc: Option<usize>,
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
    /// Every top-level function body: `{`, `}` and the return type.
    functions: Vec<Function>,
    /// Whether a function other than `main` discards.
    helper_discards: bool,
}

/// A top-level function body.
struct Function {
    open: usize,
    close: usize,
    returns: String,
}

fn scan(tokens: &[Token]) -> Result<Program, GlslError> {
    let mut uniforms = Vec::new();
    let mut precision = Vec::new();
    let mut main = None;
    let mut functions = Vec::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        if token.is_punct('{') {
            if depth == 0 {
                if let Some(returns) = function_return_type(tokens, i) {
                    let close = matching(tokens, i, '{', '}')?;
                    functions.push(Function {
                        open: i,
                        close,
                        returns,
                    });
                }
            }
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
    let helper_discards = functions.iter().any(|function| {
        function.open != open
            && tokens[function.open..function.close]
                .iter()
                .any(|token| token.is_ident("discard"))
    });
    Ok(Program {
        uniforms,
        skips,
        main_header: (header_start, open),
        main_body: (open, close),
        functions,
        helper_discards,
    })
}

/// The return type of the function whose body opens at `brace`
/// (`float f(vec2 p) {` → `float`), or `None` when the block is not a
/// function body.
fn function_return_type(tokens: &[Token], brace: usize) -> Option<String> {
    let close = prev_significant(tokens, brace).filter(|&j| tokens[j].is_punct(')'))?;
    let mut depth = 0i32;
    let open = (0..=close).rev().find(|&j| {
        if tokens[j].is_punct(')') {
            depth += 1;
        } else if tokens[j].is_punct('(') {
            depth -= 1;
        }
        depth == 0
    })?;
    let name = prev_significant(tokens, open).filter(|&j| tokens[j].kind == Kind::Ident)?;
    let mut ty = prev_significant(tokens, name).filter(|&j| tokens[j].kind == Kind::Ident)?;
    while PRECISION_QUALIFIERS.contains(&tokens[ty].text.as_str()) {
        ty = prev_significant(tokens, ty).filter(|&j| tokens[j].kind == Kind::Ident)?;
    }
    Some(tokens[ty].text.clone())
}

/// A statement returning a zero value of GLSL type `ty`, for a `discard`
/// inside a function of that return type.
fn zero_return(ty: &str) -> Result<String, GlslError> {
    let value = match ty {
        "void" => return Ok("return".to_owned()),
        "float" => "0.0".to_owned(),
        "int" => "0".to_owned(),
        "bool" => "false".to_owned(),
        "vec2" | "vec3" | "vec4" | "mat2" | "mat3" | "mat4" => {
            format!("{}(0.0)", sksl_type(ty).expect("vector type"))
        }
        "ivec2" | "ivec3" | "ivec4" => format!("{}(0)", sksl_type(ty).expect("vector type")),
        "bvec2" | "bvec3" | "bvec4" => format!("{}(false)", sksl_type(ty).expect("vector type")),
        _ => {
            return Err(GlslError::Unsupported(
                "discard in a function returning a struct",
            ))
        }
    };
    Ok(format!("return {value}"))
}

/// The value of a WebGL 1.0 built-in: the `gl_Max*` constants (the
/// minimums the spec guarantees, which is what a portable shader can rely
/// on) and the fragment inputs a fill or effect has.
fn builtin_value(name: &str) -> Option<&'static str> {
    Some(match name {
        "gl_MaxVertexAttribs" => "8",
        "gl_MaxVertexUniformVectors" => "128",
        "gl_MaxVaryingVectors" => "8",
        "gl_MaxVertexTextureImageUnits" => "0",
        "gl_MaxCombinedTextureImageUnits" => "8",
        "gl_MaxTextureImageUnits" => "8",
        "gl_MaxFragmentUniformVectors" => "16",
        "gl_MaxDrawBuffers" => "1",
        "gl_FrontFacing" => "true",
        "gl_PointCoord" => "(pen_FragCoord.xy / size)",
        _ => return None,
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
    let doc = (0..start)
        .rev()
        .find(|&j| tokens[j].kind != Kind::Space)
        .filter(|&j| tokens[j].kind == Kind::Comment && tokens[j].text.starts_with("/**"));
    let notes = doc
        .map(|j| Annotations::parse(&tokens[j].text))
        .unwrap_or_default();
    Ok(UniformDecl {
        range: (start, end),
        ty: ty.text.clone(),
        names,
        notes,
        doc,
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
    /// Image children `textureSize` reads.
    sized: Vec<String>,
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
                if self.program.helper_discards {
                    self.sksl.push_str(" pen_Discard = false;");
                }
                i = main_open + 1;
                continue;
            }
            if i == main_close {
                self.sksl
                    .push_str(&format!("\n    {};\n}}", self.main_return()));
                i += 1;
                continue;
            }
            let token = &self.tokens[i];
            let in_main = i > main_open && i < main_close;
            if self.bindings.tagged_docs.contains(&i) {
                // The doc comment of a boolean uniform gains the tag the
                // panel reads (`shader_uniform_specs`).
                let body = token.text.trim_end_matches("*/").trim_end();
                self.sksl.push_str(&format!("{body} @bool */"));
                i += 1;
                continue;
            }
            if token.kind != Kind::Ident {
                self.sksl.push_str(&token.text);
                i += 1;
                continue;
            }
            let after_dot =
                prev_significant(self.tokens, i).is_some_and(|j| self.tokens[j].is_punct('.'));
            if after_dot {
                // Swizzles and fields; a field spelled like a reserved name
                // was renamed where its struct declares it.
                self.sksl.push_str(&rename(&token.text));
                i += 1;
                continue;
            }
            match token.text.as_str() {
                q if PRECISION_QUALIFIERS.contains(&q) => {}
                "return" if in_main => {
                    let next = next_significant(self.tokens, i + 1);
                    if next.is_some_and(|j| self.tokens[j].is_punct(';')) {
                        let main_return = self.main_return();
                        self.sksl.push_str(&main_return);
                    } else {
                        self.sksl.push_str("return");
                    }
                }
                "gl_FragCoord" => self.sksl.push_str("pen_FragCoord"),
                "gl_FragColor" => self.sksl.push_str("pen_FragColor"),
                "discard" if in_main => self.sksl.push_str("return half4(0.0)"),
                "discard" => {
                    let returns = self
                        .program
                        .functions
                        .iter()
                        .find(|function| function.open < i && i < function.close)
                        .map(|function| function.returns.as_str())
                        .ok_or(GlslError::Unsupported("discard outside a function"))?;
                    let zero = zero_return(returns)?;
                    self.sksl
                        .push_str(&format!("{{ pen_Discard = true; {zero}; }}"));
                    // The block replaces the statement: drop its `;` so an
                    // `else` that follows still pairs with the `if`.
                    if let Some(semicolon) = next_significant(self.tokens, i + 1)
                        .filter(|&j| self.tokens[j].is_punct(';'))
                    {
                        i = semicolon + 1;
                        continue;
                    }
                }
                "gl_FragData" => {
                    i = self.frag_data(i)?;
                    continue;
                }
                "gl_DepthRange" => {
                    i = self.depth_range(i)?;
                    continue;
                }
                name if builtin_value(name).is_some() => {
                    self.sksl.push_str(builtin_value(name).unwrap_or_default());
                }
                "sampler2D" | "samplerCube" => {
                    return Err(GlslError::Unsupported("textures passed to functions"))
                }
                "texture2D" | "texture2DLod" | "texture" => {
                    i = self.texture_call(i, false)?;
                    continue;
                }
                "texture2DProj" | "texture2DProjLod" => {
                    i = self.texture_call(i, true)?;
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
                        // A bare name or `name.rgb` needs no parentheses;
                        // the generated calls and arithmetic all contain a
                        // space.
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
}

// The built-in and texture call rewrites of `Emitter` (800-line cap).
#[path = "glsl_calls.rs"]
mod calls;

#[cfg(test)]
#[path = "glsl_tests.rs"]
mod tests;
