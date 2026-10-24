use serde_json::{json, Map, Value};

use super::{translate, Slot, Stage, Texture};
use crate::pen_dev::glsl_error::GlslError;

const STRIPES: &str = "precision highp float;

/** @resolution */
uniform vec2 u_resolution;

/** @time */
uniform float u_time;

/**
 * @label Ink
 * @color
 * @default #112233
 */
uniform vec3 u_ink;

/**
 * @label Count
 * @range 1, 20
 * @default 6
 */
uniform float u_count;

mat2 turn(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }

void main() {
  vec2 uv = gl_FragCoord.xy / u_resolution;
  uv = turn(u_time * 0.1) * uv;
  float band = step(0.5, fract(uv.x * u_count));
  for (int i = 0; i < 3; i++) { band *= 0.9; }
  gl_FragColor = vec4(mix(u_ink, vec3(1.0), band), 1.0);
}
";

fn values(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

#[test]
fn main_and_frag_builtins_become_an_sksl_entry_point() {
    let out = translate(STRIPES, Stage::Fill, &Map::new()).unwrap();
    let sksl = &out.sksl;
    assert!(!sksl.contains("precision"), "{sksl}");
    assert!(sksl.contains("half4 main(float2 pen_fragCoord)"), "{sksl}");
    // GL's origin is bottom-left: y flips against the node height.
    assert!(sksl.contains("size.y - pen_fragCoord.y"), "{sksl}");
    assert!(
        sksl.contains("float2 uv = pen_FragCoord.xy / size;"),
        "{sksl}"
    );
    assert!(
        sksl.contains("pen_FragColor = float4(mix((u_ink.rgb), float3(1.0), band), 1.0);")
            || sksl.contains("pen_FragColor = float4(mix(u_ink.rgb, float3(1.0), band), 1.0);"),
        "{sksl}"
    );
    assert!(
        sksl.trim_end().ends_with("return half4(pen_FragColor);\n}"),
        "{sksl}"
    );
    assert!(sksl.contains("float2x2 turn(float a)"), "{sksl}");
    assert!(!sksl.contains("gl_"), "{sksl}");
}

#[test]
fn annotated_uniforms_map_onto_the_canvas_bindings() {
    let out = translate(STRIPES, Stage::Fill, &Map::new()).unwrap();
    let sksl = &out.sksl;
    assert!(sksl.contains("uniform float2 size;"), "{sksl}");
    assert!(sksl.contains("uniform float time;"), "{sksl}");
    assert!(sksl.contains("turn(time * 0.1)"), "{sksl}");
    assert!(
        !sksl.contains("u_resolution") && !sksl.contains("u_time"),
        "{sksl}"
    );
    // No pointer uniform unless the shader asks for one: declaring it makes
    // the canvas repaint on pointer moves.
    assert!(!sksl.contains("mouse"), "{sksl}");
    let names: Vec<(&str, &Slot, Option<&str>)> = out
        .uniforms
        .iter()
        .map(|u| (u.sksl_name.as_str(), &u.slot, u.default.as_deref()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("u_ink", &Slot::Colour, Some("#112233")),
            ("u_count", &Slot::Float, Some("6")),
        ]
    );
    // A colour vec3 is bound as a float4 colour and read through `.rgb`.
    assert!(sksl.contains("uniform float4 u_ink;"), "{sksl}");
    assert!(sksl.contains("u_ink.rgb"), "{sksl}");
}

#[test]
fn the_pointer_flips_like_frag_coord() {
    let src = "/** @mouse */ uniform vec2 u_mouse;\n/** @resolution */ uniform vec2 r;\nvoid main(void) { gl_FragColor = vec4(u_mouse / r, 0.0, 1.0); }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(sksl.contains("uniform float2 mouse;"), "{sksl}");
    assert!(
        sksl.contains("(float2(mouse.x, size.y - mouse.y)) / size"),
        "{sksl}"
    );
}

#[test]
fn a_string_value_makes_an_unannotated_vector_a_colour() {
    let src = "uniform vec3 tint; uniform vec2 offset; void main() { gl_FragColor = vec4(tint + vec3(offset, 0.0), 1.0); }";
    let out = translate(src, Stage::Fill, &values(json!({ "tint": "#ff0000" }))).unwrap();
    assert_eq!(out.uniforms[0].slot, Slot::Colour);
    assert_eq!(out.uniforms[1].slot, Slot::Vector(2));
    assert!(out.sksl.contains("uniform float2 offset;"), "{}", out.sksl);
}

#[test]
fn integer_uniforms_bind_as_floats() {
    let src = "uniform int steps; uniform ivec2 grid; void main() { gl_FragColor = vec4(float(steps) + float(grid.x)); }";
    let out = translate(src, Stage::Fill, &Map::new()).unwrap();
    assert!(out.sksl.contains("uniform float steps;"), "{}", out.sksl);
    assert!(out.sksl.contains("float(int(steps))"), "{}", out.sksl);
    assert!(out.sksl.contains("float(int2(grid).x)"), "{}", out.sksl);
}

#[test]
fn names_sksl_owns_are_renamed() {
    let src = "uniform float size; float half(float x) { return x * 0.5; } void main() { float time = half(size); gl_FragColor = vec4(time); }";
    let out = translate(src, Stage::Fill, &Map::new()).unwrap();
    assert_eq!(out.uniforms[0].glsl_name, "size");
    assert_eq!(out.uniforms[0].sksl_name, "pen_size");
    assert!(out.sksl.contains("float pen_half(float x)"), "{}", out.sksl);
    assert!(
        out.sksl.contains("float pen_time = pen_half(pen_size);"),
        "{}",
        out.sksl
    );
}

#[test]
fn early_returns_in_main_return_the_colour() {
    let src = "void main() { gl_FragColor = vec4(1.0); if (gl_FragCoord.x < 1.0) { return; } gl_FragColor.r = 0.0; }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(sksl.contains("{ return half4(pen_FragColor); }"), "{sksl}");
}

#[test]
fn defines_and_gl_es_guards_are_preprocessed() {
    let src = "#version 100\n#ifdef GL_ES\nprecision mediump float;\n#endif\n#define PI 3.14159\n#define TAU (2.0 * PI)\nvoid main() { gl_FragColor = vec4(sin(TAU)); }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(!sksl.contains('#'), "{sksl}");
    assert!(sksl.contains("sin(((2.0 * (3.14159))))"), "{sksl}");
}

#[test]
fn unsupported_sources_fail_with_a_reason() {
    let fill = |src: &str| translate(src, Stage::Fill, &Map::new()).unwrap_err();
    assert_eq!(fill("float f() { return 1.0; }"), GlslError::NoMain);
    assert!(matches!(
        fill("#if 1\n#endif\nvoid main() {}"),
        GlslError::Directive(_)
    ));
    assert!(matches!(
        fill("#define F(x) x\nvoid main() {}"),
        GlslError::Directive(_)
    ));
    assert_eq!(
        fill("void main() { discard; }"),
        GlslError::Unsupported("discard")
    );
    assert!(matches!(
        fill("uniform sampler2D tex; void main() { gl_FragColor = texture2D(tex, vec2(0.0)); }"),
        GlslError::Uniform { .. }
    ));
    assert!(matches!(
        fill("uniform mat3 m; void main() {}"),
        GlslError::Uniform { .. }
    ));
}

const PIXELATE: &str = "/** @resolution */ uniform vec2 u_resolution;
/** @label Content */ uniform sampler2D u_content;
/** @default 8 */ uniform float u_px;
void main() {
  vec2 cell = (floor(gl_FragCoord.xy / u_px) + 0.5) * u_px;
  gl_FragColor = texture2D(u_content, cell / u_resolution);
}";

#[test]
fn node_textures_become_effect_children() {
    let out = translate(
        PIXELATE,
        Stage::Effect,
        &values(json!({ "u_content": "pencil:object" })),
    )
    .unwrap();
    assert!(out.sksl.contains("uniform shader object;"), "{}", out.sksl);
    assert!(
        out.sksl.contains("object.eval(pen_texel(cell / size))"),
        "{}",
        out.sksl
    );
    assert_eq!(out.uniforms[0].slot, Slot::Texture(Texture::Object));
    let backdrop = translate(
        PIXELATE,
        Stage::Effect,
        &values(json!({ "u_content": "pencil:background" })),
    )
    .unwrap();
    assert!(backdrop.sksl.contains("uniform shader background;"));
    // A fill cannot sample the node itself.
    let label = translate(
        PIXELATE,
        Stage::Fill,
        &values(json!({ "u_content": "pencil:object" })),
    )
    .unwrap_err()
    .label(false);
    assert_eq!(label, "shader fill (unsupported texture)");
}

#[test]
fn image_textures_become_fill_image_children() {
    let out = translate(
        PIXELATE,
        Stage::Fill,
        &values(json!({ "u_content": "images/a.png" })),
    )
    .unwrap();
    assert!(
        out.sksl.contains("uniform shader u_content;"),
        "{}",
        out.sksl
    );
    assert_eq!(
        out.uniforms[0].slot,
        Slot::Texture(Texture::Image("images/a.png".to_owned()))
    );
}

/// Every fixture above compiles as a Skia runtime effect.
#[cfg(feature = "sksl-check")]
#[test]
fn translated_fixtures_compile_in_skia() {
    let cases: Vec<(&str, Stage, Value)> = vec![
        (STRIPES, Stage::Fill, json!({})),
        (PIXELATE, Stage::Effect, json!({ "u_content": "pencil:object" })),
        (PIXELATE, Stage::Effect, json!({ "u_content": "pencil:background" })),
        (PIXELATE, Stage::Fill, json!({ "u_content": "images/a.png" })),
        (
            "/** @mouse */ uniform vec2 m; float glow() { return length(gl_FragCoord.xy - m); } void main() { gl_FragColor = vec4(vec3(1.0 / glow()), 1.0); }",
            Stage::Fill,
            json!({}),
        ),
        (
            "uniform int steps; uniform ivec2 grid; uniform vec3 tint; void main() { float v = 0.0; for (int i = 0; i < 4; i++) { if (i >= steps) break; v += mod(float(i), 2.0); } gl_FragColor = vec4(tint * v + vec3(float(grid.x)), atan(1.0, 2.0)); }",
            Stage::Fill,
            json!({ "tint": "#00ff00" }),
        ),
    ];
    for (src, stage, vals) in cases {
        let sksl = translate(src, stage, &values(vals)).unwrap().sksl;
        if let Err(error) = skia_safe::RuntimeEffect::make_for_shader(&sksl, None) {
            panic!("{error}\n---\n{sksl}");
        }
    }
}
