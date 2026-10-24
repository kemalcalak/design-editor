//! The device-space wrapper around an effect program (#6).
//!
//! Skia runs a runtime-shader image filter in the layer's parameter space
//! and only accepts a translate there, so a layer opened under a scaled
//! matrix is rasterized at that space's resolution and upscaled. The native
//! host therefore opens effect layers in device pixels and wraps the
//! author's program: its `main` becomes `de_user_main`, called with the
//! fragment mapped back to node-local document pixels, and every
//! `<child>.eval(c)` samples the device-space layer at the mapped point.

/// Uniform carrying the node-local document → device matrix.
pub(super) const DOC_TO_DEVICE: &str = "de_doc_to_device";
/// Uniform carrying the device → node-local document matrix.
pub(super) const DEVICE_TO_DOC: &str = "de_device_to_doc";

/// `sksl` with `main` renamed and `child.eval(` routed through a mapping
/// helper, plus the device-space `main` that calls it.
pub(super) fn wrap_effect_source(sksl: &str, child: &str) -> String {
    let sampler = format!("de_sample_{child}");
    let body = rewrite_calls(
        &rewrite_calls(sksl, "main", "de_user_main"),
        &format!("{child}.eval"),
        &sampler,
    );
    format!(
        "uniform float3x3 {DOC_TO_DEVICE};\nuniform float3x3 {DEVICE_TO_DOC};\n\
         half4 {sampler}(float2 c);\n{body}\n\
         half4 {sampler}(float2 c) {{ return {child}.eval(({DOC_TO_DEVICE} * float3(c, 1.0)).xy); }}\n\
         half4 main(float2 q) {{ return de_user_main(({DEVICE_TO_DOC} * float3(q, 1.0)).xy); }}\n"
    )
}

/// Replace every call `name(` (an identifier boundary before, optional
/// whitespace before the parenthesis; `name` may contain one `.` with
/// optional whitespace around it) with `replacement(`.
fn rewrite_calls(source: &str, name: &str, replacement: &str) -> String {
    let parts: Vec<&str> = name.split('.').collect();
    let mut out = String::with_capacity(source.len() + 64);
    let bytes = source.as_bytes();
    let mut at = 0;
    while at < source.len() {
        if let Some(end) = match_call(source, at, &parts) {
            let boundary = at == 0 || !is_ident_byte(bytes[at - 1]);
            if boundary {
                out.push_str(replacement);
                out.push('(');
                at = end;
                continue;
            }
        }
        let ch = source[at..].chars().next().expect("in bounds");
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

/// End offset (just past `(`) when `parts` joined by `.` and then `(`
/// start at `at`, else `None`.
fn match_call(source: &str, at: usize, parts: &[&str]) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut pos = at;
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            pos = skip_space(bytes, pos);
            if bytes.get(pos) != Some(&b'.') {
                return None;
            }
            pos = skip_space(bytes, pos + 1);
        }
        if !source[pos..].starts_with(part) {
            return None;
        }
        pos += part.len();
        if bytes.get(pos).copied().is_some_and(is_ident_byte) {
            return None;
        }
    }
    pos = skip_space(bytes, pos);
    (bytes.get(pos) == Some(&b'(')).then_some(pos + 1)
}

fn skip_space(bytes: &[u8], mut pos: usize) -> usize {
    while bytes.get(pos).is_some_and(|b| b.is_ascii_whitespace()) {
        pos += 1;
    }
    pos
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// `matrix` as the column-major nine floats an SkSL `float3x3` binds.
pub(super) fn float3x3(matrix: &skia_safe::Matrix) -> [f32; 9] {
    use skia_safe::matrix::Member as M;
    [
        matrix[M::ScaleX],
        matrix[M::SkewY],
        matrix[M::Persp0],
        matrix[M::SkewX],
        matrix[M::ScaleY],
        matrix[M::Persp1],
        matrix[M::TransX],
        matrix[M::TransY],
        matrix[M::Persp2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_and_child_calls_are_rewritten_at_identifier_boundaries() {
        let source = "uniform shader object; half4 domain(float2 p); half4 main (float2 p) { return object . eval(p) + myobject.eval(p); }";
        let wrapped = wrap_effect_source(source, "object");
        assert!(wrapped.contains("half4 de_user_main(float2 p)"));
        assert!(wrapped.contains("half4 domain(float2 p)"), "{wrapped}");
        assert!(
            wrapped.contains("return de_sample_object(p) + myobject.eval(p)"),
            "{wrapped}"
        );
        assert!(wrapped.contains("return object.eval((de_doc_to_device"));
        assert!(wrapped.contains("half4 main(float2 q)"));
    }

    #[test]
    fn float3x3_is_column_major() {
        let matrix = skia_safe::Matrix::new_all(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.0, 0.0, 1.0);
        assert_eq!(
            float3x3(&matrix),
            [1.0, 4.0, 0.0, 2.0, 5.0, 0.0, 3.0, 6.0, 1.0]
        );
    }
}
