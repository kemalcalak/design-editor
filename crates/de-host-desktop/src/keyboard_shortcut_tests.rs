//! Track C-6 (`Cmd+P` toggles Preview) + its non-conflict with the
//! existing `Cmd+Shift+P` (Export Image) chord. Split out of
//! `main_tests.rs` to keep that file under the repo's 800-line cap.

use super::*;
use de_editor_core::{agent_settings::SettingsFocus, figma_import_state::ImportSource};
use winit::keyboard::Key;

#[test]
fn cmd_p_enters_and_exits_preview() {
    let mut app = DesktopApp::new(None);
    assert!(!app.host.preview_active());

    app.zoom_modifier = true;
    app.handle_key_pressed(&Key::Character("p".into()), Some("p"));
    assert!(app.host.preview_active(), "Cmd+P must enter preview");

    app.handle_key_pressed(&Key::Character("p".into()), Some("p"));
    assert!(!app.host.preview_active(), "Cmd+P again must exit preview");
}

#[test]
fn cmd_p_works_even_while_preview_owns_the_keyboard() {
    // Preview's own takeover (Tab / arrow keys, gated on
    // `preview_active()`) only claims UNMODIFIED named keys — it must
    // never shadow the Cmd+P character chord that exits it.
    let mut app = DesktopApp::new(None);
    app.zoom_modifier = true;
    app.handle_key_pressed(&Key::Character("p".into()), Some("p"));
    assert!(app.host.preview_active());

    app.handle_key_pressed(&Key::Character("p".into()), Some("p"));
    assert!(
        !app.host.preview_active(),
        "Cmd+P must exit preview from within preview mode, not be swallowed by its keyboard takeover"
    );
}

#[test]
fn cmd_shift_p_stays_export_image_not_preview() {
    // Cmd+Shift+P is the pre-existing Export Image chord
    // (`keyboard_input.rs`'s `zoom_modifier && shift_modifier` arm) —
    // Track C-6 must not collide with it. Plain Cmd+P (no Shift) is the
    // only chord that toggles preview.
    let mut app = DesktopApp::new(None);
    app.zoom_modifier = true;
    app.shift_modifier = true;
    app.handle_key_pressed(&Key::Character("p".into()), Some("p"));
    assert!(
        !app.host.preview_active(),
        "Cmd+Shift+P must not toggle preview — it's Export Image"
    );
}

#[test]
fn cmd_shift_import_shortcuts_select_their_source() {
    let mut app = DesktopApp::new(None);
    app.zoom_modifier = true;
    app.shift_modifier = true;

    app.host.editor_state_mut().editor_ui.import_source = ImportSource::Html;
    app.handle_key_pressed(&Key::Character("f".into()), Some("f"));
    assert!(app.host.editor_state().editor_ui.figma_import_open);
    assert_eq!(
        app.host.editor_state().editor_ui.import_source,
        ImportSource::Figma
    );

    app.host.editor_state_mut().editor_ui.figma_import_open = false;
    app.handle_key_pressed(&Key::Character("H".into()), Some("H"));
    assert!(app.host.editor_state().editor_ui.figma_import_open);
    assert_eq!(
        app.host.editor_state().editor_ui.import_source,
        ImportSource::Html
    );
}

#[test]
fn cmd_shift_alt_does_not_open_an_import_modal() {
    let mut app = DesktopApp::new(None);
    app.zoom_modifier = true;
    app.shift_modifier = true;
    app.alt_modifier = true;

    app.handle_key_pressed(&Key::Character("f".into()), Some("f"));
    app.handle_key_pressed(&Key::Character("h".into()), Some("h"));
    assert!(!app.host.editor_state().editor_ui.figma_import_open);
}

#[test]
fn closing_settings_with_cmd_comma_does_not_leave_import_shortcuts_blocked() {
    let mut app = DesktopApp::new(None);
    app.zoom_modifier = true;
    {
        let ui = &mut app.host.editor_state_mut().editor_ui;
        ui.agent_settings_open = true;
        ui.agent_settings.focus = Some(SettingsFocus::McpPort);
    }

    app.handle_key_pressed(&Key::Character(",".into()), Some(","));
    assert!(!app.host.editor_state().editor_ui.agent_settings_open);
    assert!(app
        .host
        .editor_state()
        .editor_ui
        .agent_settings
        .focus
        .is_none());

    app.shift_modifier = true;
    app.handle_key_pressed(&Key::Character("f".into()), Some("f"));
    assert!(app.host.editor_state().editor_ui.figma_import_open);
}

#[test]
fn shift_h_and_shift_v_flip_the_selection_instead_of_switching_tools() {
    use de_editor_core::pen_node_ext::PenNodeExt;
    let mut app = DesktopApp::new(None);
    let tool_before = app.host.editor_state().tool;
    {
        let state = app.host.editor_state_mut();
        state.active_children_mut().push(
            serde_json::from_value(serde_json::json!({
                "type": "rectangle", "id": "flip-me", "width": 40, "height": 20
            }))
            .expect("rect"),
        );
        state.set_single_selection(de_editor_core::NodeId::new("flip-me"));
    }
    app.shift_modifier = true;
    app.handle_key_pressed(&Key::Character("H".into()), Some("H"));
    app.handle_key_pressed(&Key::Character("V".into()), Some("V"));

    let state = app.host.editor_state();
    let base = state.active_children().last().expect("rect").base();
    assert_eq!((base.flip_x, base.flip_y), (Some(true), Some(true)));
    assert_eq!(
        state.tool, tool_before,
        "Shift+H / Shift+V must not switch tools"
    );
}
