//! The call rewrites of the GLSL to SkSL `Emitter`: `texture2D` and
//! friends, `textureSize`, `gl_FragData`, `gl_DepthRange`, and what `main`
//! returns.

use super::super::glsl_bindings::Use;
use super::super::glsl_error::GlslError;
use super::super::glsl_lex::{next_significant, text};
use super::{matching, Emitter, SamplerCall, Texture};

impl Emitter<'_> {
    /// The sampler named by the first argument of the call at `i`, the
    /// argument ranges and the index after the call's `)`.
    pub(super) fn call_args(&self, i: usize) -> Result<SamplerCall<'_>, GlslError> {
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

    /// What `main` returns: the colour, or transparent once a helper
    /// function discarded the fragment.
    pub(super) fn main_return(&self) -> String {
        if self.program.helper_discards {
            "return pen_Discard ? half4(0.0) : half4(pen_FragColor)".to_owned()
        } else {
            "return half4(pen_FragColor)".to_owned()
        }
    }

    /// `gl_FragData[0]` → `pen_FragColor` (WebGL 1.0 has one draw buffer).
    pub(super) fn frag_data(&mut self, i: usize) -> Result<usize, GlslError> {
        let words: Vec<usize> = (i + 1..self.tokens.len())
            .filter(|&j| !self.tokens[j].is_trivia())
            .take(3)
            .collect();
        let zero_index = words.len() == 3
            && self.tokens[words[0]].is_punct('[')
            && self.tokens[words[1]].text == "0"
            && self.tokens[words[2]].is_punct(']');
        if !zero_index {
            return Err(GlslError::Unsupported(
                "gl_FragData other than gl_FragData[0]",
            ));
        }
        self.sksl.push_str("pen_FragColor");
        Ok(words[2] + 1)
    }

    /// `gl_DepthRange.near | far | diff` → `0.0 | 1.0 | 1.0`.
    pub(super) fn depth_range(&mut self, i: usize) -> Result<usize, GlslError> {
        let words: Vec<usize> = (i + 1..self.tokens.len())
            .filter(|&j| !self.tokens[j].is_trivia())
            .take(2)
            .collect();
        let value = (words.len() == 2 && self.tokens[words[0]].is_punct('.'))
            .then(|| match self.tokens[words[1]].text.as_str() {
                "near" => Some("0.0"),
                "far" | "diff" => Some("1.0"),
                _ => None,
            })
            .flatten()
            .ok_or(GlslError::Unsupported("gl_DepthRange without a field"))?;
        self.sksl.push_str(value);
        Ok(words[1] + 1)
    }

    /// `texture2D(tex, uv[, bias])` → `tex.eval(pen_texel(uv))`;
    /// `texture2DProj` divides by the last coordinate first.
    pub(super) fn texture_call(&mut self, i: usize, projective: bool) -> Result<usize, GlslError> {
        let SamplerCall {
            sampler,
            args,
            after,
        } = self.call_args(i)?;
        let Use::Sampler { child, texture } = sampler else {
            unreachable!("call_args returns samplers only");
        };
        let (child, sdf) = (child.clone(), *texture == Texture::Sdf);
        let Some(&(uv_lo, uv_hi)) = args.get(1) else {
            return Err(GlslError::Unsupported("texture lookup without coordinates"));
        };
        let uv_lo = next_significant(self.tokens, uv_lo).map_or(uv_hi, |j| j.min(uv_hi));
        if sdf {
            self.sksl.push_str("pen_sdf(");
        }
        self.sksl.push_str(&format!("{child}.eval(pen_texel("));
        if projective {
            self.sksl.push_str("pen_project(");
        }
        self.emit(uv_lo, uv_hi)?;
        self.sksl.push_str(if projective { ")))" } else { "))" });
        if sdf {
            self.sksl.push(')');
        }
        Ok(after)
    }

    /// `textureSize(tex, lod)`: the node size for the node, its backdrop
    /// and its SDF; an image's pixel size from a uniform the import binds.
    pub(super) fn texture_size(&mut self, i: usize) -> Result<usize, GlslError> {
        let SamplerCall { sampler, after, .. } = self.call_args(i)?;
        let Use::Sampler { child, texture } = sampler else {
            unreachable!("call_args returns samplers only");
        };
        if matches!(texture, Texture::Image(_)) {
            let child = child.clone();
            self.sksl
                .push_str(&format!("int2({})", super::image_size_uniform(&child)));
            self.sized.push(child);
        } else {
            self.sksl.push_str("int2(size)");
        }
        Ok(after)
    }
}
