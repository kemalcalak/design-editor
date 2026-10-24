use super::*;

#[test]
fn default_effect_is_a_visible_object_pass_with_a_cell_uniform() {
    let PenEffect::Shader(body) = default_shader_effect() else {
        panic!("default is a shader effect");
    };
    assert_eq!(body.visible, None);
    assert!(!samples_background(&body.sksl));
    assert!(shader_children(&body.sksl).contains(&OBJECT_CHILD));
    assert_eq!(
        body.uniforms.as_ref().and_then(|u| u.get("cell")),
        Some(&ShaderUniformValue::Float(DEFAULT_SHADER_EFFECT_CELL))
    );
}

#[test]
fn a_background_child_makes_a_backdrop_pass() {
    assert!(samples_background(
        "uniform shader background; half4 main(float2 p) { return background.eval(p); }"
    ));
    // Declaring both is an object pass that also samples the backdrop.
    assert!(!samples_background(
        "uniform shader object; uniform shader background; half4 main(float2 p) { return background.eval(p); }"
    ));
}

#[test]
fn a_commented_out_background_child_does_not_count() {
    assert!(!samples_background(
        "// uniform shader background;\nuniform shader object; half4 main(float2 p) { return object.eval(p); }"
    ));
    assert!(!samples_background(
        "half4 main(float2 p) { return half4(1); }"
    ));
}
