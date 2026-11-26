//! Node copy marks the system clipboard (#436): after Cmd+C / Cmd+X on
//! nodes, Cmd+V pastes those nodes whatever the clipboard held before, and
//! anything copied elsewhere afterwards pastes as before.

use super::ClipboardPayload;
use crate::clipboard::ClipboardImage;
use crate::system_clipboard::{ClipboardWrite, RecordingClipboard};
use crate::DesktopApp;
use de_editor_core::node_clipboard_marker::NODE_CLIPBOARD_MARKER_HTML;
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

/// The clipboard as the OS hands the marker back: macOS wraps it in a
/// document.
fn marker_on_clipboard() -> ClipboardPayload {
    ClipboardPayload {
        html: Some(format!(
            "<html><head><meta charset=utf-8></head><body>{NODE_CLIPBOARD_MARKER_HTML}</body></html>"
        )),
        ..ClipboardPayload::default()
    }
}

/// An app whose clipboard writes are recorded instead of reaching the OS.
fn recording_app() -> (DesktopApp, RecordingClipboard) {
    let mut app = DesktopApp::new(None);
    let recorder = RecordingClipboard::default();
    app.clipboard = Box::new(recorder.clone());
    (app, recorder)
}

/// Cmd+C on the starter node through the real shortcut path; returns the
/// clipboard the next paste reads.
fn cmd_c_on_node(app: &mut DesktopApp, recorder: &RecordingClipboard) -> ClipboardPayload {
    app.host
        .editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    assert!(app.handle_cmd_copy());
    assert_eq!(
        recorder.writes.borrow().last(),
        Some(&ClipboardWrite::NodeMarker)
    );
    app.host.editor_state_mut().clear_selection();
    marker_on_clipboard()
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
fn cmd_c_and_cmd_x_on_nodes_write_only_the_marker() {
    let (mut app, recorder) = recording_app();
    cmd_c_on_node(&mut app, &recorder);
    app.host
        .editor_state_mut()
        .set_single_selection(NodeId::new("n10"));

    assert!(app.handle_cmd_cut());

    assert_eq!(
        *recorder.writes.borrow(),
        vec![ClipboardWrite::NodeMarker, ClipboardWrite::NodeMarker]
    );
    let state = app.host.editor_state();
    assert!(!state.editor_ui.canvas_view.pending_node_clipboard_marker);
    assert!(state.chat.pending_copy_text.is_none());
}

#[test]
fn cmd_c_with_nothing_selected_writes_nothing() {
    let (mut app, recorder) = recording_app();

    app.handle_cmd_copy();
    app.handle_cmd_cut();

    assert!(recorder.writes.borrow().is_empty());
}

#[test]
fn cmd_c_in_a_text_field_writes_its_text_not_the_marker() {
    let (mut app, recorder) = recording_app();
    let state = app.host.editor_state_mut();
    state.set_single_selection(NodeId::new("n10"));
    state.editor_ui.chat_model_picker.open = true;
    state.editor_ui.chat_model_picker_input.set_text("gpt");
    state.editor_ui.chat_model_picker_input.select_all();

    assert!(app.handle_cmd_copy());

    assert_eq!(
        *recorder.writes.borrow(),
        vec![ClipboardWrite::Text("gpt".to_string())]
    );
}

#[test]
fn copy_as_svg_then_node_copy_pastes_the_node() {
    let (mut app, recorder) = recording_app();
    // The stale Copy as SVG pastes as vectors while it is the clipboard...
    assert!(app.handle_paste_payload(text(SVG)));
    assert!(matches!(
        app.host.editor_state().selected_node(),
        Some(PenNode::Group(_))
    ));
    assert!(app.host.editor_state_mut().undo());
    assert_eq!(app.host.editor_state().active_children().len(), 1);

    // ...until a node copy replaces it with the marker.
    let clipboard = cmd_c_on_node(&mut app, &recorder);
    assert!(app.handle_paste_payload(clipboard));

    pasted_one_copy_of_the_node(&app);
}

#[test]
fn node_cut_then_paste_restores_the_node() {
    let (mut app, recorder) = recording_app();
    app.host
        .editor_state_mut()
        .set_single_selection(NodeId::new("n10"));
    assert!(app.handle_cmd_cut());
    assert_eq!(*recorder.writes.borrow(), vec![ClipboardWrite::NodeMarker]);
    assert!(app.host.editor_state().active_children().is_empty());

    assert!(app.handle_paste_payload(marker_on_clipboard()));

    assert_eq!(app.host.editor_state().active_children().len(), 1);
}

#[test]
fn the_marker_wins_over_flavours_a_platform_failed_to_clear() {
    let (mut app, recorder) = recording_app();
    let mut clipboard = cmd_c_on_node(&mut app, &recorder);
    clipboard.image = Some(ClipboardImage {
        png: vec![1, 2, 3],
        width: 4,
        height: 4,
    });
    clipboard.text = Some(SVG.to_string());

    assert!(app.handle_paste_payload(clipboard));

    pasted_one_copy_of_the_node(&app);
    assert!(app.pending_html_paste.is_none());
    assert!(app.pending_figma_paste.is_none());
}

#[test]
fn svg_copied_elsewhere_after_a_node_copy_pastes_as_vectors() {
    let (mut app, recorder) = recording_app();
    cmd_c_on_node(&mut app, &recorder);

    assert!(app.handle_paste_payload(text(SVG)));

    let state = app.host.editor_state();
    assert_eq!(state.active_children().len(), 2);
    assert!(matches!(state.selected_node(), Some(PenNode::Group(_))));
}

#[test]
fn image_copied_elsewhere_after_a_node_copy_pastes_as_an_image() {
    let (mut app, recorder) = recording_app();
    cmd_c_on_node(&mut app, &recorder);

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
    let (mut app, recorder) = recording_app();
    cmd_c_on_node(&mut app, &recorder);

    assert!(app.handle_paste_payload(ClipboardPayload {
        html: Some("<div><p>Hello</p></div>".to_string()),
        ..ClipboardPayload::default()
    }));

    // The HTML decode runs off the UI thread; the node clipboard is not used.
    assert!(app.pending_html_paste.is_some());
    assert_eq!(app.host.editor_state().active_children().len(), 1);
}

#[test]
fn text_inputs_paste_text_as_before_while_the_marker_is_on_the_clipboard() {
    let (mut app, recorder) = recording_app();
    cmd_c_on_node(&mut app, &recorder);
    app.host.editor_state_mut().chat.focused = true;

    // The marker has no text flavour: a text field reads none.
    assert!(app.handle_paste_payload(ClipboardPayload::default()));
    assert!(app.host.editor_state().chat.input.text().is_empty());
    assert_eq!(app.host.editor_state().active_children().len(), 1);

    assert!(app.handle_paste_payload(text("hello")));
    assert_eq!(app.host.editor_state().chat.input.text(), "hello");
}
