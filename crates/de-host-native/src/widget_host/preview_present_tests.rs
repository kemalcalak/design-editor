//! Present from any page (#157): Cmd/Ctrl+Enter presents a document that
//! is not tagged as a deck, toggles back out, and switches over from the
//! interactive preview.

use super::super::WidgetHostNative;
use de_editor_core::EditorState;
use de_editor_ui::{Color, ImageBlendMode, Point2D, Rect, RenderBackend, TextLayout};
use std::sync::{LazyLock, Mutex, MutexGuard};

static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn test_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

/// Two app screens with no scenario tag: an ordinary design, not a deck.
const TWO_SCREENS: &str = r##"{
    "version": "1.0.0",
    "children": [
        { "type": "frame", "id": "home", "x": 0, "y": 0,
          "width": 390, "height": 844,
          "fill": [{"type":"solid","color":"#ffffff"}], "children": [] },
        { "type": "rectangle", "id": "note", "x": 0, "y": 900,
          "width": 40, "height": 40 },
        { "type": "frame", "id": "detail", "x": 500, "y": 0,
          "width": 390, "height": 844,
          "fill": [{"type":"solid","color":"#eeeeee"}], "children": [] }
    ]
}"##;

fn host_with(source: &str) -> WidgetHostNative {
    let document = jian_ops_schema::load_str(source)
        .expect("parse present fixture")
        .value;
    let mut host = WidgetHostNative::new();
    host.install_imported_state(EditorState::from_document(document));
    host
}

fn board_on_screen(host: &WidgetHostNative) -> Option<String> {
    host.editor_state
        .preview_slideshow()
        .and_then(|slideshow| slideshow.current_board())
        .map(str::to_string)
}

#[test]
fn present_runs_any_page_as_slides_and_toggles_off() {
    let _guard = test_lock();
    let mut host = host_with(TWO_SCREENS);

    assert!(host.toggle_present((1200.0, 800.0)));
    assert!(host.preview_slideshow_active());
    assert_eq!(board_on_screen(&host).as_deref(), Some("home"));
    assert_eq!(
        host.editor_state.preview_slideshow().map(|s| s.len()),
        Some(2),
        "only top-level frames are slides"
    );
    assert!(host.preview_slideshow_step(1));
    assert_eq!(board_on_screen(&host).as_deref(), Some("detail"));

    assert!(host.toggle_present((1200.0, 800.0)));
    assert!(!host.preview_active());
    assert!(!host.editor_state.editor_ui.preview.present);

    // Plain Preview of the same document is still the interactive one.
    assert!(host.enter_preview((1200.0, 800.0)));
    assert!(!host.preview_slideshow_active());
}

#[test]
fn present_switches_over_from_the_interactive_preview() {
    let _guard = test_lock();
    let mut host = host_with(TWO_SCREENS);
    assert!(host.enter_preview((1200.0, 800.0)));
    assert!(!host.preview_slideshow_active());

    assert!(host.toggle_present((1200.0, 800.0)));
    assert!(host.preview_active());
    assert!(host.preview_slideshow_active());
}

#[test]
fn a_page_without_frames_does_not_present() {
    let _guard = test_lock();
    let mut host = host_with(
        r#"{"version":"1.0.0","children":[
            {"type":"rectangle","id":"r","width":10,"height":10}]}"#,
    );
    assert!(!host.toggle_present((1200.0, 800.0)));
    assert!(!host.preview_active());
    assert!(!host.editor_state.editor_ui.preview.present);
}

/// Records the opacity of every composite layer a paint opens.
#[derive(Default)]
struct FadeCaptureBackend {
    composite_alphas: Vec<f32>,
}

impl RenderBackend for FadeCaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, _: &TextLayout, _: Point2D) {}
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn push_composite_layer(&mut self, _: Rect, opacity: f32, _: ImageBlendMode) {
        self.composite_alphas.push(opacity);
    }
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

/// #158: next plays the arriving slide's fade — both slides paint, the
/// arriving one at the eased opacity — and the run clears once it ends.
#[test]
fn a_fade_transition_paints_both_slides_until_it_ends() {
    let _guard = test_lock();
    let mut host = host_with(&TWO_SCREENS.replace(
        r#""id": "detail","#,
        r#""id": "detail", "slideTransition": {"kind":"fade","durationMs":400,"easing":"linear"},"#,
    ));
    host.set_now_ms(10_000);
    assert!(host.toggle_present((1200.0, 800.0)));
    assert!(host.preview_slideshow_step(1));
    let run = host
        .editor_state
        .editor_ui
        .preview
        .slide_transition
        .clone()
        .expect("the fade runs");
    assert_eq!(run.from_board, "home");
    assert!(run.forward);

    host.set_now_ms(10_200);
    let canvas = Rect::xywh(0.0, 0.0, 1200.0, 800.0);
    let mut backend = FadeCaptureBackend::default();
    host.paint_slideshow(&mut backend, canvas);
    assert!(
        backend
            .composite_alphas
            .iter()
            .any(|alpha| (alpha - 0.5).abs() < 0.01),
        "the arriving slide is half faded in: {:?}",
        backend.composite_alphas
    );

    host.set_now_ms(10_500);
    host.frame_slideshow_board((1200.0, 800.0));
    assert!(host
        .editor_state
        .editor_ui
        .preview
        .slide_transition
        .is_none());
}
