use std::collections::BTreeMap;

use jian_ops_schema::style::ShaderUniformValue;
use serde_json::json;

use super::super::{resolve_document_for_canvas, Theme};

fn doc(theme: &str) -> jian_ops_schema::PenDocument {
    serde_json::from_value(json!({
        "version": "1.0",
        "themes": { "mode": ["light", "dark"] },
        "variables": {
            "accent": { "type": "color", "value": [
                { "value": "#ff0000", "theme": { "mode": "light" } },
                { "value": "#0000ff", "theme": { "mode": "dark" } }
            ]},
            "speed": { "type": "number", "value": 2 },
            "on": { "type": "boolean", "value": true }
        },
        "children": [{
            "type": "rectangle", "id": "r", "width": 10, "height": 10,
            "theme": { "mode": theme },
            "fill": [{ "type": "shader", "sksl": "half4 main(float2 p) { return half4(1); }",
                "uniforms": { "tint": "$accent", "speed": "$speed", "flag": "$on",
                    "gone": "$missing", "plain": 1.5 } }],
            "effects": [{ "type": "shader", "sksl": "half4 main(float2 p) { return half4(1); }",
                "uniforms": { "tint": "$accent" } }]
        }]
    }))
    .expect("document parses")
}

fn fill_uniforms(doc: &jian_ops_schema::PenDocument) -> BTreeMap<String, ShaderUniformValue> {
    let node = serde_json::to_value(&doc.children[0]).expect("node serializes");
    serde_json::from_value(node["fill"][0]["uniforms"].clone()).expect("uniforms")
}

#[test]
fn variable_uniforms_resolve_per_theme() {
    let light = resolve_document_for_canvas(&doc("light"), &Theme::new());
    let uniforms = fill_uniforms(&light);
    assert_eq!(
        uniforms["tint"],
        ShaderUniformValue::Color("#ff0000".into())
    );
    assert_eq!(uniforms["speed"], ShaderUniformValue::Float(2.0));
    assert_eq!(uniforms["flag"], ShaderUniformValue::Float(1.0));
    assert_eq!(uniforms["plain"], ShaderUniformValue::Float(1.5));
    assert!(
        !uniforms.contains_key("gone"),
        "an unresolved reference unbinds"
    );

    let dark = resolve_document_for_canvas(&doc("dark"), &Theme::new());
    assert_eq!(
        fill_uniforms(&dark)["tint"],
        ShaderUniformValue::Color("#0000ff".into())
    );
    let effect = serde_json::to_value(&dark.children[0]).expect("node");
    assert_eq!(effect["effects"][0]["uniforms"]["tint"], "#0000ff");
}

#[test]
fn documents_with_shader_variables_have_tokens() {
    assert!(super::super::document_has_tokens(&doc("light")));
}

#[test]
fn renaming_and_deleting_a_variable_rewrites_uniforms() {
    let source = doc("light");
    let vars = source.variables.clone();
    let mut nodes = source.children.clone();
    super::super::replace_variable_refs_in_tree(
        &mut nodes,
        "speed",
        Some("pace"),
        vars.as_ref(),
        &Theme::new(),
    );
    let node = serde_json::to_value(&nodes[0]).expect("node");
    assert_eq!(node["fill"][0]["uniforms"]["speed"], "$pace");

    let mut nodes = source.children.clone();
    super::super::replace_variable_refs_in_tree(
        &mut nodes,
        "accent",
        None,
        vars.as_ref(),
        &Theme::new(),
    );
    let node = serde_json::to_value(&nodes[0]).expect("node");
    assert_eq!(node["fill"][0]["uniforms"]["tint"], "#ff0000");
    assert_eq!(node["effects"][0]["uniforms"]["tint"], "#ff0000");
}
