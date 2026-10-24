//! Web Pencil tool parity with native (`pencil_press.rs`, #88).

use super::WidgetHost;
use de_editor_core::Tool;
use jian_ops_schema::node::PenNode;

const VW: f32 = 1440.0;
const VH: f32 = 900.0;

fn empty_host() -> WidgetHost {
    let mut host = WidgetHost::new();
    let doc = jian_ops_schema::load_str(r#"{"version":"1.0.0","children":[]}"#)
        .expect("fixture JSON parses")
        .value;
    host.editor_state = de_editor_core::EditorState::from_document(doc);
    host.editor_state.editor_ui.sidebar_open = false;
    host.editor_state_dirty = true;
    host
}

fn path_count(host: &WidgetHost) -> usize {
    host.editor_state
        .active_children()
        .iter()
        .filter(|n| matches!(n, PenNode::Path(_)))
        .count()
}

#[test]
fn a_pencil_drag_commits_one_path_and_undoes_in_one_step() {
    let mut host = empty_host();
    host.apply_set_tool(Tool::Pencil);
    let (cx0, cy0, _, _) = host.canvas_region(VW, VH);
    let depth = host.editor_state.history.past.len();
    assert!(host.apply_press(cx0 + 300.0, cy0 + 300.0, VW, VH));
    for i in 1..=40 {
        let t = i as f32 * 3.0;
        host.apply_cursor_move(cx0 + 300.0 + t, cy0 + 300.0 + (t * 0.1).sin() * 20.0);
    }
    assert!(host.apply_release_with_viewport(VW, VH));
    assert_eq!(path_count(&host), 1);
    assert_eq!(host.editor_state.history.past.len(), depth + 1);
    assert!(host.apply_undo());
    assert_eq!(path_count(&host), 0);
}

/// #141: Escape mid-drag discards the stroke — no node, no history.
#[test]
fn escape_during_a_pencil_drag_discards_the_stroke() {
    let mut host = empty_host();
    host.apply_set_tool(Tool::Pencil);
    let (cx0, cy0, _, _) = host.canvas_region(VW, VH);
    let depth = host.editor_state.history.past.len();
    assert!(host.apply_press(cx0 + 300.0, cy0 + 300.0, VW, VH));
    for i in 1..=20 {
        let t = i as f32 * 4.0;
        host.apply_cursor_move(cx0 + 300.0 + t, cy0 + 300.0 + t * 0.25);
    }
    assert!(host.apply_escape());
    assert!(!host.editor_state.pencil_stroke_active());
    host.apply_release_with_viewport(VW, VH);
    assert_eq!(path_count(&host), 0, "no node");
    assert_eq!(host.editor_state.history.past.len(), depth, "no history");
    assert_eq!(host.editor_state.tool, Tool::Pencil);
}

#[test]
fn shift_p_picks_the_pencil_and_bare_p_the_pen() {
    let mut host = empty_host();
    assert!(host.apply_tool_shortcut_with_shift("P", true));
    assert_eq!(host.editor_state.tool, Tool::Pencil);
    assert!(host.apply_tool_shortcut("p"));
    assert_eq!(host.editor_state.tool, Tool::Pen);
}
