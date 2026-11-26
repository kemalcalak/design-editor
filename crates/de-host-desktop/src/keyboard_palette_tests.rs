use crate::DesktopApp;
use de_editor_core::command_palette::PaletteMode;
use winit::keyboard::{Key, NamedKey};

fn app() -> DesktopApp {
    let mut app = DesktopApp::new(None);
    app.host.editor_state_mut().editor_ui.home.hide();
    app
}

fn press(app: &mut DesktopApp, key: Key, text: Option<&str>) {
    app.handle_key_pressed(&key, text);
}

fn chord(app: &mut DesktopApp, ch: &str) {
    app.zoom_modifier = true;
    press(app, Key::Character(ch.into()), Some(ch));
    app.zoom_modifier = false;
}

fn mode(app: &DesktopApp) -> Option<PaletteMode> {
    app.host
        .editor_state()
        .editor_ui
        .command_palette
        .as_ref()
        .map(|palette| palette.mode)
}

#[test]
fn chords_open_switch_and_close_the_palette() {
    let mut app = app();
    chord(&mut app, "k");
    assert_eq!(mode(&app), Some(PaletteMode::Commands));
    chord(&mut app, "f");
    assert_eq!(mode(&app), Some(PaletteMode::FindLayers));
    chord(&mut app, "f");
    assert_eq!(mode(&app), None);
    chord(&mut app, "/");
    assert_eq!(mode(&app), Some(PaletteMode::Commands));
    press(&mut app, Key::Named(NamedKey::Escape), None);
    assert_eq!(mode(&app), None);
}

#[test]
fn the_open_palette_owns_letters_arrows_and_enter() {
    let mut app = app();
    let tool_before = app.host.editor_state().tool;
    chord(&mut app, "k");
    for ch in ["r", "e", "c", "t", "a", "n", "g", "l", "e"] {
        press(&mut app, Key::Character(ch.into()), Some(ch));
    }
    assert_eq!(app.host.editor_state().tool, tool_before, "no tool switch");
    let typed = app
        .host
        .editor_state()
        .editor_ui
        .command_palette
        .as_ref()
        .unwrap()
        .input
        .text()
        .to_string();
    assert_eq!(typed, "rectangle");
    press(&mut app, Key::Named(NamedKey::ArrowDown), None);
    press(&mut app, Key::Named(NamedKey::ArrowUp), None);
    press(&mut app, Key::Named(NamedKey::Enter), Some("\r"));
    assert_eq!(mode(&app), None);
    assert_eq!(app.host.editor_state().tool, de_editor_core::Tool::Rect);
}

#[test]
fn document_chords_are_swallowed_while_open() {
    let mut app = app();
    chord(&mut app, "k");
    let revision = app.host.editor_state().revision;
    chord(&mut app, "d");
    chord(&mut app, "g");
    assert_eq!(app.host.editor_state().revision, revision);
    assert_eq!(mode(&app), Some(PaletteMode::Commands));
}

#[test]
fn a_runner_command_is_drained_after_the_event() {
    let mut app = app();
    chord(&mut app, "k");
    for ch in "new chat".chars() {
        let s = ch.to_string();
        press(&mut app, Key::Character(s.as_str().into()), Some(&s));
    }
    let tabs_before = app.host.editor_state().chat.tabs().len();
    press(&mut app, Key::Named(NamedKey::Enter), Some("\r"));
    assert!(app.drain_palette_request());
    assert_eq!(app.host.editor_state().chat.tabs().len(), tabs_before + 1);
    assert!(!app.drain_palette_request());
}
