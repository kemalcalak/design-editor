use super::*;
use crate::widgets::test_capture_backend::CaptureBackend;
use de_editor_core::LeftPanelTab;

const DOC: &str = r#"{"version":"1.0.0","children":[
  {"type":"image","id":"a","name":"Photo","src":"","width":10,"height":10},
  {"type":"image","id":"b","name":"Banner","src":"","width":10,"height":10},
  {"type":"image","id":"c","src":"","width":10,"height":10},
  {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10}
]}"#;

fn state() -> EditorState {
    let doc = jian_ops_schema::load_str(DOC)
        .expect("fixture parses")
        .value;
    let mut state = EditorState::from_document(doc);
    state.editor_ui.sidebar_open = true;
    state.editor_ui.slides_panel.tab = LeftPanelTab::Images;
    state
}

fn body() -> Rect {
    Rect::xywh(0.0, 80.0, 320.0, 300.0)
}

fn centre(r: Rect) -> Point2D {
    Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0)
}

#[test]
fn hit_test_maps_buttons_and_cells() {
    let s = state();
    let l = layout(body(), 3, 0.0);
    assert_eq!(
        hit_test(&s, body(), centre(l.search)),
        Some(ImagesTabHit::Button(ImagesTabAction::Search))
    );
    assert_eq!(
        hit_test(&s, body(), centre(l.generate)),
        Some(ImagesTabHit::Button(ImagesTabAction::Generate))
    );
    assert_eq!(
        hit_test(&s, body(), centre(l.cells[1])),
        Some(ImagesTabHit::Image(NodeId::new("b")))
    );
    // Gutter between the two columns is the tab's, not the canvas's.
    let gutter = Point2D::new(
        l.cells[0].origin.x + l.cells[0].size.x + 4.0,
        centre(l.cells[0]).y,
    );
    assert_eq!(hit_test(&s, body(), gutter), Some(ImagesTabHit::Inside));
}

#[test]
fn only_the_images_tab_hit_tests() {
    let mut s = state();
    s.editor_ui.slides_panel.tab = LeftPanelTab::Layers;
    let l = layout(body(), 3, 0.0);
    assert_eq!(hit_test(&s, body(), centre(l.search)), None);
}

#[test]
fn the_grid_scrolls_within_its_bound() {
    // Three images in two columns need two rows; a short body scrolls.
    let short = Rect::xywh(0.0, 0.0, 320.0, 200.0);
    let max = max_scroll(short, 3);
    assert!(max > 0.0);
    let top = layout(short, 3, 0.0).cells[0].origin.y;
    let scrolled = layout(short, 3, 10_000.0).cells[0].origin.y;
    assert_eq!(top - scrolled, max, "the offset clamps to the bound");
    assert_eq!(max_scroll(body(), 0), 0.0);
}

#[test]
fn a_cell_press_selects_and_asks_to_frame() {
    let mut s = state();
    let press = apply_hit(&mut s, ImagesTabHit::Image(NodeId::new("c")), 1);
    assert_eq!(press, Some(NodeId::new("c")));
    assert_eq!(s.selection.anchor.as_str(), "c");
}

#[test]
fn paint_names_every_cell_and_falls_back_to_the_id() {
    let s = state();
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint(&mut cx, &s, Rect::xywh(0.0, 80.0, 320.0, 800.0));
    let texts: Vec<_> = backend.texts.iter().map(|(t, _)| t.as_str()).collect();
    for want in ["Photo", "Banner", "c"] {
        assert!(texts.contains(&want), "{want} painted in {texts:?}");
    }
}

#[test]
fn an_empty_page_says_so() {
    let mut s = EditorState::default();
    s.editor_ui.sidebar_open = true;
    s.editor_ui.slides_panel.tab = LeftPanelTab::Images;
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint(&mut cx, &s, body());
    let empty = translate(&s.editor_ui, "imagesTab.empty");
    assert!(backend.texts.iter().any(|(t, _)| t == empty));
}
