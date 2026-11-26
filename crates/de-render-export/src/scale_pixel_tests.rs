//! #394 acceptance: a card scaled ×2 with the Scale tool's core renders
//! like the original card rendered at 200 %.

use super::*;
use de_editor_core::{EditorState, NodeId, ScaleRequest, ScaleTarget, VariableMode};

const CARD: &str = r##"{
  "version":"1.0.0",
  "children":[{
    "type":"frame","id":"card","x":10,"y":10,"width":160,"height":110,
    "layout":"vertical","gap":8,"padding":[12,14],"cornerRadius":12,
    "fill":[{"type":"solid","color":"#FFFFFF"}],
    "stroke":{"thickness":1.5,"fill":[{"type":"solid","color":"#334155"}]},
    "effects":[{"type":"shadow","offsetX":0,"offsetY":3,"blur":6,"spread":0,"color":"#00000040"}],
    "children":[
      {"type":"rectangle","id":"hero","width":"fill_container","height":36,"cornerRadius":6,
       "fill":[{"type":"solid","color":"#3366FF"}]},
      {"type":"frame","id":"row","layout":"horizontal","gap":6,"width":"fill_container","height":20,
       "children":[
         {"type":"ellipse","id":"dot","width":20,"height":20,
          "fill":[{"type":"solid","color":"#F97316"}],
          "stroke":{"thickness":2,"fill":[{"type":"solid","color":"#7C2D12"}]}},
         {"type":"rectangle","id":"bar","width":"fill_container","height":8,"cornerRadius":4,
          "fill":[{"type":"solid","color":"#94A3B8"}]}
       ]},
      {"type":"path","id":"tick","width":16,"height":12,"d":"M1 6 L6 11 L15 1",
       "stroke":{"thickness":2,"fill":[{"type":"solid","color":"#16A34A"}]}}
    ]
  }]
}"##;

fn render(state: &EditorState, scale: f32, margin: f32) -> (i32, i32, Vec<u8>) {
    let scene = de_pen_loader::editor_state_to_layout_scene(state);
    let node = scene.active_page().unwrap().find("card").expect("card");
    let bounds = node.aggregate_bounds();
    let bytes = render_raster_bytes(bounds, RasterFormat::Png, scale, margin, |canvas| {
        crate::scene_painter::paint_node(canvas, node);
    })
    .expect("render");
    let image = skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&bytes)).expect("png");
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
        pixels.as_mut_slice(),
        (w * 4) as usize,
        (0, 0),
        skia_safe::image::CachingHint::Allow,
    ));
    (w, h, pixels)
}

#[test]
fn a_card_scaled_twice_looks_like_the_card_at_200_percent() {
    let parsed = jian_ops_schema::load_str(CARD).expect("fixture");
    let original = EditorState::from_document(parsed.value);
    let mut scaled = original.clone();
    scaled
        .scale_nodes(&ScaleRequest {
            targets: vec![ScaleTarget {
                node_id: NodeId::new("card"),
                pivot: Some((10.0, 10.0)),
            }],
            factor: 2.0,
            variables: VariableMode::KeepBound,
        })
        .expect("scaled");

    let zoomed = render(&original, 2.0, 16.0);
    let direct = render(&scaled, 1.0, 32.0);
    assert_eq!(
        (zoomed.0, zoomed.1),
        (direct.0, direct.1),
        "same raster size"
    );
    let total = (zoomed.0 * zoomed.1) as usize;
    let differing = zoomed
        .2
        .chunks_exact(4)
        .zip(direct.2.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 48))
        .count();
    // Anti-aliased edges may land a sub-pixel apart; nothing else may.
    assert!(
        differing * 200 < total,
        "{differing} of {total} pixels differ"
    );
}
