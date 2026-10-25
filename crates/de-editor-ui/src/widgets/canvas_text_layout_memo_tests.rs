//! Tests for the per-node text layout memo. Each `#[test]` runs on its own
//! thread, so the thread-local memo starts empty in every test.

use super::*;
use crate::layout_scene::NodeKind;
use crate::{Color, Point2D, TextLayout};

/// Counts measure calls; 10 px per char.
#[derive(Default)]
struct CountingBackend {
    measures: usize,
}

impl RenderBackend for CountingBackend {
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
    fn measure_text_weighted(&mut self, text: &str, _: f32, _: u16) -> f32 {
        self.measures += 1;
        text.chars().count() as f32 * 10.0
    }
}

fn wrapped(id: &str, content: &str) -> SceneNode {
    let mut node = SceneNode::leaf(id, NodeKind::Text);
    node.bounds = Rect::xywh(0.0, 0.0, 120.0, 40.0);
    node.text = Some(content.to_string());
    node.font_size = 20.0;
    node.text_wrap = true;
    node
}

#[test]
fn a_canvas_frame_reuses_the_layout_of_an_unchanged_node() {
    let mut backend = CountingBackend::default();
    let node = wrapped("t", "hello brave new world");
    begin_frame(Some(1));
    let first = text_layout(&mut backend, &node);
    let measured = backend.measures;
    assert!(measured > 0);
    begin_frame(Some(1));
    let second = text_layout(&mut backend, &node);
    assert_eq!(backend.measures, measured, "second frame measures nothing");
    assert!(Rc::ptr_eq(&first, &second));
    assert_eq!(first.lines, text_edit_layout(&mut backend, &node).lines);
}

#[test]
fn any_text_input_change_recomputes() {
    let mut backend = CountingBackend::default();
    let node = wrapped("t", "hello brave new world");
    begin_frame(Some(1));
    let base = text_layout(&mut backend, &node);

    type Edit = Box<dyn Fn(&mut SceneNode)>;
    let mut edits: Vec<Edit> = vec![
        Box::new(|n| n.text = Some("hello brave old world".into())),
        Box::new(|n| n.font_size = 18.0),
        Box::new(|n| n.font_weight = 700),
        Box::new(|n| n.font_family = "Inter".into()),
        Box::new(|n| n.letter_spacing = 1.0),
        Box::new(|n| n.line_height = 1.5),
        Box::new(|n| n.italic = true),
        Box::new(|n| n.text_wrap = false),
        Box::new(|n| n.bounds.size.x = 60.0),
        Box::new(|n| n.bounds.origin.x = 5.0),
        Box::new(|n| n.text_align = SceneTextAlign::Center),
    ];
    for edit in edits.drain(..) {
        let mut changed = node.clone();
        edit(&mut changed);
        let layout = text_layout(&mut backend, &changed);
        let fresh = text_edit_layout(&mut backend, &changed);
        assert_eq!(layout.lines, fresh.lines);
        assert_eq!(layout.font_size, fresh.font_size);
        assert_eq!(layout.origin, fresh.origin);
        assert_eq!(layout.text_align, fresh.text_align);
        // Back to the base node: recomputed again, never the edited one.
        let back = text_layout(&mut backend, &node);
        assert_eq!(back.lines, base.lines);
        assert_eq!(back.origin, base.origin);
    }
}

#[test]
fn a_new_font_generation_drops_every_entry() {
    let mut backend = CountingBackend::default();
    let node = wrapped("t", "hello brave new world");
    begin_frame(Some(1));
    let before = text_layout(&mut backend, &node);
    begin_frame(Some(2));
    let after = text_layout(&mut backend, &node);
    assert!(!Rc::ptr_eq(&before, &after));
}

#[test]
fn outside_an_opted_in_frame_nothing_is_memoized() {
    let mut backend = CountingBackend::default();
    let node = wrapped("t", "hello brave new world");
    begin_frame(None);
    let a = text_layout(&mut backend, &node);
    let b = text_layout(&mut backend, &node);
    assert!(!Rc::ptr_eq(&a, &b));

    begin_frame(Some(1));
    end_frame();
    let c = text_layout(&mut backend, &node);
    let d = text_layout(&mut backend, &node);
    assert!(!Rc::ptr_eq(&c, &d), "exports after the frame recompute");
}
