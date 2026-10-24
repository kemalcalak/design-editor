use std::collections::BTreeMap;

use jian_ops_schema::style::ShaderUniformValue;

use super::*;

const SKSL: &str = "/** @label Speed @range 0, 2 */ uniform float speed;
/** @label Ink @color */ uniform float4 ink;
uniform float2 offset;
uniform float plain;
half4 main(float2 p) { return ink * speed; }";

fn body() -> ShaderBody {
    let mut values = BTreeMap::new();
    values.insert("speed".to_string(), ShaderUniformValue::Float(0.5));
    values.insert(
        "ink".to_string(),
        ShaderUniformValue::Color("#ff0000".into()),
    );
    let mut checks = ShaderCheckCache::default();
    checks.install(SKSL, Some("error: 1: oops".into()));
    ShaderBody::of(
        ShaderProgram {
            sksl: Some(SKSL),
            uniforms: Some(&values),
        },
        Some(&checks),
    )
}

#[test]
fn rows_carry_labels_values_and_the_compile_state() {
    let body = body();
    let labels: Vec<&str> = body.rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["Speed", "Ink", "offset", "plain"]);
    assert_eq!(body.rows[0].numbers, vec![0.5]);
    assert_eq!(body.rows[1].hex.as_deref(), Some("#ff0000"));
    assert_eq!(body.rows[2].numbers, vec![0.0, 0.0]);
    assert_eq!(body.check, ShaderCheck::Error("1: oops".into()));
}

#[test]
fn every_control_gets_a_target_inside_the_body() {
    let body = body();
    let target = ShaderTarget::Fill(0);
    let (x, y, w) = (0.0, 100.0, 272.0);
    let mut actions = Vec::new();
    push_action_rects(&mut actions, target, &body, x, y, w);
    let mut inputs = Vec::new();
    push_input_rects(&mut inputs, target, &body, x, y, w);
    // Slider, colour, Code.
    assert_eq!(actions.len(), 3);
    assert!(matches!(
        actions[0].0,
        PropertyPanelAction::DragShaderUniform { index: 0, .. }
    ));
    assert!(matches!(
        actions[1].0,
        PropertyPanelAction::OpenColorPicker(ColorTarget::ShaderUniform { index: 1, .. })
    ));
    assert_eq!(actions[2].0, PropertyPanelAction::OpenShaderSource(target));
    // Speed's number, offset x / y, plain.
    let focuses: Vec<(usize, usize)> = inputs
        .iter()
        .map(|(focus, _)| match focus {
            PropertyFocus::ShaderUniform {
                index, component, ..
            } => (*index, *component),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(focuses, [(0, 0), (2, 0), (2, 1), (3, 0)]);
    let bottom = y + body_height(&body);
    for rect in actions
        .iter()
        .map(|(_, r)| r)
        .chain(inputs.iter().map(|(_, r)| r))
    {
        assert!(rect.origin.y >= y && rect.origin.y + rect.size.y <= bottom);
        assert!(rect.origin.x >= x && rect.origin.x + rect.size.x <= x + w);
    }
    // The slider and its number do not overlap.
    let slider = actions[0].1;
    let number = inputs[0].1;
    assert!(slider.origin.x + slider.size.x < number.origin.x);
}

#[test]
fn the_slider_maps_its_track_onto_the_range() {
    let slider = Rect::xywh(10.0, 0.0, 100.0, 28.0);
    assert_eq!(slider_value(slider, (0.0, 2.0), 10.0), 0.0);
    assert_eq!(slider_value(slider, (0.0, 2.0), 60.0), 1.0);
    assert_eq!(slider_value(slider, (0.0, 2.0), 500.0), 2.0);
    assert_eq!(slider_value(slider, (-1.0, 1.0), 35.0), -0.5);
}

/// The whole panel with a shader fill and a shader effect: paint ends
/// every section at its shared top (the debug assert in paint), and the
/// walkers reach the bodies of both.
#[test]
fn a_panel_with_shader_fill_and_effect_paints_and_hits_both_bodies() {
    use crate::widgets::property_panel_test_support::{state_from, CountingBackend};
    use crate::widgets::{PaintCx, PropertyPanel, Widget};
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
          {"type":"rectangle","id":"r","name":"R","x":0,"y":0,"width":120,"height":80,
           "fill":[{"type":"shader","sksl":"/** @label Speed @range 0, 2 */ uniform float speed; uniform float2 size; half4 main(float2 p) { return half4(speed); }"}],
           "effects":[{"type":"shader","sksl":"uniform shader object; /** @label Cell */ uniform float cell; half4 main(float2 p) { return object.eval(p); }","uniforms":{"cell":8}}]}
        ]}"##,
    );
    state.set_single_selection(de_editor_core::NodeId::new("r"));
    let panel = PropertyPanel::for_selection(&state).expect("panel");
    let rect = Rect::xywh(0.0, 0.0, 280.0, 1600.0);
    let mut backend = CountingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(&mut cx, rect);
    }
    assert!(backend.texts.iter().any(|t| t == "Speed"));
    assert!(backend.texts.iter().any(|t| t == "Cell"));
    let mut found = (false, false);
    let mut y = 0.0;
    while y < 1600.0 {
        let mut x = 0.0;
        while x < 280.0 {
            match panel.hit_test_action(rect, crate::Point2D::new(x, y)) {
                Some(PropertyPanelAction::DragShaderUniform {
                    target: ShaderTarget::Fill(0),
                    ..
                }) => found.0 = true,
                Some(PropertyPanelAction::OpenShaderSource(ShaderTarget::Effect(0))) => {
                    found.1 = true
                }
                _ => {}
            }
            x += 4.0;
        }
        y += 4.0;
    }
    assert_eq!(found, (true, true));
}

#[test]
fn int_rows_get_fields_and_bool_rows_a_checkbox() {
    let sksl = "uniform int2 grid; /** @bool */ uniform int invert; half4 main(float2 p) { return half4(1); }";
    let mut values = BTreeMap::new();
    values.insert("invert".to_string(), ShaderUniformValue::Float(1.0));
    let body = ShaderBody::of(
        ShaderProgram {
            sksl: Some(sksl),
            uniforms: Some(&values),
        },
        None,
    );
    let target = ShaderTarget::Effect(0);
    let mut actions = Vec::new();
    push_action_rects(&mut actions, target, &body, 0.0, 0.0, 272.0);
    let mut inputs = Vec::new();
    push_input_rects(&mut inputs, target, &body, 0.0, 0.0, 272.0);
    assert_eq!(inputs.len(), 2, "two whole-number fields for the int2");
    assert!(matches!(
        actions[0].0,
        PropertyPanelAction::ToggleShaderUniform {
            index: 1,
            on: false,
            ..
        }
    ));
}
