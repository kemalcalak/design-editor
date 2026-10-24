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

#[test]
fn first_color_uniform_skips_image_children() {
    // `a_img` sorts before `tint` but is an image source, not a colour.
    let mut uniforms = std::collections::BTreeMap::new();
    uniforms.insert(
        "a_img".to_string(),
        ShaderUniformValue::Color("data:image/png;base64,AA==".into()),
    );
    uniforms.insert(
        "tint".to_string(),
        ShaderUniformValue::Color("#ff0000".into()),
    );
    let body = ShaderFillBody {
        preset: None,
        sksl: Some(
            "uniform shader a_img; uniform half4 tint; half4 main(float2 p){ return tint; }".into(),
        ),
        uniforms: Some(uniforms),
        explain: None,
        opacity: None,
        blend_mode: None,
    };
    assert_eq!(first_color_uniform(&body), Some("#ff0000"));
}

fn custom_body() -> ShaderFillBody {
    ShaderFillBody {
        preset: None,
        sksl: Some("half4 main(float2 p){ return half4(1, 0, 0, 1); }".into()),
        uniforms: None,
        explain: None,
        opacity: Some(0.4),
        blend_mode: None,
    }
}

fn state_with_custom_shader() -> EditorState {
    let mut node = rect("a", "A", 0.0, 0.0, 40.0, 40.0);
    if let Some(fills) = node_fills_mut(&mut node) {
        *fills = vec![PenFill::Shader(custom_body())];
    }
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("a"));
    s
}

#[test]
fn a_custom_program_offers_custom_and_comes_back_after_a_preset() {
    let mut s = state_with_custom_shader();
    assert!(s.selected_custom_shader_available());
    assert!(
        !s.restore_selected_custom_shader(),
        "nothing stashed while the program is still on the node"
    );

    assert!(s.set_selected_shader_preset(ShaderPreset::Glow));
    assert_eq!(
        ShaderPreset::detect(&shader_body(&s)),
        Some(ShaderPreset::Glow)
    );
    assert!(s.selected_custom_shader_available(), "stashed for the cell");
    // A second preset pick does not overwrite the stash with a preset.
    assert!(s.set_selected_shader_preset(ShaderPreset::Fade));

    assert!(s.restore_selected_custom_shader());
    let back = shader_body(&s);
    assert_eq!(back.sksl, custom_body().sksl);
    assert_eq!(back.opacity, Some(0.4));
    assert!(!s.restore_selected_custom_shader(), "already the program");
}

#[test]
fn undo_across_a_preset_pick_keeps_custom_available() {
    let mut s = state_with_custom_shader();
    let snap = s.snapshot_for_history();
    assert!(s.set_selected_shader_preset(ShaderPreset::Noise));
    s.history_push_past(snap);

    assert!(s.undo());
    assert_eq!(shader_body(&s).sksl, custom_body().sksl);
    assert!(s.redo());
    assert_eq!(
        ShaderPreset::detect(&shader_body(&s)),
        Some(ShaderPreset::Noise)
    );
    assert!(s.selected_custom_shader_available());
    assert!(s.restore_selected_custom_shader());
    assert_eq!(shader_body(&s).sksl, custom_body().sksl);
}

#[test]
fn a_replaced_document_drops_the_stash() {
    let mut s = state_with_custom_shader();
    assert!(s.set_selected_shader_preset(ShaderPreset::Fade));
    let doc = s.doc.clone();
    s.replace_document(doc);
    s.set_single_selection(NodeId::new("a"));
    assert!(!s.selected_custom_shader_available());
    assert!(!s.restore_selected_custom_shader());
}
