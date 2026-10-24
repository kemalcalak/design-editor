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
fn integer_uniforms_stay_integers() {
    let src = "uniform int steps; uniform ivec2 grid; void main() { gl_FragColor = vec4(float(steps) + float(grid.x)); }";
    let out = translate(src, Stage::Fill, &Map::new()).unwrap();
    assert!(out.sksl.contains("uniform int steps;"), "{}", out.sksl);
    assert!(out.sksl.contains("uniform int2 grid;"), "{}", out.sksl);
    assert!(
        out.sksl.contains("float(steps) + float(grid.x)"),
        "{}",
        out.sksl
    );
    assert_eq!(out.uniforms[0].slot, Slot::Int(1));
    assert_eq!(out.uniforms[1].slot, Slot::Int(2));
}

#[test]
fn boolean_uniforms_bind_as_tagged_ints() {
    let src = "/** @label Invert */ uniform bool flip; uniform bvec2 axes; void main() { gl_FragColor = vec4(flip ? 1.0 : 0.0, axes.x ? 1.0 : 0.0, 0.0, 1.0); }";
    let out = translate(src, Stage::Fill, &Map::new()).unwrap();
    assert!(
        out.sksl
            .contains("/** @label Invert @bool */ uniform int flip;"),
        "{}",
        out.sksl
    );
    assert!(out.sksl.contains("bool(flip) ? 1.0"), "{}", out.sksl);
    assert!(out.sksl.contains("bool2(axes).x"), "{}", out.sksl);
    assert_eq!(out.uniforms[0].slot, Slot::Bool(1));
    assert_eq!(out.uniforms[1].slot, Slot::Bool(2));
}

#[test]
fn discard_returns_transparent() {
    let src = "void main() { if (gl_FragCoord.x < 1.0) discard; gl_FragColor = vec4(1.0); }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(!sksl.contains("discard"), "{sksl}");
    assert!(sksl.contains("< 1.0) return half4(0.0);"), "{sksl}");
    assert!(!sksl.contains("pen_Discard"), "{sksl}");
}

#[test]
fn discard_in_a_helper_sets_a_flag_main_checks() {
    let src = "float edge(float x) { if (x < 0.0) discard; return x; } void main() { gl_FragColor = vec4(edge(gl_FragCoord.x - 2.0)); if (true) return; }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(sksl.contains("bool pen_Discard;"), "{sksl}");
    assert!(
        sksl.contains("{ pen_Discard = true; return 0.0; } return x;"),
        "{sksl}"
    );
    assert!(sksl.contains("pen_Discard = false;"), "{sksl}");
    assert_eq!(
        sksl.matches("return pen_Discard ? half4(0.0) : half4(pen_FragColor)")
            .count(),
        2,
        "{sksl}"
    );
}

#[test]
fn webgl_builtins_read_fragment_values() {
    let src = "void main() { vec2 p = gl_PointCoord; float n = float(gl_MaxDrawBuffers) + gl_DepthRange.far; if (gl_FrontFacing) { gl_FragData[0] = vec4(p, n, 1.0); } }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(
        sksl.contains("float2 p = (pen_FragCoord.xy / size);"),
        "{sksl}"
    );
    assert!(sksl.contains("float(1) + 1.0"), "{sksl}");
    assert!(sksl.contains("if (true)"), "{sksl}");
    assert!(
        sksl.contains("pen_FragColor = float4(p, n, 1.0);"),
        "{sksl}"
    );
}

#[test]
fn projective_lookups_divide_by_the_last_coordinate() {
    let src = "uniform sampler2D tex; void main() { gl_FragColor = texture2DProj(tex, vec3(0.5, 0.5, 2.0)); }";
    let sksl = translate(src, Stage::Fill, &values(json!({ "tex": "a.png" })))
        .unwrap()
        .sksl;
    assert!(sksl.contains("float2 pen_project(float3 p)"), "{sksl}");
    assert!(
        sksl.contains("tex.eval(pen_texel(pen_project(float3(0.5, 0.5, 2.0))))"),
        "{sksl}"
    );
}

#[test]
fn conditional_blocks_and_function_macros_translate() {
    let src = "#ifdef GL_ES
precision highp float;
#endif
#define SQ(x) ((x) * (x))
#if __VERSION__ >= 100 && defined(GL_ES)
#define SHADE 0.5
#else
#define SHADE 1.0
#endif
void main() { gl_FragColor = vec4(SQ(SHADE)); }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(sksl.contains("float4(((0.5) * (0.5)))"), "{sksl}");
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
    assert!(sksl.contains("sin((2.0 * 3.14159))"), "{sksl}");
}

#[test]
fn directive_comments_and_single_token_defines_stay_valid() {
    let src = "#ifdef GL_ES\nprecision mediump float;\n#endif // GL_ES\n#define PI 3.14159 // pi\n#define COLOR vec3\n#define OUT gl_FragColor\nstruct Ray { float time; vec2 size; };\nvoid main() { Ray r = Ray(PI, vec2(1.0)); COLOR c = COLOR(r.time + r.size.x); OUT = vec4(c, 1.0); }";
    let sksl = translate(src, Stage::Fill, &Map::new()).unwrap().sksl;
    assert!(!sksl.contains("// pi"), "{sksl}");
    assert!(sksl.contains("Ray(3.14159, float2(1.0))"), "{sksl}");
    assert!(
        sksl.contains("float3 c = float3(r.pen_time + r.pen_size.x);"),
        "{sksl}"
    );
    assert!(sksl.contains("pen_FragColor = float4(c, 1.0);"), "{sksl}");
}

#[test]
fn two_samplers_on_the_node_share_one_child() {
    let src = "uniform sampler2D a; uniform sampler2D b; void main() { gl_FragColor = texture2D(a, vec2(0.5)) + texture2D(b, vec2(0.5)); }";
    let sksl = translate(
        src,
        Stage::Effect,
        &values(json!({ "a": "pencil:object", "b": "pencil:object" })),
    )
    .unwrap()
    .sksl;
    assert_eq!(sksl.matches("uniform shader object;").count(), 1, "{sksl}");
}

#[test]
fn unsupported_sources_fail_with_a_reason() {
    let fill = |src: &str| translate(src, Stage::Fill, &Map::new()).unwrap_err();
    assert_eq!(fill("float f() { return 1.0; }"), GlslError::NoMain);
    assert!(matches!(
        fill("#if 1\nvoid main() {}"),
        GlslError::Directive(_)
    ));
    assert!(matches!(
        fill("#error unsupported\nvoid main() {}"),
        GlslError::Directive(_)
    ));
    assert_eq!(
        fill("struct S { float a; }; S f() { discard; } void main() {}"),
        GlslError::Unsupported("discard in a function returning a struct")
    );
    assert_eq!(
        fill("void main() { gl_FragData[1] = vec4(1.0); }"),
        GlslError::Unsupported("gl_FragData other than gl_FragData[0]")
    );
    assert!(matches!(
        fill("uniform sampler2D tex; void main() { gl_FragColor = texture2D(tex, vec2(0.0)); }"),
        GlslError::Texture { .. }
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
    // A backdrop-only effect still replaces the node: it declares `object`.
    assert!(
        backdrop.sksl.contains("uniform shader object;"),
        "{}",
        backdrop.sksl
    );
    // A fill samples the fills under it through the same child (#165).
    let fill = translate(
        PIXELATE,
        Stage::Fill,
        &values(json!({ "u_content": "pencil:object" })),
    )
    .unwrap();
    assert!(
        fill.sksl.contains("uniform shader object;"),
        "{}",
        fill.sksl
    );
    assert!(!fill.sksl.contains("uniform shader background;"));
}

const GLASS: &str = "/** @resolution */ uniform vec2 r;
/** @mipmap */ uniform sampler2D u_back;
uniform sampler2D u_node;
/** @sdf */ uniform sampler2D u_shape;
uniform sampler2D u_noise;
void main() {
  vec2 uv = gl_FragCoord.xy / r;
  vec4 s = texture2D(u_shape, uv);
  vec2 bend = s.gb * 0.02 * float(textureSize(u_noise, 0).x);
  vec4 node = texture2D(u_node, uv + bend);
  gl_FragColor = s.r > 0.0 ? texture2D(u_back, uv + bend) * (1.0 - node.a) + node : node * texture2D(u_noise, uv);
}";

#[test]
fn node_backdrop_sdf_and_images_sample_together() {
    let vals = values(
        json!({ "u_back": "pencil:background", "u_node": "pencil:object", "u_noise": "noise.png" }),
    );
    for stage in [Stage::Fill, Stage::Effect] {
        let out = translate(GLASS, stage, &vals).unwrap();
        let sksl = &out.sksl;
        assert!(
            sksl.contains("/** @mipmap */ uniform shader background;"),
            "{sksl}"
        );
        assert!(sksl.contains("uniform shader object;"), "{sksl}");
        assert!(
            sksl.contains("/** @sdf */ uniform shader u_shape;"),
            "{sksl}"
        );
        assert!(
            sksl.contains("pen_sdf(u_shape.eval(pen_texel(uv)))"),
            "{sksl}"
        );
        assert!(sksl.contains("float4 pen_sdf(half4 s)"), "{sksl}");
        assert!(
            sksl.contains("/** @internal */ uniform float2 pen_size_u_noise;"),
            "{sksl}"
        );
        assert!(sksl.contains("int2(pen_size_u_noise).x"), "{sksl}");
        assert_eq!(out.sized_images, ["u_noise"]);
        assert_eq!(out.uniforms[2].slot, Slot::Texture(Texture::Sdf));
    }
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
        (
            "#define PI 3.14159 // pi\n#define COLOR vec3\nstruct Ray { float time; vec2 size; };\nvoid main() { Ray r = Ray(PI, vec2(1.0)); COLOR c = COLOR(r.time + r.size.x); gl_FragColor = vec4(c, 1.0); }",
            Stage::Fill,
            json!({}),
        ),
        (
            "uniform sampler2D a; uniform sampler2D b; void main() { gl_FragColor = texture2D(a, vec2(0.5)) + texture2D(b, vec2(0.5)); }",
            Stage::Effect,
            json!({ "a": "pencil:object", "b": "pencil:object" }),
        ),
        (PIXELATE, Stage::Effect, json!({ "u_content": "pencil:object" })),
        (PIXELATE, Stage::Effect, json!({ "u_content": "pencil:background" })),
        (PIXELATE, Stage::Fill, json!({ "u_content": "images/a.png" })),
        (PIXELATE, Stage::Fill, json!({ "u_content": "pencil:object" })),
        (PIXELATE, Stage::Effect, json!({ "u_content": "images/a.png" })),
        (
            GLASS,
            Stage::Fill,
            json!({ "u_back": "pencil:background", "u_node": "pencil:object", "u_noise": "n.png" }),
        ),
        (
            GLASS,
            Stage::Effect,
            json!({ "u_back": "pencil:background", "u_node": "pencil:object", "u_noise": "n.png" }),
        ),
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
        (
            "/** @label Invert */ uniform bool flip; uniform bvec2 axes; void main() { gl_FragColor = vec4(flip ? 1.0 : 0.0, axes.x ? 1.0 : 0.0, 0.0, 1.0); }",
            Stage::Fill,
            json!({}),
        ),
        (
            "float edge(float x) { if (x < 0.0) discard; else x = x * 2.0; return x; } void main() { if (gl_FragCoord.y < 2.0) discard; gl_FragColor = vec4(edge(gl_FragCoord.x - 2.0)); if (true) return; }",
            Stage::Fill,
            json!({}),
        ),
        (
            "void main() { vec2 p = gl_PointCoord; float n = float(gl_MaxDrawBuffers) + gl_DepthRange.far; if (gl_FrontFacing) { gl_FragData[0] = vec4(p, n, 1.0); } }",
            Stage::Fill,
            json!({}),
        ),
        (
            "uniform sampler2D tex; void main() { gl_FragColor = texture2DProj(tex, vec3(0.5, 0.5, 2.0)) + texture2DProj(tex, vec4(0.5)); }",
            Stage::Fill,
            json!({ "tex": "a.png" }),
        ),
        (
            "#define SQ(x) ((x) * (x))\n#if defined(GL_ES)\n#define SHADE 0.5\n#endif\nvoid main() { gl_FragColor = vec4(SQ(SHADE)); }",
            Stage::Fill,
            json!({}),
        ),
    ];
    for (src, stage, vals) in cases {
        let sksl = translate(src, stage, &values(vals)).unwrap().sksl;
        if let Err(error) = skia_safe::RuntimeEffect::make_for_shader(&sksl, None) {
            panic!("{error}\n---\n{sksl}");
        }
    }
}
