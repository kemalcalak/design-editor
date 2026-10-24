use std::collections::BTreeMap;

use jian_ops_schema::style::ShaderUniformValue;

use super::*;

/// What the `.pen` import writes for an annotated GLSL file (#163).
const IMPORTED: &str = "// Translated from pen.dev GLSL by the .pen import.
uniform float2 size;
uniform float time;
float4 pen_FragCoord;

/** @resolution */


/** @time */


/**
 * @label Ink
 * @color
 * @default #112233
 */
uniform float4 u_ink;

/**
 * @label Count
 * @range 1, 20
 * @default 6
 */
uniform float u_count;
/** @default 1, 2 */ uniform float2 u_scale;
uniform shader u_image;
uniform int u_steps;

half4 main(float2 pen_fragCoord) {
    float uniform_like = 1.0;
    return half4(u_ink.rgb * u_count, 1.0);
}
";

#[test]
fn imported_annotations_become_rows() {
    let specs = uniform_specs(IMPORTED, None);
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["u_ink", "u_count", "u_scale"]);
    assert_eq!(specs[0].kind, UniformKind::Color { arity: 4 });
    assert_eq!(specs[0].display_label(), "Ink");
    assert_eq!(specs[0].default.as_deref(), Some("#112233"));
    assert_eq!(specs[1].kind, UniformKind::Float);
    assert_eq!(specs[1].display_label(), "Count");
    assert_eq!(specs[1].range, Some((1.0, 20.0)));
    assert_eq!(specs[2].kind, UniformKind::Vector(2));
    assert_eq!(specs[2].display_label(), "u_scale");
    assert_eq!(specs[2].range, None);
}

#[test]
fn missing_values_take_the_default_then_zero() {
    let specs = uniform_specs(IMPORTED, None);
    assert_eq!(
        specs[0].value_in(None),
        ShaderUniformValue::Color("#112233".into())
    );
    assert_eq!(specs[1].value_in(None), ShaderUniformValue::Float(6.0));
    assert_eq!(
        specs[2].value_in(None),
        ShaderUniformValue::Vec(vec![1.0, 2.0])
    );
    let mut values = BTreeMap::new();
    values.insert("u_count".to_string(), ShaderUniformValue::Float(3.5));
    // A stored value of the wrong arity is ignored, not bound.
    values.insert("u_scale".to_string(), ShaderUniformValue::Vec(vec![1.0]));
    assert_eq!(specs[1].numbers_in(Some(&values)), vec![3.5]);
    assert_eq!(specs[2].numbers_in(Some(&values)), vec![1.0, 2.0]);
    let bare = uniform_specs(
        "uniform float4 tint; half4 main(float2 p) { return tint; }",
        None,
    );
    assert_eq!(
        bare[0].value_in(None),
        ShaderUniformValue::Vec(vec![0.0; 4])
    );
}

#[test]
fn preset_colours_are_colours_by_their_stored_value() {
    let sksl = crate::ShaderPreset::Fade.body("#ff0000").sksl.unwrap();
    let body = crate::ShaderPreset::Fade.body("#ff0000");
    let specs = uniform_specs(&sksl, body.uniforms.as_ref());
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["accent", "base"], "size is canvas-bound");
    assert!(specs
        .iter()
        .all(|s| s.kind == UniformKind::Color { arity: 4 }));
    assert_eq!(
        specs[0].hex_in(body.uniforms.as_ref()).as_deref(),
        Some("#ff0000")
    );
}

#[test]
fn layout_color_line_comments_and_min_max() {
    let sksl = "// @label Glow strength
// @min 2 @max 0
uniform half glow;
layout(color) uniform half3 tint, edge;
uniform float a, b;
half4 main(float2 p) { return half4(tint * glow, 1); }";
    let specs = uniform_specs(sksl, None);
    assert_eq!(specs[0].display_label(), "Glow strength");
    assert_eq!(specs[0].range, Some((0.0, 2.0)), "ordered low to high");
    assert_eq!(specs[1].kind, UniformKind::Color { arity: 3 });
    assert_eq!(specs[2].name, "edge");
    assert_eq!(specs[2].kind, UniformKind::Color { arity: 3 });
    assert_eq!(
        specs[1].value_in(None),
        ShaderUniformValue::Vec(vec![0.0; 3])
    );
    // A comment separated by code does not annotate the next uniform.
    assert_eq!(specs[3].label, None);
    assert_eq!(specs.len(), 5);
}

#[test]
fn non_ascii_code_does_not_split_a_char() {
    let specs = uniform_specs("uniform float a; float é = 1.0; uniform float b;", None);
    assert_eq!(specs.len(), 2);
}

#[test]
fn colour_coercion_between_hex_and_floats() {
    let three = ShaderUniformSpec {
        name: "c".into(),
        kind: UniformKind::Color { arity: 3 },
        label: None,
        range: None,
        default: Some("#ff0000".into()),
    };
    assert_eq!(
        three.value_in(None),
        ShaderUniformValue::Vec(vec![1.0, 0.0, 0.0])
    );
    assert_eq!(three.hex_in(None).as_deref(), Some("#ff0000"));
    let four = ShaderUniformSpec {
        kind: UniformKind::Color { arity: 4 },
        ..three
    };
    assert_eq!(
        four.coerce(&ShaderUniformValue::Vec(vec![0.0, 0.0, 1.0, 0.5])),
        Some(ShaderUniformValue::Color("#0000ff80".into()))
    );
    assert_eq!(four.coerce(&ShaderUniformValue::Color("nope".into())), None);
}

#[test]
fn sourceless_bodies_list_their_stored_values() {
    let body = crate::ShaderPreset::Noise.body("#7c3aed");
    let specs = stored_value_specs(body.uniforms.as_ref().unwrap());
    let kinds: Vec<(&str, UniformKind)> = specs.iter().map(|s| (s.name.as_str(), s.kind)).collect();
    assert_eq!(
        kinds,
        [
            ("accent", UniformKind::Color { arity: 4 }),
            ("baseFrequency", UniformKind::Vector(2)),
            ("numOctaves", UniformKind::Float),
            ("seed", UniformKind::Float),
        ]
    );
}
