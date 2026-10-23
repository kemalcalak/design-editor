//! Component-browser panel unit tests — split out of
//! `component_browser_panel.rs` to keep that module under the
//! 800-line ceiling. A `#[path]` submodule so it retains `super::`
//! access to the parent's private items (`filtered_call_count`, etc.).

use super::*;
use crate::RenderBackend;

#[derive(Default)]
struct NoopBackend;

impl RenderBackend for NoopBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
    fn stroke_rect(&mut self, _rect: Rect, _color: Color, _width: f32) {}
    fn draw_text(&mut self, _layout: &TextLayout, _origin: Point2D) {}
    fn clip_rect(&mut self, _rect: Rect) {}
    fn stroke_line(&mut self, _from: Point2D, _to: Point2D, _color: Color, _width: f32) {}
    fn fill_round_rect(&mut self, _rect: Rect, _radius: f32, _color: Color) {}
    fn stroke_round_rect(&mut self, _rect: Rect, _radius: f32, _color: Color, _width: f32) {}
    fn stroke_svg_path(
        &mut self,
        _d: &str,
        _top_left: Point2D,
        _size: f32,
        _color: Color,
        _width: f32,
    ) {
    }
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _offset: Point2D) {}
    fn resize(&mut self, _width: u32, _height: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

/// A rail-sized body: the Components tab fills the left rail (#30).
const RAIL_W: f32 = 320.0;
const RAIL_H: f32 = 760.0;

fn open_state() -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.open_components_tab();
    state
}

fn rect() -> Rect {
    Rect::xywh(0.0, 0.0, RAIL_W, RAIL_H)
}

/// Before this fix, `hit_test` called `self.filtered()` once
/// directly AND again indirectly through `card_rects`. It must now
/// filter the kit set exactly once per call.
#[test]
fn hit_test_filters_the_kit_set_exactly_once() {
    let state = open_state();
    let panel = ComponentBrowserPanel::for_editor(&state).expect("panel open");
    let before = filtered_call_count();

    // A point well inside the card-grid area (below the header, kit
    // strip, pills, and search row) so `hit_test` walks into the
    // branch that filters the kit set — a header click resolves to
    // `Inside` and never reaches `filtered()` at all.
    let _ = panel.hit_test(
        rect(),
        Point2D::new(
            rect().origin.x + RAIL_W / 2.0,
            rect().origin.y + RAIL_H - 40.0,
        ),
    );

    assert_eq!(
        filtered_call_count(),
        before + 1,
        "a single hit_test pass must filter the kit set exactly once, not twice"
    );
}

/// Before this fix, `paint` called `self.filtered()` once directly
/// (for the empty-state check) AND again indirectly through
/// `card_rects` for the grid layout. It must now filter the kit set
/// exactly once per call.
#[test]
fn paint_filters_the_kit_set_exactly_once() {
    let state = open_state();
    let panel = ComponentBrowserPanel::for_editor(&state).expect("panel open");
    let before = filtered_call_count();
    let mut backend = NoopBackend;
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect());

    assert_eq!(
        filtered_call_count(),
        before + 1,
        "a single paint pass must filter the kit set exactly once, not twice"
    );
}

#[test]
fn the_tab_only_builds_while_the_rail_shows_it() {
    let mut state = EditorState::default();
    assert!(ComponentBrowserPanel::for_editor(&state).is_none());
    state.editor_ui.open_components_tab();
    assert!(ComponentBrowserPanel::for_editor(&state).is_some());
    state.editor_ui.sidebar_open = false;
    assert!(
        ComponentBrowserPanel::for_editor(&state).is_none(),
        "a shut rail shows no tab"
    );
}

#[test]
fn the_search_box_hit_asks_for_focus_and_the_header_never_drags() {
    let state = open_state();
    let panel = ComponentBrowserPanel::for_editor(&state).expect("tab on show");
    let search = panel.search_rect(rect());
    assert_eq!(
        panel.hit_test(
            rect(),
            Point2D::new(
                search.origin.x + search.size.x / 2.0,
                search.origin.y + search.size.y / 2.0
            )
        ),
        Some(ComponentBrowserHit::FocusSearch)
    );
    assert_eq!(
        panel.hit_test(rect(), Point2D::new(20.0, HEADER_H / 2.0)),
        Some(ComponentBrowserHit::Inside)
    );
}

#[test]
fn the_card_grid_scrolls_and_clamps() {
    let mut state = open_state();
    let short = Rect::xywh(0.0, 0.0, RAIL_W, 360.0);
    let max = ComponentBrowserPanel::for_editor(&state)
        .expect("tab on show")
        .current_max_scroll(short);
    assert!(max > 0.0, "the built-in kit overflows a short rail");
    let point = Point2D::new(RAIL_W / 2.0, 340.0);
    assert_eq!(
        crate::widgets::scroll_flow::scroll_component_browser(&mut state, short, point, -10_000.0),
        Some(true)
    );
    assert!((state.editor_ui.component_browser_scroll.offset - max).abs() < 0.01);
    // A pointer off the tab leaves the wheel to someone else.
    assert_eq!(
        crate::widgets::scroll_flow::scroll_component_browser(
            &mut state,
            short,
            Point2D::new(RAIL_W + 10.0, 100.0),
            10.0
        ),
        None
    );
}

#[test]
fn pills_wrap_inside_the_rail() {
    let state = open_state();
    let panel = ComponentBrowserPanel::for_editor(&state).expect("tab on show");
    for (pill, _) in panel.pill_rects(rect()) {
        assert!(
            pill.origin.x + pill.size.x <= RAIL_W - PAD + 0.01,
            "{pill:?}"
        );
    }
}
