use super::*;
use crate::node_id::NodeId;
use crate::test_support::{rect, state_with};
use crate::walkers::find_node;
use crate::FillType;

fn shader_body(state: &EditorState) -> ShaderFillBody {
    let node = find_node(state.active_children(), &NodeId::new("a")).expect("node");
    match node_fills(node).and_then(|fills| fills.first()) {
        Some(PenFill::Shader(body)) => body.clone(),
        other => panic!("expected a shader fill, got {other:?}"),
    }
}

#[test]
fn every_preset_round_trips_through_detect() {
    for preset in ShaderPreset::ALL {
        assert_eq!(ShaderPreset::detect(&preset.body("#ff0000")), Some(preset));
    }
    let custom = ShaderFillBody {
        preset: None,
        sksl: Some("half4 main(float2 p){ return half4(1); }".into()),
        uniforms: None,
        explain: None,
        opacity: None,
        blend_mode: None,
    };
    assert_eq!(ShaderPreset::detect(&custom), None);
}

#[test]
fn set_selected_shader_preset_keeps_colour_opacity_and_blend() {
    let mut node = rect("a", "A", 0.0, 0.0, 40.0, 40.0);
    assert!(crate::fills::set_primary_fill_type(
        &mut node,
        FillType::Shader
    ));
    if let Some(PenFill::Shader(body)) = node_fills_mut(&mut node).and_then(|f| f.first_mut()) {
        body.opacity = Some(0.5);
    }
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("a"));
    let hex = first_color_uniform(&shader_body(&s)).unwrap().to_string();

    assert!(s.set_selected_shader_preset(ShaderPreset::Glow));
    let glow = shader_body(&s);
    assert_eq!(ShaderPreset::detect(&glow), Some(ShaderPreset::Glow));
    assert_eq!(glow.opacity, Some(0.5));
    assert_eq!(first_color_uniform(&glow), Some(hex.as_str()));
    assert!(
        !s.set_selected_shader_preset(ShaderPreset::Glow),
        "same preset"
    );

    assert!(s.set_selected_shader_preset(ShaderPreset::Noise));
    let noise = shader_body(&s);
    assert_eq!(noise.preset.as_deref(), Some("turbulence"));
    assert_eq!(noise.sksl, None);
    assert_eq!(
        first_color_uniform(&noise),
        Some(hex.as_str()),
        "Noise keeps the hue"
    );
    assert!(s.set_selected_shader_preset(ShaderPreset::Fade));
    assert_eq!(first_color_uniform(&shader_body(&s)), Some(hex.as_str()));
    assert!(s.set_selected_shader_preset(ShaderPreset::Noise));
    let node = find_node(s.active_children(), &NodeId::new("a")).unwrap();
    assert_eq!(primary_shader_preset(node), Some(ShaderPreset::Noise));
}

#[test]
fn shader_preset_needs_a_primary_shader_fill() {
    let mut s = state_with(vec![rect("a", "A", 0.0, 0.0, 40.0, 40.0)]);
    s.set_single_selection(NodeId::new("a"));
    assert!(!s.set_selected_shader_preset(ShaderPreset::Fade));
}
