//! Uniform bindings of the GLSL → SkSL translation ([`super::glsl`]):
//! how each declared uniform is declared in SkSL, how its uses are
//! rewritten, and the prelude every translated program starts with.

use std::collections::{BTreeSet, HashMap};

use serde_json::{Map, Value};

use super::glsl::{Slot, Stage, Texture, UniformDecl, UserUniform};
use super::glsl_error::GlslError;

/// Names the translation itself declares or SkSL reserves; a GLSL
/// identifier spelled like one is renamed with a `pen_` prefix.
pub(super) const RESERVED: &[&str] = &[
    "size",
    "time",
    "mouse",
    "object",
    "background",
    "shader",
    "half",
    "half2",
    "half3",
    "half4",
    "float2",
    "float3",
    "float4",
    "int2",
    "int3",
    "int4",
    "bool2",
    "bool3",
    "bool4",
    "float2x2",
    "float3x3",
    "float4x4",
    "half2x2",
    "half3x3",
    "half4x4",
    "inline",
    "noinline",
    "eval",
];

/// How a GLSL identifier is rewritten.
#[derive(Clone, Debug)]
pub(super) enum Use {
    /// Replaced by this expression.
    Expr(String),
    /// A sampler: valid only as `texture2D` / `textureSize` argument.
    Sampler { child: String, texture: Texture },
}

#[derive(Default)]
pub(super) struct Bindings {
    pub(super) users: Vec<UserUniform>,
    pub(super) uses: HashMap<String, Use>,
    /// Statement start → the SkSL declarations replacing it.
    pub(super) declarations: HashMap<usize, String>,
    /// Doc comments (token indices) of boolean uniforms, which the emitter
    /// tags `@bool`.
    pub(super) tagged_docs: BTreeSet<usize>,
    /// `uniform shader` children declared so far.
    children: BTreeSet<String>,
    time: bool,
    mouse: bool,
}

impl Bindings {
    pub(super) fn declare(
        &mut self,
        decl: &UniformDecl,
        stage: Stage,
        values: &Map<String, Value>,
    ) -> Result<(), GlslError> {
        let mut declared = Vec::new();
        for name in &decl.names {
            let error = |reason| GlslError::Uniform {
                name: name.clone(),
                reason,
            };
            let texture_error = |reason| GlslError::Texture {
                name: name.clone(),
                reason,
            };
            let notes = &decl.notes;
            let renamed = rename(name);
            let (declaration, expr, slot) = if notes.resolution {
                match decl.ty.as_str() {
                    "vec2" => (None, "size".to_owned(), None),
                    "vec3" => (None, "float3(size, 1.0)".to_owned(), None),
                    _ => return Err(error("@resolution must be a vec2 or vec3")),
                }
            } else if notes.time {
                if decl.ty != "float" {
                    return Err(error("@time must be a float"));
                }
                self.time = true;
                (None, "time".to_owned(), None)
            } else if notes.mouse {
                if decl.ty != "vec2" {
                    return Err(error("@mouse must be a vec2"));
                }
                self.mouse = true;
                (None, "float2(mouse.x, size.y - mouse.y)".to_owned(), None)
            } else {
                let value = values.get(name);
                let colour = notes.color || value.is_some_and(Value::is_string);
                match decl.ty.as_str() {
                    "float" => (Some("float"), renamed.clone(), Some(Slot::Float)),
                    "int" => (Some("int"), renamed.clone(), Some(Slot::Int(1))),
                    // SkSL has no boolean uniforms: an int read through
                    // `bool()`, tagged so the panel shows a switch.
                    "bool" => (Some("int"), format!("bool({renamed})"), Some(Slot::Bool(1))),
                    ty @ ("bvec2" | "bvec3" | "bvec4") => {
                        let n = vector_len(ty);
                        (
                            Some(int_vector(n)),
                            format!("bool{n}({renamed})"),
                            Some(Slot::Bool(n)),
                        )
                    }
                    "vec3" if colour => {
                        (Some("float4"), format!("{renamed}.rgb"), Some(Slot::Colour))
                    }
                    "vec4" if colour => (Some("float4"), renamed.clone(), Some(Slot::Colour)),
                    ty @ ("vec2" | "vec3" | "vec4") => {
                        let n = vector_len(ty);
                        (
                            Some(float_vector(n)),
                            renamed.clone(),
                            Some(Slot::Vector(n)),
                        )
                    }
                    ty @ ("ivec2" | "ivec3" | "ivec4") => {
                        let n = vector_len(ty);
                        (Some(int_vector(n)), renamed.clone(), Some(Slot::Int(n)))
                    }
                    "sampler2D" => {
                        if notes.sdf {
                            return Err(texture_error("@sdf textures are not supported"));
                        }
                        let source = value
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .or_else(|| notes.default.clone())
                            .ok_or_else(|| texture_error("texture without a source"))?;
                        let texture = match source.as_str() {
                            "pencil:object" => Texture::Object,
                            "pencil:background" => Texture::Background,
                            _ => Texture::Image(source),
                        };
                        let child = match (&texture, stage) {
                            (Texture::Object, Stage::Effect) => "object".to_owned(),
                            (Texture::Background, Stage::Effect) => "background".to_owned(),
                            (Texture::Image(_), Stage::Fill) => renamed.clone(),
                            (Texture::Image(_), Stage::Effect) => {
                                return Err(texture_error("image textures in a shader effect"))
                            }
                            (_, Stage::Fill) => {
                                return Err(texture_error("pencil:object texture in a shader fill"))
                            }
                        };
                        self.uses.insert(
                            name.clone(),
                            Use::Sampler {
                                child: child.clone(),
                                texture: texture.clone(),
                            },
                        );
                        self.users.push(UserUniform {
                            glsl_name: name.clone(),
                            sksl_name: child.clone(),
                            slot: Slot::Texture(texture),
                            default: notes.default.clone(),
                        });
                        // Two samplers may read the same node / backdrop.
                        let declaration = format!("uniform shader {child};");
                        if self.children.insert(child) {
                            declared.push(declaration);
                        }
                        continue;
                    }
                    _ => return Err(error("unsupported uniform type")),
                }
            };
            self.uses.insert(name.clone(), Use::Expr(expr));
            if let (Some(ty), Some(slot)) = (declaration, slot) {
                if matches!(slot, Slot::Bool(1)) {
                    // Tag it for the panel: in the author's doc comment
                    // when it is the first one that comment describes,
                    // else in a comment of its own.
                    match decl.doc.filter(|_| declared.is_empty()) {
                        Some(doc) => {
                            self.tagged_docs.insert(doc);
                        }
                        None => declared.push("/** @bool */".to_owned()),
                    }
                }
                declared.push(format!("uniform {ty} {renamed};"));
                self.users.push(UserUniform {
                    glsl_name: name.clone(),
                    sksl_name: renamed,
                    slot,
                    default: notes.default.clone(),
                });
            }
        }
        self.declarations.insert(decl.range.0, declared.join(" "));
        Ok(())
    }

    /// SkSL effects sample one texture: the node or its backdrop.
    pub(super) fn check_textures(&self, stage: Stage) -> Result<(), GlslError> {
        if stage == Stage::Fill {
            return Ok(());
        }
        let children: BTreeSet<&str> = self
            .users
            .iter()
            .filter(|user| matches!(user.slot, Slot::Texture(_)))
            .map(|user| user.sksl_name.as_str())
            .collect();
        if children.len() > 1 {
            return Err(GlslError::Texture {
                name: children.into_iter().collect::<Vec<_>>().join(", "),
                reason: "a shader effect samples one texture (object or background)",
            });
        }
        Ok(())
    }

    /// Declarations every translated program starts with.
    pub(super) fn prelude(&self) -> String {
        let mut out = String::from(
            "// Translated from pen.dev GLSL by the .pen import.\nuniform float2 size;\n",
        );
        if self.time {
            out.push_str("uniform float time;\n");
        }
        if self.mouse {
            out.push_str("uniform float2 mouse;\n");
        }
        out.push_str("float4 pen_FragCoord;\nfloat4 pen_FragColor;\n");
        if self.uses.values().any(|u| matches!(u, Use::Sampler { .. })) {
            out.push_str(
                "float2 pen_texel(float2 uv) { return float2(uv.x * size.x, (1.0 - uv.y) * size.y); }\n",
            );
        }
        out
    }
}

pub(super) fn rename(name: &str) -> String {
    if RESERVED.contains(&name) || name.starts_with("sk_") {
        format!("pen_{name}")
    } else {
        name.to_owned()
    }
}

fn vector_len(ty: &str) -> usize {
    ty.chars()
        .last()
        .and_then(|c| c.to_digit(10))
        .map_or(4, |n| n as usize)
}

fn int_vector(n: usize) -> &'static str {
    match n {
        2 => "int2",
        3 => "int3",
        _ => "int4",
    }
}

fn float_vector(n: usize) -> &'static str {
    match n {
        2 => "float2",
        3 => "float3",
        _ => "float4",
    }
}
