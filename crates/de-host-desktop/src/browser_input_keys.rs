//! winit keys and modifiers in DOM terms, for a browser node's page in
//! interaction mode (#324). winit's `KeyCode` names already follow the
//! W3C `code` values (`KeyA`, `Digit1`, `ArrowLeft`, …) and its named keys
//! the `key` values, apart from the few renamed here.

use de_editor_core::browser_input::InputModifiers;
use de_editor_ui::widgets::browser_interact_flow::KeyInput;
use winit::event::KeyEvent;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

/// The modifiers winit reports, as the page sees them.
pub(crate) fn input_modifiers(state: ModifiersState) -> InputModifiers {
    InputModifiers {
        alt: state.alt_key(),
        ctrl: state.control_key(),
        meta: state.super_key(),
        shift: state.shift_key(),
    }
}

/// `event` as a DOM key event.
pub(crate) fn key_input(event: &KeyEvent, modifiers: InputModifiers) -> KeyInput {
    KeyInput {
        key: dom_key(&event.logical_key),
        code: dom_code(event.physical_key),
        text: event.text.as_ref().map(|text| text.to_string()),
        modifiers,
    }
}

/// The DOM `key` value of a winit logical key.
pub(crate) fn dom_key(key: &Key) -> String {
    match key {
        Key::Character(text) => text.to_string(),
        Key::Named(NamedKey::Space) => " ".into(),
        Key::Named(NamedKey::Super | NamedKey::Meta) => "Meta".into(),
        Key::Named(named) => format!("{named:?}"),
        Key::Dead(_) => "Dead".into(),
        Key::Unidentified(_) => "Unidentified".into(),
    }
}

/// The DOM `code` value of a winit physical key (empty when unknown).
pub(crate) fn dom_code(key: PhysicalKey) -> String {
    match key {
        PhysicalKey::Code(KeyCode::SuperLeft | KeyCode::Meta) => "MetaLeft".into(),
        PhysicalKey::Code(KeyCode::SuperRight) => "MetaRight".into(),
        PhysicalKey::Code(code) => format!("{code:?}"),
        PhysicalKey::Unidentified(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_codes_use_dom_names() {
        assert_eq!(dom_key(&Key::Character("a".into())), "a");
        assert_eq!(dom_key(&Key::Named(NamedKey::Enter)), "Enter");
        assert_eq!(dom_key(&Key::Named(NamedKey::ArrowLeft)), "ArrowLeft");
        assert_eq!(dom_key(&Key::Named(NamedKey::Space)), " ");
        assert_eq!(dom_key(&Key::Named(NamedKey::Super)), "Meta");
        assert_eq!(dom_code(PhysicalKey::Code(KeyCode::KeyA)), "KeyA");
        assert_eq!(dom_code(PhysicalKey::Code(KeyCode::Digit1)), "Digit1");
        assert_eq!(dom_code(PhysicalKey::Code(KeyCode::Backspace)), "Backspace");
        assert_eq!(dom_code(PhysicalKey::Code(KeyCode::SuperLeft)), "MetaLeft");
        assert_eq!(
            dom_code(PhysicalKey::Code(KeyCode::BracketLeft)),
            "BracketLeft"
        );
    }

    #[test]
    fn modifiers_map_super_to_meta() {
        let mods = input_modifiers(ModifiersState::SUPER | ModifiersState::SHIFT);
        assert!(mods.meta && mods.shift && !mods.ctrl && !mods.alt);
    }
}
