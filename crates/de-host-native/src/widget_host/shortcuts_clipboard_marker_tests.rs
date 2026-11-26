//! A user node copy / cut asks for the node-copy marker on the system
//! clipboard (#436); nothing else does, and no plain text is queued.

use super::WidgetHostNative;
use de_editor_core::NodeId;

const TWO_RECTS: &str = r#"{"version":"1.0.0","children":[
    {"type":"rectangle","id":"n1","name":"n1","x":0,"y":0,"width":10,"height":10},
    {"type":"rectangle","id":"n2","name":"n2","x":20,"y":0,"width":10,"height":10}]}"#;

fn host() -> WidgetHostNative {
    let doc = jian_ops_schema::load_str(TWO_RECTS)
        .expect("fixture JSON parses")
        .value;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host
}

fn marker_requested(host: &WidgetHostNative) -> bool {
    host.editor_state()
        .editor_ui
        .canvas_view
        .pending_node_clipboard_marker
}

fn text_queued(host: &WidgetHostNative) -> Option<&str> {
    host.editor_state().chat.pending_copy_text.as_deref()
}

#[test]
fn node_copy_requests_the_marker_and_fills_the_node_clipboard() {
    let mut host = host();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n1"));

    assert!(host.apply_copy());

    assert!(marker_requested(&host));
    assert_eq!(text_queued(&host), None);
    assert_eq!(host.editor_state().clipboard.len(), 1);
}

#[test]
fn node_cut_requests_the_marker() {
    let mut host = host();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n2"));

    assert!(host.apply_cut());

    assert!(marker_requested(&host));
    assert_eq!(text_queued(&host), None);
    assert_eq!(host.editor_state().clipboard.len(), 1);
    assert_eq!(host.editor_state().active_children().len(), 1);
}

#[test]
fn copy_or_cut_with_nothing_selected_leaves_the_system_clipboard_alone() {
    let mut host = host();

    assert!(!host.apply_copy());
    assert!(!host.apply_cut());

    assert!(!marker_requested(&host));
    assert_eq!(text_queued(&host), None);
}

#[test]
fn duplicate_and_paste_do_not_touch_the_system_clipboard() {
    let mut host = host();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n1"));
    assert!(host.apply_copy());
    host.editor_state_mut()
        .editor_ui
        .canvas_view
        .pending_node_clipboard_marker = false;

    assert!(host.apply_duplicate());
    assert!(host.apply_paste());

    assert!(!marker_requested(&host));
    assert_eq!(text_queued(&host), None);
    assert_eq!(host.editor_state().active_children().len(), 4);
}
