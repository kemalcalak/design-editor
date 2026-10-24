use super::{annotation_body, is_annotation, paint, palette, with_annotations};
use crate::layout_scene::{NodeKind, SceneNode, SceneWidget};
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

fn annotation(tag: &str, text: &str) -> SceneNode {
    let mut node = SceneNode::leaf("n1", NodeKind::Other(tag.into()));
    node.bounds = Rect::xywh(100.0, 50.0, 240.0, 160.0);
    node.text = Some(text.into());
    node.font_size = 14.0;
    node
}

fn paint_once(node: &SceneNode) -> (bool, CaptureBackend) {
    let mut backend = CaptureBackend::default();
    let painted = {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        paint(&mut cx, node, node.bounds, Point2D::ZERO, 1.0, &None)
    };
    (painted, backend)
}

#[test]
fn annotations_stay_off_exports() {
    let (painted, backend) = paint_once(&annotation("note", "Hello"));
    assert!(!painted, "outside the editor canvas nothing paints");
    assert!(backend.round_fills.is_empty());
    assert!(backend.texts.is_empty());
}

#[test]
fn a_note_paints_its_card_and_content_on_the_canvas() {
    let (painted, backend) = with_annotations(false, || paint_once(&annotation("note", "Hello")));
    assert!(painted);
    let paper = palette("note", false).paper;
    assert!(backend
        .round_fills
        .iter()
        .any(|(rect, _, color)| *color == paper && rect.size.x == 240.0));
    assert!(backend.texts.iter().any(|(text, _)| text.contains("Hello")));
    assert!(!backend.svg_strokes.is_empty(), "the kind glyph paints");
}

#[test]
fn a_prompt_shows_its_model_and_dark_mode_changes_the_paper() {
    let mut node = annotation("prompt", "Write the hero copy");
    node.widget = Some(SceneWidget {
        kind: "prompt".into(),
        label: Some("claude-opus".into()),
        ..Default::default()
    });
    let (_, light) = with_annotations(false, || paint_once(&node));
    let (_, dark) = with_annotations(true, || paint_once(&node));
    assert!(light.texts.iter().any(|(text, _)| text == "claude-opus"));
    let light_paper = palette("prompt", false).paper;
    let dark_paper = palette("prompt", true).paper;
    assert_ne!(light_paper, dark_paper);
    assert!(dark.round_fills.iter().any(|(_, _, c)| *c == dark_paper));
}

#[test]
fn the_body_sits_inside_the_card_under_the_header() {
    let node = annotation("context", "Brand voice");
    let body = annotation_body(&node).expect("annotation body");
    assert_eq!(body.kind, NodeKind::Text);
    assert_eq!(body.id, node.id);
    assert!(body.text_wrap);
    assert!(body.bounds.origin.x > node.bounds.origin.x);
    assert!(body.bounds.origin.y > node.bounds.origin.y);
    assert!(body.bounds.origin.x + body.bounds.size.x < node.bounds.origin.x + node.bounds.size.x);
    assert!(is_annotation(&node));
    assert!(annotation_body(&SceneNode::leaf("t", NodeKind::Text)).is_none());
}
