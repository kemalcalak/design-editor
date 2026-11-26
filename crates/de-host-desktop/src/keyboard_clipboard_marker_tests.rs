//! Node copy marks the system clipboard (#436): after Cmd+C / Cmd+X on
//! nodes, Cmd+V pastes those nodes whatever the clipboard held before, and
//! anything copied elsewhere afterwards pastes as before.

use super::ClipboardPayload;
use crate::clipboard::ClipboardImage;
use crate::DesktopApp;
use de_editor_core::node_clipboard_marker::NODE_CLIPBOARD_MARKER;
use de_editor_core::NodeId;
use jian_ops_schema::node::PenNode;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
    <path d="M2 2 L22 22" stroke="#000"/></svg>"##;

fn text(text: &str) -> ClipboardPayload {
    ClipboardPayload {
        text: Some(text.to_string()),
        ..ClipboardPayload::default()
    }
}

/// Cmd+C on the starter node through the host, then hand the runner's
/// queued system-clipboard write back as the next paste's clipboard, as
/// the OS would.
fn copy_node_like_cmd_c(app: &mut DesktopApp) -> ClipboardPayload {
    let state = app.host.editor_state_mut();
    state.set_single_selection(NodeId::new("n10"));
    assert!(app.host.apply_copy());
    let written = app
        .host
        .editor_state_mut()
        .chat
        .pending_copy_text
        .take()
        .expect("node copy queues a system-clipboard write");
    assert_eq!(written, NODE_CLIPBOARD_MARKER);
    app.host.editor_state_mut().clear_selection();
    text(&written)
}

fn pasted_one_copy_of_the_node(app: &DesktopApp) {
    let state = app.host.editor_state();
    assert_eq!(state.active_children().len(), 2);
    assert!(state.selection.anchor.is_real());
    assert_ne!(state.selection.anchor, NodeId::new("n10"));
    let pasted = state.selected_node().expect("the pasted node is selected");
    assert!(!matches!(pasted, PenNode::Group(_) | PenNode::Image(_)));
}

#[test]
fn copy_as_svg_then_node_copy_pastes_the_node() {
    let mut app = DesktopApp::new(None);
    // The stale Copy as SVG pastes as vectors while it is the clipboard...
    assert!(app.handle_paste_payload(text(SVG)));
    assert!(matches!(
        app.host.editor_state().selected_node(),
        Some(PenNode::Group(_))
    ));
    assert!(app.host.editor_state_mut().undo());
    assert_eq!(app.host.editor_state().active_children().len(), 1);

    // ...until a node copy replaces it with the marker.
    let clipboard = copy_node_like_cmd_c(&mut app);
    assert!(app.handle_paste_payload(clipboard));

    pasted_one_copy_of_the_node(&app);
}

#[test]
fn node_cut_then_paste_restores_the_node() {
    let mut app = DesktopApp::new(None);
    app.host
        .editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    assert!(app.host.apply_cut());
    let written = app.host.editor_state_mut().chat.pending_copy_text.take();
    assert_eq!(written.as_deref(), Some(NODE_CLIPBOARD_MARKER));
    assert!(app.host.editor_state().active_children().is_empty());

    assert!(app.handle_paste_payload(text(NODE_CLIPBOARD_MARKER)));

    assert_eq!(app.host.editor_state().active_children().len(), 1);
}

#[test]
fn the_marker_wins_over_flavours_a_platform_failed_to_clear() {
    let mut app = DesktopApp::new(None);
    let mut clipboard = copy_node_like_cmd_c(&mut app);
    clipboard.image = Some(ClipboardImage {
        png: vec![1, 2, 3],
        width: 4,
        height: 4,
    });
    clipboard.html = Some("<p>stale</p>".to_string());

    assert!(app.handle_paste_payload(clipboard));

    pasted_one_copy_of_the_node(&app);
    assert!(app.pending_html_paste.is_none());
}

#[test]
fn svg_copied_elsewhere_after_a_node_copy_pastes_as_vectors() {
    let mut app = DesktopApp::new(None);
    copy_node_like_cmd_c(&mut app);

    assert!(app.handle_paste_payload(text(SVG)));

    let state = app.host.editor_state();
    assert_eq!(state.active_children().len(), 2);
    assert!(matches!(state.selected_node(), Some(PenNode::Group(_))));
}

#[test]
fn image_copied_elsewhere_after_a_node_copy_pastes_as_an_image() {
    let mut app = DesktopApp::new(None);
    copy_node_like_cmd_c(&mut app);

    assert!(app.handle_paste_payload(ClipboardPayload {
        image: Some(ClipboardImage {
            png: vec![1, 2, 3],
            width: 40,
            height: 20,
        }),
        ..ClipboardPayload::default()
    }));

    assert!(matches!(
        app.host.editor_state().selected_node(),
        Some(PenNode::Image(_))
    ));
}

#[test]
fn html_copied_elsewhere_after_a_node_copy_is_imported() {
    let mut app = DesktopApp::new(None);
    copy_node_like_cmd_c(&mut app);

    assert!(app.handle_paste_payload(ClipboardPayload {
        html: Some("<div><p>Hello</p></div>".to_string()),
        ..ClipboardPayload::default()
    }));

    // The HTML decode runs off the UI thread; the node clipboard is not used.
    assert!(app.pending_html_paste.is_some());
    assert_eq!(app.host.editor_state().active_children().len(), 1);
}

#[test]
fn text_inputs_paste_nothing_for_the_marker_and_text_as_before() {
    let mut app = DesktopApp::new(None);
    copy_node_like_cmd_c(&mut app);
    app.host.editor_state_mut().chat.focused = true;

    assert!(app.handle_paste_payload(text(NODE_CLIPBOARD_MARKER)));
    assert!(app.host.editor_state().chat.input.text().is_empty());
    assert_eq!(app.host.editor_state().active_children().len(), 1);

    assert!(app.handle_paste_payload(text("hello")));
    assert_eq!(app.host.editor_state().chat.input.text(), "hello");

    let state = app.host.editor_state_mut();
    state.chat.focused = false;
    state.editor_ui.chat_model_picker.open = true;
    assert!(app.handle_paste_payload(text(NODE_CLIPBOARD_MARKER)));
    assert!(app
        .host
        .editor_state()
        .editor_ui
        .chat_model_picker_input
        .text()
        .is_empty());
    assert!(app.handle_paste_payload(text("gp")));
    assert_eq!(
        app.host
            .editor_state()
            .editor_ui
            .chat_model_picker_input
            .text(),
        "gp"
    );
}
