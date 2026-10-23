use super::*;
use crate::widgets::test_capture_backend::CaptureBackend;

fn panel() -> Rect {
    Rect::xywh(1168.0, 0.0, 272.0, 900.0)
}

fn centre(r: Rect) -> Point2D {
    Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
}

#[test]
fn only_the_code_view_close_button_hits() {
    let close = centre(close_rect(panel()));
    assert_eq!(
        header_hit(PropertyTab::Code, panel(), close),
        Some(PropertyTab::Design)
    );
    assert_eq!(header_hit(PropertyTab::Design, panel(), close), None);
    let title = Point2D::new(panel().origin.x + 30.0, 20.0);
    assert_eq!(header_hit(PropertyTab::Code, panel(), title), None);
}

#[test]
fn the_close_button_sits_inside_the_band() {
    let band = header_rect(panel());
    let close = close_rect(panel());
    assert!(close.origin.y >= band.origin.y);
    assert!(close.origin.y + close.size.y <= band.origin.y + band.size.y);
    assert!(close.origin.x + close.size.x <= band.origin.x + band.size.x);
}

#[test]
fn a_node_header_paints_name_and_kind() {
    let theme = Theme::dark();
    let header = PanelHeader::Node {
        icon: Icon::Hash,
        accent: theme.primary,
        title: "Pricing",
        kind: "Frame",
        create_component: false,
    };
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_panel_header(&mut cx, &theme, panel(), &header, false);
    let texts: Vec<_> = backend.texts.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(texts, ["Frame", "Pricing"]);
    assert!(!backend.svg_strokes.is_empty(), "the kind glyph paints");
}
