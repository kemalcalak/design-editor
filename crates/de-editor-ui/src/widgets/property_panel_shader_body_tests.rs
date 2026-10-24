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
