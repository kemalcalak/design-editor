//! Present replays the arriving slide's entrances (#245) — through the
//! host's own step, pump and paint path.

#![cfg(all(test, feature = "gl-host", not(target_os = "windows")))]

use super::WidgetHostNative;
use de_editor_core::scene_template_catalog::TemplateScene;
use de_editor_core::EditorState;
use de_editor_ui::{Color, Point2D, Rect, RenderBackend, TextLayout};

const VIEWPORT: (f32, f32) = (1200.0, 800.0);

/// Two slides, each with a 400 ms fade-in on its title.
fn deck() -> WidgetHostNative {
    let fade = r#"[{"trigger":"mount","durationMs":400,"easing":"linear",
        "keyframes":{"from":{"opacity":0},"to":{"opacity":1}}}]"#;
    let source = format!(
        r##"{{"version":"1.0.0","children":[
            {{"type":"frame","id":"slide-1","x":0,"y":0,"width":1920,"height":1080,
              "fill":[{{"type":"solid","color":"#ffffff"}}],"children":[
                {{"type":"rectangle","id":"title-1","x":100,"y":100,"width":800,"height":120,
                  "opacity":1,"fill":[{{"type":"solid","color":"#111111"}}],"animations":{fade}}}]}},
            {{"type":"frame","id":"slide-2","x":2100,"y":0,"width":1920,"height":1080,
              "fill":[{{"type":"solid","color":"#ffffff"}}],"children":[
                {{"type":"rectangle","id":"title-2","x":100,"y":100,"width":800,"height":120,
                  "opacity":1,"fill":[{{"type":"solid","color":"#111111"}}],"animations":{fade}}}]}}
        ]}}"##
    );
    let document = jian_ops_schema::load_str(&source).expect("deck parses").value;
    let mut state = EditorState::from_document(document);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    let mut host = WidgetHostNative::new();
    host.install_imported_state(state);
    host.last_viewport_w = VIEWPORT.0;
    host.last_viewport_h = VIEWPORT.1;
    host
}

/// Paints nowhere: the test reads the scene, the paint only has to run.
struct NullBackend;

impl RenderBackend for NullBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn opacity(host: &WidgetHostNative, id: &str) -> f32 {
    host.preview_scene_for_test()
        .active_page()
        .and_then(|page| page.find(id))
        .map(|node| node.opacity)
        .expect("scene node")
}

fn at(host: &mut WidgetHostNative, now_ms: u64) {
    host.set_now_ms(now_ms);
    let _ = host.pump_preview();
    host.paint_slideshow(
        &mut NullBackend,
        Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(VIEWPORT.0, VIEWPORT.1),
        },
    );
}

#[test]
fn stepping_to_a_slide_plays_its_entrance_again() {
    let mut host = deck();
    host.set_now_ms(600_000);
    assert!(host.enter_preview(VIEWPORT));
    assert!(host.preview_slideshow_active());
    host.settle_mode_transition();
    at(&mut host, 600_100);

    // Long after entry every slide's entrance has finished.
    at(&mut host, 610_000);
    assert!((opacity(&host, "title-2") - 1.0).abs() < 0.001);

    assert!(host.preview_slideshow_step(1));
    at(&mut host, 610_000);
    assert!(
        opacity(&host, "title-2").abs() < 0.001,
        "the arriving slide starts at its first frame"
    );
    at(&mut host, 610_200);
    let mid = opacity(&host, "title-2");
    assert!(mid > 0.0 && mid < 1.0, "mid-entrance on arrival: {mid}");
    at(&mut host, 610_500);
    assert!((opacity(&host, "title-2") - 1.0).abs() < 0.001);

    // Going back plays the first slide's entrance again too.
    assert!(host.preview_slideshow_step(-1));
    at(&mut host, 610_700);
    let back = opacity(&host, "title-1");
    assert!(back > 0.0 && back < 1.0, "mid-entrance going back: {back}");
}
