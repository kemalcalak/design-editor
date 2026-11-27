//! Shift / Alt while drawing a shape (#389), through the host's create
//! drag: the geometry is `de_editor_core::draw_constraints`; these check
//! the wiring, a mid-drag modifier change and the line's direction.

use super::{CreateDragState, WidgetHostNative};
use de_editor_core::{NodeId, Tool};
use de_editor_ui::Point2D;
use jian_ops_schema::node::PenNode;

/// Start drawing with `tool` at `start` (doc px) and return the new id.
fn begin(host: &mut WidgetHostNative, tool: Tool, start: Point2D) -> NodeId {
    host.editor_state_mut().tool = tool;
    let id = host
        .create_node_for_active_tool(start)
        .expect("the tool creates a node");
    host.editor_state_mut().set_single_selection(id.clone());
    host.create_drag = Some(CreateDragState {
        start_doc_x: start.x,
        start_doc_y: start.y,
        cursor_doc: None,
    });
    id
}

fn move_to(host: &mut WidgetHostNative, doc: Point2D) {
    let (cx, cy) = de_editor_ui::widgets::host_canvas_geometry::canvas_origin(host.editor_state());
    assert!(host.apply_cursor_move(cx + doc.x, cy + doc.y));
}

fn scene_box(host: &mut WidgetHostNative, id: &NodeId) -> (f32, f32, f32, f32) {
    let node = host
        .layout_scene()
        .active_page()
        .and_then(|page| page.find(id.as_str()))
        .expect("scene node");
    let b = node.bounds;
    (b.origin.x, b.origin.y, b.size.x, b.size.y)
}

fn line(host: &WidgetHostNative, id: &NodeId) -> (f64, f64, f64, f64) {
    let node = de_editor_core::walkers::find_node(host.editor_state().active_children(), id)
        .expect("line node");
    let PenNode::Line(n) = node else {
        panic!("not a line: {node:?}");
    };
    (
        n.base.x.unwrap_or(0.0),
        n.base.y.unwrap_or(0.0),
        n.x2.unwrap_or(0.0),
        n.y2.unwrap_or(0.0),
    )
}

#[test]
fn shift_squares_and_alt_centres_a_rectangle_even_mid_drag() {
    let mut host = WidgetHostNative::new();
    let id = begin(&mut host, Tool::Rect, Point2D::new(200.0, 200.0));
    move_to(&mut host, Point2D::new(260.0, 220.0));
    assert_eq!(scene_box(&mut host, &id), (200.0, 200.0, 60.0, 20.0));
    // Pressing Shift without moving squares it at once.
    assert!(host.set_modifier_shift(true));
    host.refresh_layout_scene();
    assert_eq!(scene_box(&mut host, &id), (200.0, 200.0, 60.0, 60.0));
    // Alt on top grows it about the press point.
    assert!(host.set_modifier_alt(true));
    host.refresh_layout_scene();
    assert_eq!(scene_box(&mut host, &id), (140.0, 140.0, 120.0, 120.0));
    // Letting go of both goes back to corner to cursor.
    assert!(host.set_modifier_shift(false));
    assert!(host.set_modifier_alt(false));
    host.refresh_layout_scene();
    assert_eq!(scene_box(&mut host, &id), (200.0, 200.0, 60.0, 20.0));
}

#[test]
fn an_ellipse_with_shift_is_a_circle_and_a_frame_a_square() {
    for tool in [Tool::Ellipse, Tool::Frame, Tool::Polygon, Tool::Star] {
        let mut host = WidgetHostNative::new();
        host.set_modifier_shift(true);
        let id = begin(&mut host, tool, Point2D::new(100.0, 100.0));
        move_to(&mut host, Point2D::new(80.0, 150.0));
        assert_eq!(scene_box(&mut host, &id), (50.0, 100.0, 50.0, 50.0), "{tool:?}");
    }
}

#[test]
fn a_drawn_line_follows_the_cursor_and_keeps_its_direction() {
    let mut host = WidgetHostNative::new();
    let id = begin(&mut host, Tool::Line, Point2D::new(100.0, 200.0));
    // Up and to the right: the start stays where the press was.
    move_to(&mut host, Point2D::new(160.0, 170.0));
    assert_eq!(line(&host, &id), (100.0, 200.0, 60.0, -30.0));
    host.refresh_layout_scene();
    let node = host
        .layout_scene()
        .active_page()
        .and_then(|page| page.find(id.as_str()))
        .expect("scene line");
    let (from, to) = node.line_endpoints();
    assert_eq!((from.x, from.y, to.x, to.y), (100.0, 200.0, 160.0, 170.0));
    // Shift snaps it flat, Alt runs it through the press point.
    host.set_modifier_shift(true);
    move_to(&mut host, Point2D::new(160.0, 190.0));
    assert_eq!(line(&host, &id), (100.0, 200.0, 60.0, 0.0));
    host.set_modifier_alt(true);
    assert_eq!(line(&host, &id), (40.0, 200.0, 120.0, 0.0));
}

#[test]
fn a_modifier_change_without_a_drag_needs_no_repaint() {
    let mut host = WidgetHostNative::new();
    assert!(!host.set_modifier_shift(true));
    assert!(!host.set_modifier_shift(true));
}
