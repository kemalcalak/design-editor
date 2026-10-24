use serde_json::{json, Value};

use crate::pen_dev::convert;

const WAVES: &str = "precision highp float;
/** @resolution */ uniform vec2 u_resolution;
/** @time */ uniform float u_time;
/**
 * @label Base
 * @color
 * @default #102030
 */
uniform vec3 u_base;
/**
 * @label Accent
 * @color
 * @default #ffffff
 */
uniform vec3 u_accent;
/**
 * @label Speed
 * @range 0, 2
 * @default 0.5
 */
uniform float u_speed;
/** @default 1, 2 */ uniform vec2 u_scale;
void main() {
  vec2 uv = gl_FragCoord.xy / u_resolution * u_scale;
  float w = 0.5 + 0.5 * sin(uv.x * 6.0 + u_time * u_speed);
  gl_FragColor = vec4(mix(u_base, u_accent, w), 1.0);
}
";

const PIXELATE: &str = "/** @resolution */ uniform vec2 u_resolution;
uniform sampler2D u_content;
/** @default 8 */ uniform float u_px;
void main() {
  vec2 cell = (floor(gl_FragCoord.xy / u_px) + 0.5) * u_px;
  gl_FragColor = texture2D(u_content, cell / u_resolution);
}";

fn reader(relative: &str) -> Option<Vec<u8>> {
    match relative {
        "shaders/waves.glsl" => Some(WAVES.as_bytes().to_vec()),
        "fx/pixelate.glsl" => Some(PIXELATE.as_bytes().to_vec()),
        "shaders/broken.glsl" => Some(b"void main() { discard; }".to_vec()),
        _ => None,
    }
}

fn document(children: Value) -> Value {
    json!({
        "version": "2.21",
        "themes": { "mode": ["light", "dark"] },
        "variables": {
            "surface": { "type": "color", "value": [
                { "value": "#f4f4f4" },
                { "value": "#101010", "theme": { "mode": "dark" } }
            ] },
            "pace": { "type": "number", "value": 0.25 }
        },
        "children": children
    })
}

#[test]
fn shader_fills_import_as_sksl_with_resolved_uniforms() {
    let mut doc = document(json!([
        { "type": "frame", "id": "light", "width": 320, "height": 200, "fill": [
            { "type": "shader", "enabled": true, "url": "shaders/waves.glsl?v=6",
              "uniforms": { "u_base": "$surface", "u_speed": "$pace" } }
        ]},
        { "type": "frame", "id": "dark", "theme": { "mode": "dark" }, "width": 10, "height": 10,
          "fill": { "type": "shader", "url": "./shaders/waves.glsl", "opacity": 0.5 } }
    ]));
    let report = convert(&mut doc, &reader);
    let fill = &doc["children"][0]["fill"][0];
    assert_eq!(fill["type"], "shader");
    assert!(fill.get("url").is_none());
    let sksl = fill["sksl"].as_str().unwrap();
    assert!(sksl.contains("half4 main(float2 pen_fragCoord)"), "{sksl}");
    assert!(sksl.contains("uniform float time;"), "{sksl}");
    assert_eq!(
        fill["uniforms"],
        json!({
            "size": [320.0, 200.0],
            "u_base": "#f4f4f4",
            "u_accent": "#ffffff",
            "u_speed": 0.25,
            "u_scale": [1.0, 2.0],
        })
    );
    // The second fill has no values: every uniform takes its `@default`.
    let defaults = &doc["children"][1]["fill"][0];
    assert_eq!(defaults["uniforms"]["u_base"], "#102030");
    assert_eq!(defaults["uniforms"]["u_speed"], 0.5);
    assert_eq!(defaults["opacity"], 0.5);
    assert_eq!(
        report
            .unsupported
            .get("shader uniform variable (fixed value)"),
        Some(&2)
    );
    assert!(!report
        .unsupported
        .keys()
        .any(|k| k.starts_with("shader fill")));
}

#[test]
fn a_themed_variable_resolves_for_the_node_theme() {
    let mut doc = document(json!([
        { "type": "frame", "id": "dark", "theme": { "mode": "dark" }, "fill": [
            { "type": "shader", "url": "shaders/waves.glsl", "uniforms": { "u_base": "$surface" } }
        ]}
    ]));
    convert(&mut doc, &reader);
    let fill = &doc["children"][0]["fill"][0];
    assert_eq!(fill["uniforms"]["u_base"], "#101010");
    // No numeric box: the canvas binds the real size at render time.
    assert_eq!(fill["uniforms"]["size"], json!([1.0, 1.0]));
}

#[test]
fn unreadable_or_broken_shaders_are_dropped_and_reported() {
    let mut doc = document(json!([
        { "type": "frame", "id": "f", "fill": [
            "#000000",
            { "type": "shader", "url": "shaders/gone.glsl" },
            { "type": "shader", "url": "../outside.glsl" },
            { "type": "shader", "url": "shaders/broken.glsl" }
        ]}
    ]));
    let report = convert(&mut doc, &reader);
    assert_eq!(doc["children"][0]["fill"].as_array().unwrap().len(), 1);
    assert_eq!(
        report.unsupported.get("shader fill (GLSL file missing)"),
        Some(&2)
    );
    assert_eq!(
        report.unsupported.get("shader fill (unsupported GLSL)"),
        Some(&1)
    );
}

#[test]
fn shader_effects_sample_the_node_or_its_backdrop() {
    let mut doc = document(json!([
        { "type": "frame", "id": "f", "width": 40, "height": 20, "effect": [
            { "type": "shader", "url": "fx/pixelate.glsl", "uniforms": { "u_content": "pencil:object", "u_px": 4 } },
            { "type": "shader", "url": "fx/pixelate.glsl", "uniforms": { "u_content": "images/a.png" } }
        ]}
    ]));
    let report = convert(&mut doc, &reader);
    let effects = doc["children"][0]["effects"].as_array().unwrap();
    assert_eq!(effects.len(), 1);
    assert!(effects[0]["sksl"]
        .as_str()
        .unwrap()
        .contains("uniform shader object;"));
    assert_eq!(
        effects[0]["uniforms"],
        json!({ "size": [40.0, 20.0], "u_px": 4.0 })
    );
    assert_eq!(
        report
            .unsupported
            .get("shader effect (unsupported texture)"),
        Some(&1)
    );
}

#[test]
fn imported_shader_paints_load_as_canonical_documents() {
    let src = document(json!([
        { "type": "frame", "id": "f", "width": 64, "height": 64, "fill": [
            { "type": "shader", "url": "shaders/waves.glsl", "uniforms": { "u_base": "$surface" } }
        ], "effect": [
            { "type": "shader", "url": "fx/pixelate.glsl", "uniforms": { "u_content": "pencil:background" } }
        ]}
    ]))
    .to_string();
    let loaded =
        crate::payload::load_canonical_with_assets(&src, &reader).expect("pen.dev document loads");
    let jian_ops_schema::node::PenNode::Frame(frame) = &loaded.loaded.value.children[0] else {
        panic!("root is a frame");
    };
    let fills = frame.container.fill.as_ref().expect("fill kept");
    let jian_ops_schema::style::PenFill::Shader(body) = &fills[0] else {
        panic!("shader fill expected, got {:?}", fills[0]);
    };
    assert!(body.sksl.as_deref().unwrap().contains("half4 main"));
    let effects = frame.container.effects.as_ref().expect("effect kept");
    assert!(matches!(
        effects[0],
        jian_ops_schema::style::PenEffect::Shader(_)
    ));
}
