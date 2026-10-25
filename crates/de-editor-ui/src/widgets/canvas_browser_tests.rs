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
