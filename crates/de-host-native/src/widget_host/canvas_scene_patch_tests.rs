//! Live bounds patches and the scene data built from a node's box (#389).

use super::super::{HandleDragState, WidgetHostNative};
use de_editor_core::NodeId;
use de_editor_ui::{widgets::SelectionHandle, Rect};

fn seed(host: &mut WidgetHostNative, json: &str) {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
}

fn line_doc(stroke: &str) -> String {
    format!(
        r##"{{"version":"1.0.0","children":[{{
          "type":"line","id":"l","name":"l","x":100,"y":80,"x2":120,"y2":40,
          "stroke":{{"thickness":2,"fill":[{{"type":"solid","color":"#000000"}}]{stroke}}}
        }}]}}"##
    )
}

fn start_resize(host: &mut WidgetHostNative) {
    host.editor_state_mut()
        .set_single_selection(NodeId::new("l"));
    host.mark_paint_dirty_for_test();
    let _ = host.layout_scene();
    host.editor_state_mut().commit_history();
    host.handle_drag = Some(HandleDragState {
        handle: SelectionHandle::Right,
        start_screen_x: 500.0,
        start_screen_y: 500.0,
        start_bounds: Rect::xywh(100.0, 80.0, 120.0, 40.0),
        start_authored_x: Some(100.0),
        start_authored_y: Some(80.0),
    });
}

/// A live resize frame never patches a line with end shapes: its
/// geometry is node-local and fitted to the box, so a patched box would
/// leave the arrowhead where the old box ended. The scene the paint reads
/// keeps box and geometry in step on every frame.
#[test]
fn a_line_with_end_shapes_keeps_box_and_geometry_in_step_while_resizing() {
    let mut host = WidgetHostNative::new();
    seed(&mut host, &line_doc(r#","strokeEnd":"triangle_arrow""#));
    start_resize(&mut host);
    for cursor_x in [520.0, 540.0] {
        assert!(host.apply_cursor_move(cursor_x, 500.0));
        let scene = host.layout_scene();
        let line = scene
            .active_page()
            .and_then(|page| page.find("l"))
            .expect("scene line");
        let geometry = line.stroke_geometry.as_deref().expect("geometry");
        // The arrow's tip sits on the line's end: the box's far corner.
        let tip = format!("M {} {} L", line.bounds.size.x, line.bounds.size.y);
        assert!(
            geometry.fill_d.contains(&tip),
            "{cursor_x}: box {:?}, heads {}",
            line.bounds,
            geometry.fill_d
        );
    }
}

#[test]
fn only_plain_boxes_take_the_cheap_patch() {
    let mut host = WidgetHostNative::new();
    seed(&mut host, &line_doc(""));
    assert!(host.bounds_patch_is_safe(&NodeId::new("l")));
    seed(&mut host, &line_doc(r#","strokeStart":"circle""#));
    assert!(!host.bounds_patch_is_safe(&NodeId::new("l")));
    seed(
        &mut host,
        &line_doc("").replace(r#""x2":120"#, r#""x2":-120"#),
    );
    assert!(!host.bounds_patch_is_safe(&NodeId::new("l")));
}
