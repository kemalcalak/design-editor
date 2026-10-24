//! Pencil tool dispatch tests (`pencil_press.rs`, #88): a drag on the
//! canvas commits one fitted path on release, undo removes it in one step,
//! and the path takes the Select tool's anchor editing.

use super::WidgetHostNative;
use de_editor_core::Tool;
use jian_ops_schema::node::PenNode;

const VW: f32 = 1440.0;
const VH: f32 = 900.0;

fn empty_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(r#"{"version":"1.0.0","children":[]}"#)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    // The open left card floats over the canvas's left edge (#86).
    host.editor_state_mut().editor_ui.sidebar_open = false;
    host.mark_paint_dirty_for_test();
    host
}

fn screen(host: &WidgetHostNative, doc_x: f32, doc_y: f32) -> (f32, f32) {
    let (cx0, cy0, _, _) = host.canvas_region(VW, VH);
    (cx0 + doc_x, cy0 + doc_y)
}

/// Press at (300, 300), drag a half circle of radius 60, release.
fn draw_stroke(host: &mut WidgetHostNative) {
    let (px, py) = screen(host, 300.0, 300.0);
    assert!(host.apply_press(px, py, VW, VH));
    for i in 1..=60 {
        let a = std::f32::consts::PI * i as f32 / 60.0;
        let (x, y) = screen(host, 360.0 - 60.0 * a.cos(), 300.0 + 60.0 * a.sin());
        host.apply_cursor_move(x, y);
    }
    assert!(host.apply_release_with_viewport(VW, VH));
}

fn paths(host: &WidgetHostNative) -> Vec<&jian_ops_schema::node::PathNode> {
    host.editor_state()
        .active_children()
        .iter()
        .filter_map(|n| match n {
            PenNode::Path(p) => Some(p),
            _ => None,
        })
        .collect()
}

#[test]
fn a_pencil_drag_commits_one_fitted_path_and_undoes_in_one_step() {
    let mut host = empty_host();
    host.apply_set_tool(Tool::Pencil);
    let depth = host.editor_state().history.past.len();
    draw_stroke(&mut host);

    let committed = paths(&host);
    assert_eq!(committed.len(), 1, "one stroke, one path");
    let anchors = committed[0].anchors.as_ref().expect("anchors");
    assert!(
        anchors.len() >= 2 && anchors.len() < 30,
        "{} anchors",
        anchors.len()
    );
    assert!(anchors.iter().any(|a| a.handle_out.is_some()), "curved");
    assert_eq!(host.editor_state().history.past.len(), depth + 1);
    assert_eq!(
        host.editor_state().tool,
        Tool::Pencil,
        "stays on the Pencil"
    );
    assert!(!host.editor_state().pencil_stroke_active());

    assert!(host.apply_undo());
    assert!(paths(&host).is_empty(), "one undo removes the stroke");
}

#[test]
fn a_pencil_click_without_drag_adds_nothing() {
    let mut host = empty_host();
    host.apply_set_tool(Tool::Pencil);
    let depth = host.editor_state().history.past.len();
    let (px, py) = screen(&host, 300.0, 300.0);
    assert!(host.apply_press(px, py, VW, VH));
    assert!(host.apply_release_with_viewport(VW, VH));
    assert!(paths(&host).is_empty());
    assert_eq!(host.editor_state().history.past.len(), depth);
}

#[test]
fn switching_tools_mid_stroke_drops_it() {
    let mut host = empty_host();
    host.apply_set_tool(Tool::Pencil);
    let (px, py) = screen(&host, 300.0, 300.0);
    host.apply_press(px, py, VW, VH);
    host.apply_cursor_move(px + 40.0, py + 10.0);
    assert!(host.editor_state().pencil_stroke_active());
    host.apply_set_tool(Tool::Select);
    assert!(!host.editor_state().pencil_stroke_active());
    host.apply_release_with_viewport(VW, VH);
    assert!(paths(&host).is_empty());
}

#[test]
fn the_select_tool_edits_a_pencil_path_anchor() {
    let mut host = empty_host();
    host.apply_set_tool(Tool::Pencil);
    draw_stroke(&mut host);
    host.apply_set_tool(Tool::Select);
    let first = paths(&host)[0].anchors.as_ref().unwrap()[0].clone();
    // Grab the first anchor (still selected after the stroke) and move it.
    let (ax, ay) = screen(&host, first.x as f32, first.y as f32);
    assert!(host.apply_press(ax, ay, VW, VH));
    host.apply_cursor_move(ax - 20.0, ay - 10.0);
    host.apply_release_with_viewport(VW, VH);
    let moved = paths(&host)[0].anchors.as_ref().unwrap()[0].clone();
    assert!(
        (moved.x - (first.x - 20.0)).abs() < 0.5,
        "anchor x {}",
        moved.x
    );
    assert!(
        (moved.y - (first.y - 10.0)).abs() < 0.5,
        "anchor y {}",
        moved.y
    );
}
