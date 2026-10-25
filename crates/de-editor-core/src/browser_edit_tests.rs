use jian_ops_schema::node::PenNode;

use crate::command_node::build_leaf_node;
use crate::pen_node_ext::PenNodeExt;
use crate::test_support::state_with;
use crate::{EditorState, NodeId};

fn with_browser() -> EditorState {
    let node = build_leaf_node("browser", "b1", "Browser", 10, 20, 1440, 900).expect("builds");
    let mut state = state_with(vec![node]);
    state.set_single_selection(NodeId::new("b1"));
    state
}

#[test]
fn a_browser_builds_at_its_box_and_takes_a_normalized_url() {
    let mut state = with_browser();
    assert!(matches!(state.selected_node(), Some(PenNode::Browser(_))));
    assert!(state.set_selected_browser_url("example.com"));
    assert!(!state.set_selected_browser_url("https://example.com"));
    assert!(!state.set_selected_browser_url("javascript:alert(1)"));
    let browser = state.selected_browser().expect("browser");
    assert_eq!(browser.url.as_deref(), Some("https://example.com"));
    assert_eq!(browser.base.x, Some(10.0));
    assert!(state.set_selected_browser_url("  "));
    assert_eq!(state.selected_browser().unwrap().url, None);
}

#[test]
fn a_known_device_resizes_the_node_and_an_unknown_one_is_kept() {
    let mut state = with_browser();
    assert!(state.set_selected_browser_device(Some("phone")));
    let node = state.selected_node().unwrap();
    assert_eq!(
        (node.width_px(), node.height_px()),
        (Some(390.0), Some(844.0))
    );
    assert!(!state.set_selected_browser_device(Some("phone")));
    assert!(state.set_selected_browser_device(Some("foldable-x")));
    let node = state.selected_node().unwrap();
    assert_eq!(node.width_px(), Some(390.0));
    assert_eq!(
        state.selected_browser().unwrap().device_id.as_deref(),
        Some("foldable-x")
    );
    assert!(state.set_selected_browser_device(None));
    assert_eq!(state.selected_browser().unwrap().device_id, None);
}

#[test]
fn zoom_and_scroll_clamp_and_skip_no_ops() {
    let mut state = with_browser();
    assert!(!state.set_selected_browser_zoom(1.0));
    assert!(state.set_selected_browser_zoom(9.0));
    assert_eq!(state.selected_browser().unwrap().zoom, Some(4.0));
    assert!(state.set_selected_browser_scroll(None, Some(240.0)));
    assert!(!state.set_selected_browser_scroll(Some(-3.0), Some(240.0)));
    let browser = state.selected_browser().unwrap();
    assert_eq!((browser.scroll_x, browser.scroll_y), (None, Some(240.0)));
}

#[test]
fn an_edit_undoes_as_one_step_and_other_kinds_are_refused() {
    // The panel records the step around the write, as its dispatch does.
    let mut state = with_browser();
    let before = state.snapshot_for_history();
    assert!(state.set_selected_browser_url("example.com"));
    state.history_push_past(before);
    assert!(state.undo());
    assert_eq!(state.selected_browser().unwrap().url, None);
    let rect = build_leaf_node("rect", "r1", "Rect", 0, 0, 10, 10).unwrap();
    let mut state = state_with(vec![rect]);
    state.set_single_selection(NodeId::new("r1"));
    assert!(state.selected_browser().is_none());
    assert!(!state.set_selected_browser_url("example.com"));
    assert!(!state.set_selected_browser_zoom(2.0));
}
