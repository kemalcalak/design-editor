use super::*;
use crate::layout_scene::{NodeKind, SceneNode, ScenePage};
use crate::widgets::test_capture_backend::CaptureBackend;
use de_editor_core::NodeId;

fn node(id: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, NodeKind::Frame);
    n.bounds = Rect::xywh(x, y, w, h);
    n
}

/// A 200x100 frame `f` holding `a` (20,30 40x20) and `b` (100,30 50x20).
fn scene() -> LayoutScene {
    let mut f = node("f", 0.0, 0.0, 200.0, 100.0);
    f.children = vec![
        node("a", 20.0, 30.0, 40.0, 20.0),
        node("b", 100.0, 30.0, 50.0, 20.0),
    ];
    LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "P".into(),
            children: vec![f],
        }],
        active_page_index: 0,
    }
}

fn state(selected: &str, hovered: Option<&str>, alt: bool) -> EditorState {
    let mut s = EditorState::starter();
    s.tool = Tool::Select;
    s.selection.anchor = NodeId::new(selected);
    s.selection.set = vec![NodeId::new(selected)];
    s.editor_ui.canvas_hover_node = hovered.map(NodeId::new);
    s.editor_ui.canvas_view.measure_modifier = alt;
    s
}

fn labels(state: &EditorState) -> Option<Vec<String>> {
    MeasureOverlay::build(state, &scene()).map(|m| m.labels().map(str::to_string).collect())
}

#[test]
fn nothing_without_alt() {
    assert!(labels(&state("a", Some("b"), false)).is_none());
}

#[test]
fn sibling_gap_is_measured() {
    assert_eq!(
        labels(&state("a", Some("b"), true)),
        Some(vec!["40".into()])
    );
}

#[test]
fn hovering_the_parent_shows_the_padding() {
    assert_eq!(
        labels(&state("a", Some("f"), true)),
        Some(vec!["20".into(), "140".into(), "30".into(), "50".into()])
    );
}

#[test]
fn no_hover_or_hovering_the_selection_measures_nothing() {
    assert!(labels(&state("a", None, true)).is_none());
    assert!(labels(&state("a", Some("a"), true)).is_none());
}

#[test]
fn other_tools_do_not_measure() {
    let mut s = state("a", Some("b"), true);
    s.tool = Tool::Rect;
    assert!(MeasureOverlay::build(&s, &scene()).is_none());
}

#[test]
fn a_steady_hover_reuses_the_built_overlay() {
    let s = state("a", Some("b"), true);
    let first = MeasureOverlay::build(&s, &scene()).unwrap();
    let second = MeasureOverlay::build(&s, &scene()).unwrap();
    assert!(Rc::ptr_eq(&first, &second));
}

#[test]
fn paint_draws_a_line_and_a_value_pill() {
    let overlay = MeasureOverlay::build(&state("a", Some("b"), true), &scene()).unwrap();
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    let viewport = DocViewport {
        pan_x: 0.0,
        pan_y: 0.0,
        zoom: 1.0,
    };
    overlay.paint(&mut cx, Rect::xywh(0.0, 0.0, 800.0, 600.0), &viewport);
    assert_eq!(backend.lines, 1);
    assert_eq!(backend.round_fills.len(), 1);
    assert_eq!(backend.texts.len(), 1);
    assert_eq!(backend.texts[0].0, "40");
}
