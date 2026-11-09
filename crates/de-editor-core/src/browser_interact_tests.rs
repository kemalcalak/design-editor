use crate::browser_live::BrowserLayout;
use crate::command_node::build_leaf_node;
use crate::test_support::state_with;

use super::*;

/// A page with browser `b1` (address set) and rect `r1`, nothing selected.
fn page() -> EditorState {
    let browser = build_leaf_node("browser", "b1", "Browser", 10, 20, 1440, 900).expect("builds");
    let rect = build_leaf_node("rect", "r1", "Rect", 0, 0, 10, 10).expect("builds");
    let mut state = state_with(vec![browser, rect]);
    state.set_single_selection(NodeId::new("b1"));
    assert!(state.set_selected_browser_url("example.com"));
    state.set_single_selection(NodeId::NONE);
    state
}

fn b1() -> NodeId {
    NodeId::new("b1")
}

#[test]
fn entering_selects_the_node_and_makes_it_interactive() {
    let mut state = page();
    assert!(state.enter_browser_interaction(b1()));
    assert_eq!(state.interactive_browser(), Some(b1()));
    assert_eq!(state.selection.anchor, b1());
    assert_eq!(state.selection_count(), 1);
    assert!(
        !state.enter_browser_interaction(b1()),
        "already interactive"
    );
}

#[test]
fn only_a_browser_with_a_live_page_can_be_driven() {
    let mut state = page();
    assert!(!state.enter_browser_interaction(NodeId::new("r1")));
    assert!(!state.enter_browser_interaction(NodeId::new("missing")));
    state.editor_ui.browser_live.hold_pages();
    assert!(!state.enter_browser_interaction(b1()), "pages held");
    state.editor_ui.browser_live.allow_pages();
    state.editor_ui.browser_live.set_live_pages(false);
    assert!(!state.enter_browser_interaction(b1()), "live pages off");
    state.editor_ui.browser_live.set_live_pages(true);
    state.set_single_selection(b1());
    assert!(state.set_selected_browser_url(" "));
    assert!(!state.enter_browser_interaction(b1()), "no address");
    assert_eq!(state.interactive_browser(), None);
}

#[test]
fn leaving_keeps_the_selection() {
    let mut state = page();
    assert!(!state.leave_browser_interaction());
    state.enter_browser_interaction(b1());
    assert!(state.leave_browser_interaction());
    assert_eq!(state.interactive_browser(), None);
    assert_eq!(
        state.selection.anchor,
        b1(),
        "still selected, so still live"
    );
}

#[test]
fn a_selection_change_ends_the_mode() {
    let mut state = page();
    state.enter_browser_interaction(b1());
    assert!(!state.sync_browser_interaction(), "unchanged");
    state.set_single_selection(NodeId::new("r1"));
    assert!(state.sync_browser_interaction());
    assert_eq!(state.interactive_browser(), None);

    state.enter_browser_interaction(b1());
    state.toggle_selection(NodeId::new("r1"));
    assert!(
        state.sync_browser_interaction(),
        "a multi-selection ends it"
    );

    state.enter_browser_interaction(b1());
    state.set_single_selection(NodeId::NONE);
    assert!(state.sync_browser_interaction(), "deselect ends it");
}

#[test]
fn losing_the_page_ends_the_mode() {
    let mut state = page();
    state.enter_browser_interaction(b1());
    state.editor_ui.browser_live.set_live_pages(false);
    assert!(state.sync_browser_interaction());
    state.editor_ui.browser_live.set_live_pages(true);
    state.enter_browser_interaction(b1());
    state.editor_ui.browser_live.clear_nodes();
    assert!(
        !state.sync_browser_interaction(),
        "a new document already cleared it"
    );
}

#[test]
fn the_hit_path_names_its_deepest_browser() {
    let state = page();
    assert_eq!(browser_in_hit_path(&state, &[b1()]), Some(b1()));
    assert_eq!(browser_in_hit_path(&state, &[NodeId::new("r1")]), None);
    let (id, layout) = {
        let mut s = state.clone();
        s.enter_browser_interaction(b1());
        s.interactive_browser_layout().expect("layout")
    };
    assert_eq!(id, b1());
    assert_eq!((layout.width, layout.height), (1440, 900));
}

fn layout(width: u32, height: u32) -> BrowserLayout {
    BrowserLayout {
        width,
        height,
        scale: 2.0,
        mobile: false,
    }
}

#[test]
fn a_node_at_its_page_size_maps_one_to_one() {
    let map = PageMapping::new([10.0, 20.0, 1440.0, 900.0], &layout(1440, 900)).unwrap();
    assert_eq!(map.page_point(10.0, 20.0), (0.0, 0.0));
    assert_eq!(map.page_point(110.0, 70.0), (100.0, 50.0));
    assert!(map.contains(1450.0, 920.0));
    assert!(!map.contains(9.0, 20.0));
}

#[test]
fn a_resized_node_scales_into_page_px() {
    // A phone page (390 CSS px) shown in a 195 px box: 2 CSS px per doc px.
    let map = PageMapping::new([0.0, 0.0, 195.0, 422.0], &layout(390, 844)).unwrap();
    assert_eq!(map.page_point(97.5, 211.0), (195.0, 422.0));
    // A zoomed page (layout halved by the node's own zoom 2×).
    let zoomed = PageMapping::new([0.0, 0.0, 1440.0, 900.0], &layout(720, 450)).unwrap();
    assert_eq!(zoomed.page_point(144.0, 90.0), (72.0, 45.0));
}

#[test]
fn canvas_zoom_reaches_the_page_through_the_document_point() {
    // The host turns screen into document px through the viewport; at any
    // canvas zoom the same document point maps to the same page point.
    let map = PageMapping::new([100.0, 100.0, 800.0, 600.0], &layout(800, 600)).unwrap();
    for zoom in [0.25_f32, 0.5, 1.0, 2.0, 4.0] {
        let viewport = crate::Viewport {
            pan_x: 30.0,
            pan_y: -40.0,
            zoom,
        };
        // The page point (200, 150) is the document point (300, 250).
        // Document (300, 250) on screen at this pan and zoom.
        let screen = crate::Point2D::new(300.0 * zoom + 30.0, 250.0 * zoom - 40.0);
        let doc = viewport.to_document(screen);
        let (x, y) = map.page_point(doc.x, doc.y);
        assert!(
            (x - 200.0).abs() < 1e-3 && (y - 150.0).abs() < 1e-3,
            "zoom {zoom}: {x},{y}"
        );
    }
}

#[test]
fn a_scroll_follows_the_pointer_at_any_zoom() {
    let map = PageMapping::new([0.0, 0.0, 720.0, 450.0], &layout(1440, 900)).unwrap();
    // Content moved up 100 screen px at zoom 1: the page scrolls down
    // 200 CSS px (2 CSS px per doc px).
    assert_eq!(map.page_scroll(0.0, -100.0, 1.0), (0.0, 200.0));
    assert_eq!(map.page_scroll(50.0, 0.0, 2.0), (-50.0, 0.0));
    assert_eq!(
        map.page_scroll(0.0, -10.0, 0.0),
        (0.0, 20.0),
        "a broken zoom counts as 1"
    );
}

#[test]
fn an_empty_box_has_no_mapping() {
    assert!(PageMapping::new([0.0, 0.0, 0.0, 10.0], &layout(10, 10)).is_none());
    assert!(PageMapping::new([f32::NAN, 0.0, 10.0, 10.0], &layout(10, 10)).is_none());
}

#[test]
fn opening_the_preview_ends_the_canvas_mode() {
    let mut state = page();
    state.enter_browser_interaction(b1());
    state.editor_ui.preview.mode = true;
    assert!(state.sync_browser_interaction());
    assert_eq!(state.interactive_browser(), None);
}

#[test]
fn the_preview_drives_every_page_it_may_load() {
    let mut state = page();
    assert_eq!(state.preview_page_layout(&b1()), None, "not previewing");
    state.editor_ui.preview.mode = true;
    let layout = state.preview_page_layout(&b1()).expect("drivable");
    assert_eq!((layout.width, layout.height), (1440, 900));
    assert_eq!(state.preview_page_layout(&NodeId::new("r1")), None);
    state.editor_ui.browser_live.set_live_pages(false);
    assert_eq!(state.preview_page_layout(&b1()), None, "live pages off");
}
