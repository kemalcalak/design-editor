//! Why a pen.dev GLSL shader could not become an SkSL shader.

use std::fmt;

use super::glsl::Stage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GlslError {
    /// No `void main()` at the top level.
    NoMain,
    /// A preprocessor directive other than `#version`, `#extension`,
    /// `#pragma`, `#ifdef GL_ES` / `#endif` or an object-like `#define`.
    Directive(String),
    /// A GLSL feature SkSL runtime effects cannot express.
    Unsupported(&'static str),
    /// A uniform declaration the translation cannot bind.
    Uniform { name: String, reason: &'static str },
    /// A `sampler2D` the target paint cannot bind.
    Texture { name: String, reason: &'static str },
    /// Unbalanced `(` / `{` in the source.
    Unbalanced,
}

impl GlslError {
    /// Short report label for the import summary.
    pub(crate) fn label(&self, stage: Stage) -> &'static str {
        let texture = matches!(self, GlslError::Texture { .. });
        match (stage == Stage::Effect, texture) {
            (false, false) => "shader fill (unsupported GLSL)",
            (false, true) => "shader fill (unsupported texture)",
            (true, false) => "shader effect (unsupported GLSL)",
            (true, true) => "shader effect (unsupported texture)",
        }
    }
}

impl fmt::Display for GlslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlslError::NoMain => write!(f, "no `void main()` entry point"),
            GlslError::Directive(line) => write!(f, "unsupported preprocessor line `{line}`"),
            GlslError::Unsupported(what) => write!(f, "unsupported GLSL: {what}"),
            GlslError::Uniform { name, reason } | GlslError::Texture { name, reason } => {
                write!(f, "uniform `{name}`: {reason}")
            }
            GlslError::Unbalanced => write!(f, "unbalanced brackets"),
        }
    }
}

impl std::error::Error for GlslError {}
