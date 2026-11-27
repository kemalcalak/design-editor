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
    // Ends that repeat the cap build no geometry: the cheap patch holds.
    seed(
        &mut host,
        &line_doc(r#","cap":"round","strokeStart":"round","strokeEnd":"round""#),
    );
    assert!(host.bounds_patch_is_safe(&NodeId::new("l")));
    seed(
        &mut host,
        &line_doc("").replace(r#""x2":120"#, r#""x2":-120"#),
    );
    assert!(!host.bounds_patch_is_safe(&NodeId::new("l")));
}

fn shape_doc(node: &str) -> String {
    format!(r#"{{"version":"1.0.0","children":[{node}]}}"#)
}

/// Every box shape patches: a smoothed box or a rounded polygon gets its
/// outline rebuilt alone for the new box instead of a scene rebuild.
#[test]
fn corner_outlines_patch_like_plain_corners() {
    let mut host = WidgetHostNative::new();
    let id = NodeId::new("s");
    for (node, safe) in [
        (
            r#"{"type":"rectangle","id":"s","width":80,"height":60,"cornerRadius":12}"#,
            true,
        ),
        (
            r#"{"type":"rectangle","id":"s","width":80,"height":60,"cornerRadius":12,"cornerSmoothing":0.6}"#,
            true,
        ),
        (
            r#"{"type":"rectangle","id":"s","width":80,"height":60,"cornerSmoothing":0.6}"#,
            true,
        ),
        (
            r#"{"type":"frame","id":"s","width":80,"height":60,"cornerRadius":[0,8,0,0],"cornerSmoothing":1}"#,
            true,
        ),
        (
            r#"{"type":"polygon","id":"s","width":80,"height":60,"polygonCount":5}"#,
            true,
        ),
        (
            r#"{"type":"polygon","id":"s","width":80,"height":60,"polygonCount":5,"cornerRadius":6}"#,
            true,
        ),
    ] {
        seed(&mut host, &shape_doc(node));
        assert_eq!(host.bounds_patch_is_safe(&id), safe, "{node}");
    }
}

/// While a smoothed rect is resized, the outline the canvas paints
/// follows the box on every frame, through the cheap patch.
#[test]
fn a_smoothed_rect_outline_follows_a_live_resize() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        &shape_doc(
            r#"{"type":"rectangle","id":"l","x":100,"y":80,"width":120,"height":40,
                "cornerRadius":10,"cornerSmoothing":0.6}"#,
        ),
    );
    start_resize(&mut host);
    for (cursor_x, width) in [(520.0, 140.0), (540.0, 160.0)] {
        assert!(host.apply_cursor_move(cursor_x, 500.0));
        assert!(!host.editor_state_dirty, "patched, not rebuilt");
        let scene = host.layout_scene();
        let rect = scene
            .active_page()
            .and_then(|page| page.find("l"))
            .expect("scene rect");
        assert_eq!(rect.bounds.size.x, width);
        let outline = rect.corner_path.as_deref().expect("outline");
        assert!(
            (outline.bounds.size.x - width).abs() < 0.01,
            "{cursor_x}: outline {:?}",
            outline.bounds
        );
    }
}

/// The outline a patch rebuilds is the one a full scene build makes.
#[test]
fn a_patched_outline_matches_a_rebuilt_one() {
    let mut host = WidgetHostNative::new();
    let star = r##"{"type":"polygon","id":"l","x":100,"y":80,"width":120,"height":40,
        "polygonCount":5,"innerRadius":0.5,"cornerRadius":6,"cornerSmoothing":0.4,
        "stroke":{"thickness":4,"align":"inside","fill":[{"type":"solid","color":"#000000"}]}}"##;
    seed(&mut host, &shape_doc(star));
    start_resize(&mut host);
    assert!(host.apply_cursor_move(540.0, 500.0));
    let patched = host
        .layout_scene()
        .active_page()
        .and_then(|page| page.find("l"))
        .and_then(|n| n.corner_path.clone())
        .expect("patched outline");
    host.invalidate_live_scene_for_rebuild();
    host.refresh_layout_scene();
    let rebuilt = host
        .layout_scene()
        .active_page()
        .and_then(|page| page.find("l"))
        .and_then(|n| n.corner_path.clone())
        .expect("rebuilt outline");
    assert_eq!(patched.d, rebuilt.d);
    assert_eq!(patched.bounds, rebuilt.bounds);
}
