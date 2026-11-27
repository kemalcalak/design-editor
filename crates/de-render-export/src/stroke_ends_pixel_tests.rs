//! #389 acceptance: arrowheads and per-end caps reach the raster export
//! (the shared canvas painter), sized by the stroke width.

use super::*;
use de_editor_core::EditorState;

const BOARD: &str = r##"{
  "version":"1.0.0",
  "children":[{
    "type":"frame","id":"board","x":0,"y":0,"width":200,"height":80,"layout":"none",
    "fill":[{"type":"solid","color":"#FFFFFF"}],
    "children":[
      {"type":"line","id":"plain","x":20,"y":20,"x2":160,"y2":0,
       "stroke":{"thickness":2,"cap":"none","fill":[{"type":"solid","color":"#000000"}]}},
      {"type":"line","id":"arrow","x":20,"y":60,"x2":160,"y2":0,
       "stroke":{"thickness":2,"cap":"none","strokeEnd":"triangle_arrow","strokeStart":"circle",
                 "fill":[{"type":"solid","color":"#000000"}]}}
    ]
  }]
}"##;

/// RGBA of the rendered board at 1×, no margin.
fn render() -> (i32, Vec<u8>) {
    render_doc(BOARD)
}

/// Manual review: `DE_STROKE_ENDS_DOC=<file.de> DE_STROKE_ENDS_PNG=<out.png>
/// cargo test -p de-render-export render_stroke_ends_doc -- --ignored`
/// renders the document's `board` frame.
#[test]
#[ignore]
fn render_stroke_ends_doc() {
    let path = std::env::var_os("DE_STROKE_ENDS_DOC").expect("DE_STROKE_ENDS_DOC");
    render_doc(&std::fs::read_to_string(path).expect("doc"));
}

fn render_doc(src: &str) -> (i32, Vec<u8>) {
    let parsed = jian_ops_schema::load_str(src).expect("fixture");
    let state = EditorState::from_document(parsed.value);
    let scene = de_pen_loader::editor_state_to_layout_scene(&state);
    let node = scene.active_page().unwrap().find("board").expect("board");
    let bytes = render_raster_bytes(node.aggregate_bounds(), RasterFormat::Png, 1.0, 0.0, |c| {
        crate::scene_painter::paint_node(c, node);
    })
    .expect("render");
    if let Some(path) = std::env::var_os("DE_STROKE_ENDS_PNG") {
        std::fs::write(path, &bytes).expect("write png");
    }
    let image = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&bytes)).expect("png");
    let w = image.width();
    let info = skia_safe::ImageInfo::new(
        (w, image.height()),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixels = vec![0u8; (w * image.height() * 4) as usize];
    assert!(image.read_pixels(
        &info,
        pixels.as_mut_slice(),
        (w * 4) as usize,
        (0, 0),
        skia_safe::image::CachingHint::Allow,
    ));
    (w, pixels)
}

fn dark(w: i32, px: &[u8], x: i32, y: i32) -> bool {
    px[((y * w + x) * 4) as usize] < 96
}

#[test]
fn arrowheads_and_end_dots_paint_beyond_the_plain_stroke_band() {
    let (w, px) = render();
    // head_length(2) = 10: the triangle's base corners sit 10 px before
    // the end, 5 px either side; a plain 2 px line never reaches there.
    let (end_x, y) = (180, 60);
    // Pixel rows whose centres sit 2.5 px off the line are inside the
    // head (4 px half-width there) and outside a plain 2 px band.
    assert!(dark(w, &px, end_x - 8, y - 3), "triangle above the line");
    assert!(dark(w, &px, end_x - 8, y + 2), "triangle below the line");
    assert!(
        !dark(w, &px, end_x - 8, 20 - 3),
        "the plain line stays 2 px wide"
    );
    // The circle (r = 4) is centred on the start point.
    assert!(dark(w, &px, 20, y - 3) && dark(w, &px, 17, y), "start dot");
    assert!(!dark(w, &px, 17, 20), "the plain line has no dot");
    // Nothing past the tip.
    assert!(!dark(w, &px, end_x + 2, y));
}
