use de_editor_core::render_backend::ImageBlendMode;
use de_editor_core::EditorState;
use jian_scene::layout_scene::{SceneFillLayer, SceneNode};

use crate::editor_state_to_layout_scene;

fn layered_node(nodes: &[SceneNode]) -> Option<&SceneNode> {
    nodes.iter().find_map(|node| {
        (node.fill_layers.len() >= 3)
            .then_some(node)
            .or_else(|| layered_node(&node.children))
    })
}

#[test]
fn html_background_stack_and_per_layer_blends_reach_layout_scene() {
    let html = r#"
      <div style="
        width: 120px;
        height: 80px;
        background-image:
          linear-gradient(90deg, #ff0000 0%, #0000ff 100%),
          url('data:image/svg+xml;base64,PHN2Zy8+');
        background-color: #112233;
        background-repeat: no-repeat, no-repeat;
        background-size: cover, contain;
        background-blend-mode: screen, multiply;
      "></div>
    "#;
    let imported =
        de_html::import_html_document(html, &de_html::HtmlImportOptions::default(), None, None);
    let state = EditorState::from_document(imported.document);
    let scene = editor_state_to_layout_scene(&state);
    let node = layered_node(&scene.active_page().expect("active page").children)
        .expect("HTML background node with all three fill layers");

    assert_eq!(node.fill_layers.len(), 3);
    assert!(matches!(
        &node.fill_layers[0],
        SceneFillLayer::Gradient {
            blend_mode: ImageBlendMode::Screen,
            ..
        }
    ));
    assert!(matches!(
        &node.fill_layers[1],
        SceneFillLayer::Image {
            blend_mode: ImageBlendMode::Multiply,
            fit: jian_scene::layout_scene::SceneImageFit::Fit,
            ..
        }
    ));
    assert!(matches!(
        &node.fill_layers[2],
        SceneFillLayer::Solid {
            color,
            blend_mode: ImageBlendMode::Normal,
        } if (color.r - 0x11 as f32 / 255.0).abs() < 0.001
          && (color.g - 0x22 as f32 / 255.0).abs() < 0.001
          && (color.b - 0x33 as f32 / 255.0).abs() < 0.001
    ));
}

fn find<'a>(nodes: &'a [SceneNode], hit: &dyn Fn(&SceneNode) -> bool) -> Option<&'a SceneNode> {
    nodes.iter().find_map(|node| {
        hit(node)
            .then_some(node)
            .or_else(|| find(&node.children, hit))
    })
}

/// #244: a card row pinned with `bottom:0` inside a `flex:1` slide body sits
/// on the body's bottom edge once laid out, inside the slide's padding,
/// although the body's height is only known after layout.
#[test]
fn a_bottom_pinned_row_in_a_flex_sized_slide_body_lands_on_its_bottom_edge() {
    let html = r#"<style>
      .sl { width: 1280px; height: 720px; box-sizing: border-box; padding: 60px 72px;
            display: flex; flex-direction: column; }
      .sl + .sl { display: none; }
      h2 { font-size: 44px; margin: 14px 0 0; }
      .bd { flex: 1; position: relative; }
      .row { position: absolute; left: 0; right: 0; bottom: 0; display: flex; gap: 24px; }
      .card { flex: 1; height: 200px; background: #ffffff; }
    </style>
    <section class="sl"><h2>Numbers</h2><div class="bd"><div class="row">
      <div class="card">a</div><div class="card">b</div><div class="card">c</div>
    </div></div></section>
    <section class="sl"><h2>Next</h2></section>"#;
    let imported =
        de_html::import_html_document(html, &de_html::HtmlImportOptions::default(), None, None);
    assert_eq!(imported.document.children.len(), 2, "one board per slide");
    let state = EditorState::from_document(imported.document);
    let scene = editor_state_to_layout_scene(&state);
    let boards = &scene.active_page().expect("active page").children;
    let slide = boards[0].bounds;
    let near = |a: f32, b: f32| (a - b).abs() < 1.0;
    let row = find(&boards[0].children, &|node| {
        near(node.bounds.size.x, 1136.0) && near(node.bounds.size.y, 200.0)
    })
    .expect("the 1136 x 200 card row")
    .bounds;
    assert!(
        near(row.origin.x, slide.origin.x + 72.0),
        "{row:?} in {slide:?}"
    );
    assert!(
        near(row.origin.y + row.size.y, slide.origin.y + 720.0 - 60.0),
        "{row:?} in {slide:?}"
    );
}
