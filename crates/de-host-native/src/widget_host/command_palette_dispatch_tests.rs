use super::*;
use de_editor_core::NodeId;

fn host() -> WidgetHostNative {
    let doc = serde_json::json!({"version": "1.0.0", "pages": [
        {"id": "p1", "name": "Home", "children": [
            {"type": "frame", "id": "board", "name": "Board", "width": 400, "height": 300,
             "children": [{"type": "text", "id": "title", "name": "Title", "content": "Welcome"}]}
        ]},
        {"id": "p2", "name": "Checkout", "children": [
            {"type": "frame", "id": "pay", "name": "Pay board", "x": 2000, "y": 2000,
             "width": 400, "height": 300,
             "children": [{"type": "text", "id": "cta", "name": "Pay now", "content": "Pay now"}]}
        ]}
    ]});
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    );
    host.editor_state_mut().editor_ui.home.hide();
    host.mark_editor_state_dirty();
    host
}

fn type_text(host: &mut WidgetHostNative, text: &str) {
    for c in text.chars() {
        assert!(host.apply_text(c));
    }
}

#[test]
fn the_palette_is_modal_over_keys_presses_and_document_chords() {
    let mut host = host();
    assert!(host.apply_toggle_command_palette(PaletteMode::Commands));
    assert!(host.input_active_pub(), "letters must not switch tools");
    assert!(host.non_chat_input_owns_keyboard_pub());
    host.editor_state_mut()
        .set_single_selection(NodeId::new("board"));
    assert!(!host.apply_group());
    assert!(!host.apply_undo());
    assert!(!host.apply_open_rename_layers());
    assert!(!host.toggle_preview_with_cached_viewport());
    assert!(host.dispatch_command_palette_press(600.0, 790.0, 1200.0, 800.0));
    assert!(!host.command_palette_open(), "the scrim closes it");
}

#[test]
fn find_enter_switches_page_selects_and_frames() {
    let mut host = host();
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    assert!(host.apply_toggle_command_palette(PaletteMode::FindLayers));
    type_text(&mut host, "pay now");
    let before = host.editor_state().viewport;
    assert!(host.apply_send());
    assert!(!host.command_palette_open());
    assert_eq!(host.editor_state().ui.active_page_index, 1);
    assert_eq!(host.editor_state().selection.anchor.as_str(), "cta");
    assert_ne!(host.editor_state().viewport, before, "the match is framed");
}

#[test]
fn shift_enter_selects_every_match_and_escape_closes() {
    let mut host = host();
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    assert!(host.apply_toggle_command_palette(PaletteMode::FindLayers));
    type_text(&mut host, "type:frame");
    assert!(host.palette_activate(true));
    assert_eq!(host.editor_state().selection.set.len(), 1);
    assert!(host.apply_toggle_command_palette(PaletteMode::FindLayers));
    assert!(host.apply_escape());
    assert!(!host.command_palette_open());
}

#[test]
fn commands_run_through_their_shortcut_paths() {
    let mut host = host();
    assert!(host.apply_toggle_command_palette(PaletteMode::Commands));
    type_text(&mut host, "rectangle tool");
    assert!(host.apply_send());
    assert_eq!(host.editor_state().tool, de_editor_core::Tool::Rect);
    // A file command is handed to the runner.
    assert!(host.apply_toggle_command_palette(PaletteMode::Commands));
    type_text(&mut host, "save as");
    assert!(host.apply_send());
    assert_eq!(
        host.take_palette_runner_request(),
        Some(PaletteRunnerRequest::Command(PaletteCommand::SaveAs))
    );
    assert_eq!(host.take_palette_runner_request(), None);
}

#[test]
fn backspace_and_paste_refresh_the_results() {
    let mut host = host();
    assert!(host.apply_toggle_command_palette(PaletteMode::FindLayers));
    assert!(host.apply_input_paste("welc"));
    let rows = |host: &WidgetHostNative| {
        host.editor_state()
            .editor_ui
            .command_palette
            .as_ref()
            .unwrap()
            .rows
            .len()
    };
    assert_eq!(rows(&host), 1);
    for _ in 0..4 {
        assert!(host.apply_backspace());
    }
    assert_eq!(rows(&host), 0, "an empty query lists nothing");
}

#[test]
fn a_replace_survives_an_inline_edit_that_was_open() {
    let mut host = host();
    assert!(host
        .editor_state_mut()
        .start_text_edit(NodeId::new("title")));
    host.mark_editor_state_dirty();
    assert!(host.apply_toggle_command_palette(PaletteMode::FindLayers));
    type_text(&mut host, "welcome");
    assert!(host.palette_tab());
    type_text(&mut host, "Hello");
    assert!(host.apply_send(), "Enter in the replace field");
    assert!(host.apply_escape());
    // The next keys must not write the old edit's draft back.
    host.apply_text('x');
    let content = |host: &WidgetHostNative| {
        let node = de_editor_core::walkers::find_node(
            host.editor_state().active_children(),
            &NodeId::new("title"),
        )
        .unwrap();
        de_editor_core::find_layers::node_text_content(node).unwrap()
    };
    assert_eq!(content(&host), "Hello");
    assert!(host.editor_state().ui.text_editing.is_none());
}

#[test]
fn cmd_a_from_the_menu_selects_the_palette_field() {
    let mut host = host();
    assert!(host.apply_toggle_command_palette(PaletteMode::Commands));
    type_text(&mut host, "undo");
    host.editor_state_mut()
        .set_single_selection(NodeId::new("board"));
    assert!(host.apply_select_all());
    let palette = host
        .editor_state()
        .editor_ui
        .command_palette
        .as_ref()
        .unwrap();
    assert!(palette.input.is_select_all());
    assert_eq!(
        host.editor_state().selection.set.len(),
        1,
        "canvas untouched"
    );
}
