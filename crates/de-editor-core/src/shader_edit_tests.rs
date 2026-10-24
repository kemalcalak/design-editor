use jian_ops_schema::style::{PenEffect, PenFill, ShaderFillBody, ShaderUniformValue};

use super::*;
use crate::fills::{node_effects, node_fills_mut};
use crate::node_id::NodeId;
use crate::test_support::{rect, state_with};
use crate::walkers::find_node;
use crate::EditorCommand;

const SKSL: &str = "/** @label Speed @range 0, 2 @default 0.5 */ uniform float speed;
/** @color @default #ff0000 */ uniform float4 tint;
uniform float2 offset;
uniform float2 size;
half4 main(float2 p) { return half4(tint.rgb * speed, 1); }";

fn shader_state() -> EditorState {
    let mut node = rect("a", "A", 0.0, 0.0, 40.0, 40.0);
    *node_fills_mut(&mut node).expect("fills") = vec![PenFill::Shader(ShaderFillBody {
        preset: None,
        sksl: Some(SKSL.into()),
        uniforms: None,
        explain: None,
        opacity: None,
        blend_mode: None,
    })];
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("a"));
    s
}

fn uniforms(s: &EditorState) -> std::collections::BTreeMap<String, ShaderUniformValue> {
    s.selected_shader_program(ShaderTarget::Fill(0))
        .and_then(|p| p.uniforms.cloned())
        .unwrap_or_default()
}

#[test]
fn uniform_writes_are_checked_against_the_declaration() {
    let mut s = shader_state();
    let id = NodeId::new("a");
    let set = |s: &mut EditorState, name: &str, value: ShaderUniformValue| {
        s.apply(EditorCommand::SetShaderUniform {
            node_id: id.clone(),
            target: ShaderTarget::Fill(0),
            name: name.into(),
            value,
        })
    };
    assert!(set(&mut s, "speed", ShaderUniformValue::Float(1.5)));
    assert!(
        !set(&mut s, "speed", ShaderUniformValue::Float(1.5)),
        "unchanged"
    );
    assert!(
        !set(&mut s, "speed", ShaderUniformValue::Vec(vec![1.0, 2.0])),
        "arity"
    );
    assert!(
        !set(&mut s, "size", ShaderUniformValue::Vec(vec![1.0, 2.0])),
        "canvas-bound"
    );
    assert!(
        !set(&mut s, "nope", ShaderUniformValue::Float(1.0)),
        "undeclared"
    );
    assert!(!set(
        &mut s,
        "tint",
        ShaderUniformValue::Color("#xyz".into())
    ));
    assert!(set(
        &mut s,
        "tint",
        ShaderUniformValue::Color("#00ff00".into())
    ));
    assert!(!set(&mut s, "speed", ShaderUniformValue::Float(f32::NAN)));
    let map = uniforms(&s);
    assert_eq!(map["speed"], ShaderUniformValue::Float(1.5));
    assert_eq!(map["tint"], ShaderUniformValue::Color("#00ff00".into()));
    assert!(!s.apply(EditorCommand::SetShaderUniform {
        node_id: id,
        target: ShaderTarget::Effect(0),
        name: "speed".into(),
        value: ShaderUniformValue::Float(1.0),
    }));
}

#[test]
fn number_fields_clamp_ranges_and_write_vector_components() {
    let mut s = shader_state();
    let target = ShaderTarget::Fill(0);
    assert!(s.set_selected_shader_uniform_number(target, 0, 0, 9.0));
    assert_eq!(uniforms(&s)["speed"], ShaderUniformValue::Float(2.0));
    assert!(s.set_selected_shader_uniform_number(target, 2, 1, 4.0));
    assert_eq!(
        uniforms(&s)["offset"],
        ShaderUniformValue::Vec(vec![0.0, 4.0])
    );
    assert!(
        !s.set_selected_shader_uniform_number(target, 2, 2, 4.0),
        "no third component"
    );
    assert!(
        !s.set_selected_shader_uniform_number(target, 1, 0, 1.0),
        "colour row"
    );
}

#[test]
fn a_source_write_drops_the_preset_and_seeds_defaults() {
    let mut s = shader_state();
    s.set_selected_shader_preset(crate::ShaderPreset::Noise);
    let target = ShaderTarget::Fill(0);
    assert_eq!(s.selected_shader_program(target).unwrap().sksl, None);
    assert!(s.apply(EditorCommand::SetShaderSource {
        node_id: NodeId::new("a"),
        target,
        sksl: SKSL.into(),
    }));
    let program = s.selected_shader_program(target).unwrap();
    assert_eq!(program.sksl, Some(SKSL));
    let map = uniforms(&s);
    assert_eq!(map["speed"], ShaderUniformValue::Float(0.5));
    assert_eq!(map["tint"], ShaderUniformValue::Color("#ff0000".into()));
    assert!(!map.contains_key("offset"), "no default, left unbound");
    assert!(
        !s.apply(EditorCommand::SetShaderSource {
            node_id: NodeId::new("a"),
            target,
            sksl: SKSL.into(),
        }),
        "unchanged"
    );
}

#[test]
fn effects_are_targets_too() {
    let mut s = shader_state();
    assert!(s.apply(EditorCommand::AddNodeEffect {
        node_id: NodeId::new("a"),
        kind: "shader".into(),
    }));
    let target = ShaderTarget::Effect(0);
    let specs = s.selected_shader_program(target).unwrap().specs();
    assert_eq!(specs.len(), 1, "the default pixelate cell");
    assert_eq!(specs[0].name, "cell");
    assert!(s.set_selected_shader_uniform_number(target, 0, 0, 12.0));
    assert!(s.apply(EditorCommand::SetShaderSource {
        node_id: NodeId::new("a"),
        target,
        sksl: "uniform shader object; half4 main(float2 p) { return object.eval(p); }".into(),
    }));
    let node = find_node(s.active_children(), &NodeId::new("a")).unwrap();
    match &node_effects(node)[0] {
        PenEffect::Shader(body) => assert!(body.sksl.starts_with("uniform shader object;")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_batch_carries_shader_writes() {
    let mut s = shader_state();
    assert!(s.apply(EditorCommand::Batch {
        commands: vec![EditorCommand::SetShaderUniform {
            node_id: NodeId::new("a"),
            target: ShaderTarget::Fill(0),
            name: "speed".into(),
            value: ShaderUniformValue::Float(1.0),
        }],
    }));
    assert_eq!(uniforms(&s)["speed"], ShaderUniformValue::Float(1.0));
}
