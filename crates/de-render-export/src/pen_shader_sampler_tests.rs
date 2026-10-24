//! pen.dev shader samplers end to end (#165): a `.pen` document with
//! neutral GLSL shaders imports, lays out and renders through the same
//! painter as the canvas. Set `DE_SHOTS_DIR` to also write the PNGs.

use std::collections::BTreeMap;

use super::*;

const GLASS: &str = "/** @resolution */ uniform vec2 r;
/** @mipmap */ uniform sampler2D u_back;
/** @sdf */ uniform sampler2D u_shape;
uniform sampler2D u_tint;
void main() {
  vec2 uv = gl_FragCoord.xy / r;
  vec4 s = texture2D(u_shape, uv);
  float edge = clamp(1.0 - s.r / 14.0, 0.0, 1.0);
  vec2 bend = s.gb * edge * edge * 0.12;
  vec4 back = texture2D(u_back, uv - bend);
  vec4 tint = texture2D(u_tint, uv);
  gl_FragColor = vec4(mix(back.rgb, tint.rgb, 0.35) + edge * 0.2, 1.0);
}";

/// Outlines the node's rendered content (its SDF), shows the node over its
/// backdrop elsewhere, and tints by an image whose aspect it reads.
const OUTLINE: &str = "/** @resolution */ uniform vec2 r;
uniform sampler2D u_node;
uniform sampler2D u_back;
/** @sdf */ uniform sampler2D u_edge;
uniform sampler2D u_img;
#define WIDE(t) (float(textureSize(t, 0).x) / float(textureSize(t, 0).y) > 1.5)
void main() {
  vec2 uv = gl_FragCoord.xy / r;
  float d = texture2D(u_edge, uv).r;
  vec4 node = texture2D(u_node, uv);
  if (abs(d) < 2.0) { gl_FragColor = WIDE(u_img) ? vec4(0.0, 1.0, 0.0, 1.0) : vec4(1.0); return; }
  gl_FragColor = node + texture2D(u_back, uv) * (1.0 - node.a);
}";

const FLAGS: &str = "uniform bool u_on;
uniform int u_n;
/** @color */ uniform vec3 u_tint;
float pick(float x) { if (x < 0.0) discard; return x; }
void main() {
#if __VERSION__ >= 100 && defined(GL_ES)
  bool ok = u_on && u_n == 3;
#else
  bool ok = false;
#endif
  gl_FragColor = ok ? vec4(u_tint * pick(1.0), 1.0) : vec4(0.0, 1.0, 0.0, 1.0);
}";

/// A 2 x 1 PNG: the image the outline shader reads the aspect of.
fn wide_png() -> Vec<u8> {
    let mut surface = skia_safe::surfaces::raster_n32_premul((2, 1)).expect("surface");
    surface.canvas().clear(skia_safe::Color::RED);
    surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::PNG, None)
        .expect("png")
        .as_bytes()
        .to_vec()
}

fn document() -> String {
    serde_json::json!({
        "version": "2.21",
        "themes": { "mode": ["light", "dark"] },
        "variables": {
            "accent": { "type": "color", "value": [
                { "value": "#ff0000" },
                { "value": "#0000ff", "theme": { "mode": "dark" } }
            ] }
        },
        "children": [{
            "type": "frame", "id": "board", "x": 0, "y": 0, "width": 360, "height": 140,
            "fill": "#203040", "layout": "none",
            "children": [
                { "type": "rectangle", "id": "stripes", "x": 10, "y": 10, "width": 160, "height": 120,
                  "fill": { "type": "gradient", "gradientType": "linear", "rotation": 90,
                            "colors": [{ "color": "#ffcc00", "position": 0 }, { "color": "#ff3366", "position": 1 }] } },
                { "type": "rectangle", "id": "glass", "x": 30, "y": 30, "width": 120, "height": 80,
                  "cornerRadius": 24,
                  "fill": [
                      "#ffffff",
                      { "type": "shader", "url": "fx/glass.glsl",
                        "uniforms": { "u_back": "pencil:background", "u_tint": "pencil:object" } }
                  ] },
                { "type": "frame", "id": "outlined", "x": 190, "y": 10, "width": 70, "height": 120,
                  "fill": "#0000ff", "layout": "none",
                  "effect": [{ "type": "shader", "url": "fx/outline.glsl",
                               "uniforms": { "u_node": "pencil:object", "u_back": "pencil:background",
                                             "u_img": "fx/wide.png" } }],
                  "children": [
                      { "type": "rectangle", "id": "core", "x": 15, "y": 40, "width": 40, "height": 40, "fill": "#ff0000" }
                  ] },
                { "type": "rectangle", "id": "flags", "x": 280, "y": 10, "width": 70, "height": 120,
                  "fill": [{ "type": "shader", "url": "fx/flags.glsl",
                             "uniforms": { "u_on": true, "u_n": 3, "u_tint": "$accent" } }] }
            ]
        }]
    })
    .to_string()
}

fn reader(relative: &str) -> Option<Vec<u8>> {
    match relative {
        "fx/glass.glsl" => Some(GLASS.as_bytes().to_vec()),
        "fx/outline.glsl" => Some(OUTLINE.as_bytes().to_vec()),
        "fx/flags.glsl" => Some(FLAGS.as_bytes().to_vec()),
        "fx/wide.png" => Some(wide_png()),
        _ => None,
    }
}

/// The imported document rendered under `mode`, at 2x, decoded to RGBA.
fn render(mode: &str) -> (Vec<u8>, i32, Vec<u8>) {
    let loaded = de_pen_loader::payload::load_canonical_with_assets(&document(), &reader)
        .expect("the pen.dev document loads");
    let report = loaded.compatibility.pen_dev.expect("converted as pen.dev");
    assert!(
        !report.unsupported.keys().any(|k| k.contains("shader")),
        "no shader is skipped: {}",
        report.summary()
    );
    let mut theme = BTreeMap::new();
    theme.insert("mode".to_owned(), mode.to_owned());
    let scene = de_pen_loader::pen_document_to_layout_scene(&loaded.loaded.value, &theme, 0);
    let png = render_page_raster_bytes(&scene.pages[0], RasterFormat::Png, 2.0)
        .expect("the page renders");
    let image = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&png)).expect("decodes");
    let (w, h) = (image.width(), image.height());
    let info = skia_safe::ImageInfo::new(
        (w, h),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    assert!(image.read_pixels(
        &info,
        &mut pixels,
        (w * 4) as usize,
        (0, 0),
        skia_safe::image::CachingHint::Allow
    ));
    (pixels, w, png)
}

fn at(pixels: &[u8], width: i32, x: i32, y: i32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

#[test]
fn imported_sampler_shaders_render() {
    let (pixels, width, png) = render("light");
    if let Ok(dir) = std::env::var("DE_SHOTS_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join("pen-samplers-light.png"),
            &png,
        )
        .expect("write shot");
    }
    // The page starts at the board (no export margin), at 2x.
    let p = |x: i32, y: i32| at(&pixels, width, x * 2, y * 2);
    // Flags: bool + int + live colour variable (light = red).
    let flags = p(315, 70);
    assert!(flags[0] > 230 && flags[1] < 30, "flags red: {flags:?}");
    // Outline effect: the edge of the node's content (its blue frame)
    // turns green (the image is wide); inside, the content shows.
    let middle = p(225, 70);
    assert!(middle[0] > 230 && middle[1] < 30, "core: {middle:?}");
    let edge = p(190, 70);
    assert!(edge[1] > 200 && edge[0] < 60, "edge green: {edge:?}");
    let inside = p(200, 20);
    assert!(inside[2] > 200 && inside[1] < 40, "frame: {inside:?}");
    // Glass: neither the fallback grey nor transparent, and brighter at
    // the rim than flat white over the stripes would be.
    let glass = p(90, 70);
    assert_eq!(glass[3], 255, "{glass:?}");
    assert!(glass != [128, 128, 128, 255], "{glass:?}");
}

#[test]
fn a_variable_bound_uniform_follows_the_theme() {
    let (pixels, width, png) = render("dark");
    if let Ok(dir) = std::env::var("DE_SHOTS_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join("pen-samplers-dark.png"),
            &png,
        )
        .expect("write shot");
    }
    let flags = at(&pixels, width, 315 * 2, 70 * 2);
    assert!(
        flags[2] > 230 && flags[0] < 30,
        "dark accent is blue: {flags:?}"
    );
}
