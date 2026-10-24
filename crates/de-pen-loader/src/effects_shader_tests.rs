use super::*;

fn rectangle(effects: &str) -> PenNode {
    serde_json::from_str(&format!(
        r#"{{"type":"rectangle","id":"r1","width":40,"height":20,"effects":{effects}}}"#
    ))
    .expect("rectangle parses")
}

const OBJECT: &str =
    "uniform shader object; uniform float cell; half4 main(float2 p) { return object.eval(p); }";
const BACKDROP: &str =
    "uniform shader background; half4 main(float2 p) { return background.eval(p); }";

#[test]
fn visible_shader_effects_keep_list_order_and_input() {
    let node = rectangle(&format!(
        r##"[{{"type":"shader","sksl":"{BACKDROP}"}},{{"type":"shadow","offsetX":0,"offsetY":4,"blur":8,"spread":0,"color":"#000"}},{{"type":"shader","sksl":"{OBJECT}","uniforms":{{"cell":6,"tint":"#ff000080"}}}},{{"type":"shader","sksl":"{OBJECT}","visible":false}}]"##
    ));
    let payloads = shader_effects_from_canonical(&node);
    assert_eq!(payloads.len(), 2, "hidden effect dropped: {payloads:?}");
    assert!(payloads[0].background);
    assert!(!payloads[1].background);
    let cell = payloads[1]
        .uniforms
        .iter()
        .find(|u| u.name == "cell")
        .expect("float uniform");
    assert_eq!(cell.values, vec![6.0]);
    let tint = payloads[1]
        .uniforms
        .iter()
        .find(|u| u.name == "tint")
        .expect("colour uniform");
    // Premultiplied like a shader fill's colour uniform.
    assert!((tint.values[3] - 128.0 / 255.0).abs() < 1e-3);
    assert!((tint.values[0] - tint.values[3]).abs() < 1e-3);
}

#[test]
fn an_empty_program_is_dropped() {
    let node = rectangle(r#"[{"type":"shader","sksl":"   "}]"#);
    assert!(shader_effects_from_canonical(&node).is_empty());
}

#[test]
fn scene_pass_always_binds_the_node_size() {
    let payload = ShaderEffectPayload {
        sksl: OBJECT.to_string(),
        uniforms: vec![ShaderUniformPayload {
            name: "size".to_string(),
            values: vec![1.0, 1.0],
        }],
        background: false,
    };
    let Effect::Shader(pass) = shader_effect_to_scene(&payload, [40.0, 20.0]) else {
        panic!("shader pass expected");
    };
    assert_eq!(pass.input, ShaderEffectInput::Object);
    let sizes: Vec<_> = pass.uniforms.iter().filter(|u| u.name == "size").collect();
    assert_eq!(sizes.len(), 1);
    assert_eq!(sizes[0].values, vec![40.0, 20.0]);
}
