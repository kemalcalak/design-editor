use super::{browser_url, display_url, is_painted, paint};
use crate::layout_scene::{NodeKind, SceneNode, SceneWidget};
use crate::widgets::canvas_annotation::with_annotations;
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::PaintCx;
use crate::Rect;

fn browser(url: Option<&str>) -> SceneNode {
    let mut node = SceneNode::leaf("b1", NodeKind::Other("browser".into()));
    node.bounds = Rect::xywh(0.0, 0.0, 390.0, 844.0);
    node.widget = Some(SceneWidget {
        kind: "browser".into(),
        value_str: url.map(str::to_string),
        ..Default::default()
    });
    node
}

fn paint_once(node: &SceneNode) -> CaptureBackend {
    let mut backend = CaptureBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        paint(&mut cx, node, node.bounds, 1.0);
    }
    backend
}

#[test]
fn browsers_stay_off_exports() {
    let node = browser(Some("https://example.com"));
    assert!(!is_painted(&node));
    let backend = paint_once(&node);
    assert!(backend.rect_fills.is_empty() && backend.texts.is_empty());
}

#[test]
fn the_canvas_paints_a_page_box_with_the_address() {
    let node = browser(Some("https://example.com/pricing/"));
    let backend = with_annotations(false, || {
        assert!(is_painted(&node));
        paint_once(&node)
    });
    assert!(backend
        .rect_fills
        .iter()
        .any(|(rect, _)| rect.size.x == 390.0));
    assert!(backend
        .texts
        .iter()
        .any(|(text, _)| text == "example.com/pricing"));
    assert!(!backend.svg_strokes.is_empty(), "the globe paints");
}

#[test]
fn a_blank_browser_shows_only_the_globe_and_long_addresses_are_cut() {
    let backend = with_annotations(true, || paint_once(&browser(None)));
    assert!(backend.texts.is_empty());
    assert!(!backend.svg_strokes.is_empty());
    assert_eq!(browser_url(&browser(None)), None);
    let long = format!("https://example.com/{}", "a".repeat(80));
    let shown = display_url(&long);
    assert_eq!(shown.chars().count(), 48);
    assert!(shown.ends_with('…'));
}

#[test]
fn only_a_canvas_paint_records_the_node_for_the_page_renderer() {
    let mut node = browser(Some("https://example.com"));
    node.id = "cb-visible".into();
    use crate::widgets::browser_textures::{begin_frame, lock_for_tests, take_visible};
    let _lock = lock_for_tests();
    let seen = || {
        take_visible()
            .unwrap_or_default()
            .contains(&"cb-visible".to_string())
    };
    begin_frame(false);
    paint_once(&node);
    assert!(!seen());
    begin_frame(false);
    with_annotations(false, || paint_once(&node));
    assert!(seen());
}

#[test]
fn a_page_that_cannot_load_says_why_under_the_address() {
    let _guard = crate::widgets::browser_textures::lock_for_tests();
    let node = browser(Some("./video/player.html"));
    crate::widgets::browser_textures::set_failure(
        &node.id,
        Some(("browser.status.failed", "Page could not load")),
    );
    let backend = with_annotations(false, || paint_once(&node));
    crate::widgets::browser_textures::set_failure(&node.id, None);
    let texts: Vec<&str> = backend.texts.iter().map(|(t, _)| t.as_str()).collect();
    assert!(texts.contains(&"./video/player.html"), "{texts:?}");
    assert!(texts.contains(&"Page could not load"), "{texts:?}");
    let backend = with_annotations(false, || paint_once(&node));
    assert_eq!(
        backend.texts.len(),
        1,
        "a cleared failure shows only the address"
    );
}

#[test]
fn a_live_frame_replaces_the_placeholder_and_records_its_box() {
    use crate::widgets::browser_textures::{forget_live, lock_for_tests, set_live_frame};
    use crate::widgets::{canvas_animation_regions, canvas_shader_pointer};
    let _lock = lock_for_tests();
    let mut node = browser(Some("https://example.com"));
    node.id = "cb-live".into();
    // The capture backend reports every raster resident.
    set_live_frame("cb-live", 3);
    canvas_shader_pointer::begin_frame(None, false);
    canvas_animation_regions::begin_frame();
    let backend = with_annotations(false, || paint_once(&node));
    let recorded = canvas_animation_regions::snapshot();
    canvas_shader_pointer::end_frame();
    forget_live("cb-live");
    assert!(backend.svg_strokes.is_empty(), "no globe over a live page");
    assert!(recorded.live);
    assert_eq!(recorded.rects, vec![node.bounds]);
    let still = with_annotations(false, || paint_once(&node));
    assert!(!still.svg_strokes.is_empty(), "the placeholder is back");
}
