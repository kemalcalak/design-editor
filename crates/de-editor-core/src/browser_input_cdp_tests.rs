use serde_json::json;

use super::*;

fn none() -> InputModifiers {
    InputModifiers::default()
}

fn key_down(
    key: &str,
    code: &str,
    text: Option<&str>,
    modifiers: InputModifiers,
) -> BrowserInputEvent {
    BrowserInputEvent::KeyDown {
        key: key.into(),
        code: code.into(),
        text: text.map(str::to_string),
        modifiers,
    }
}

#[test]
fn a_click_is_a_press_and_a_release_with_the_button_and_count() {
    let down = cdp_call(
        &BrowserInputEvent::MouseDown {
            x: 12.5,
            y: 40.0,
            button: MouseButton::Left,
            click_count: 1,
            buttons: 0,
            modifiers: none(),
        },
        false,
    );
    assert_eq!(down.method, "Input.dispatchMouseEvent");
    assert_eq!(
        down.params,
        json!({
            "type": "mousePressed", "x": 12.5, "y": 40.0, "button": "left",
            "buttons": 1, "clickCount": 1, "modifiers": 0,
        })
    );
    let up = cdp_call(
        &BrowserInputEvent::MouseUp {
            x: 12.5,
            y: 40.0,
            button: MouseButton::Left,
            click_count: 1,
            buttons: 1,
            modifiers: none(),
        },
        false,
    );
    assert_eq!(up.params["type"], "mouseReleased");
    assert_eq!(up.params["buttons"], 0);
}

#[test]
fn a_move_names_the_held_button() {
    let call = |buttons| {
        cdp_call(
            &BrowserInputEvent::MouseMove {
                x: 1.0,
                y: 2.0,
                buttons,
                modifiers: none(),
            },
            false,
        )
        .params
    };
    assert_eq!(call(0)["button"], "none");
    assert_eq!(call(1)["button"], "left");
    assert_eq!(call(2)["button"], "right");
    assert_eq!(call(0)["type"], "mouseMoved");
    assert!(call(0).get("clickCount").is_none());
}

#[test]
fn a_wheel_carries_its_deltas_and_modifiers() {
    let call = cdp_call(
        &BrowserInputEvent::Wheel {
            x: 5.0,
            y: 6.0,
            delta_x: -3.0,
            delta_y: 120.0,
            modifiers: InputModifiers {
                shift: true,
                ..none()
            },
        },
        false,
    );
    assert_eq!(
        call.params,
        json!({ "type": "mouseWheel", "x": 5.0, "y": 6.0, "deltaX": -3.0, "deltaY": 120.0, "modifiers": 8 })
    );
}

#[test]
fn a_typing_key_sends_its_text_and_key_code() {
    let call = cdp_call(&key_down("a", "KeyA", Some("a"), none()), false);
    assert_eq!(call.method, "Input.dispatchKeyEvent");
    assert_eq!(
        call.params,
        json!({
            "type": "keyDown", "key": "a", "code": "KeyA", "modifiers": 0,
            "text": "a", "unmodifiedText": "a",
            "windowsVirtualKeyCode": 65, "nativeVirtualKeyCode": 65,
        })
    );
}

#[test]
fn enter_types_a_carriage_return_and_backspace_types_nothing() {
    let enter = cdp_call(&key_down("Enter", "Enter", None, none()), false);
    assert_eq!(enter.params["type"], "keyDown");
    assert_eq!(enter.params["text"], "\r");
    assert_eq!(enter.params["windowsVirtualKeyCode"], 13);
    // A host that reports the control char as text gets the same.
    let back = cdp_call(
        &key_down("Backspace", "Backspace", Some("\u{8}"), none()),
        false,
    );
    assert_eq!(back.params["type"], "rawKeyDown");
    assert!(back.params.get("text").is_none());
    assert_eq!(back.params["windowsVirtualKeyCode"], 8);
}

#[test]
fn a_shortcut_chord_types_nothing() {
    let meta = InputModifiers {
        meta: true,
        ..none()
    };
    let call = cdp_call(&key_down("a", "KeyA", Some("a"), meta), false);
    assert_eq!(call.params["type"], "rawKeyDown");
    assert!(call.params.get("text").is_none());
    assert_eq!(call.params["modifiers"], 4);
    assert!(call.params.get("commands").is_none());
}

#[test]
fn alt_gr_types_its_character() {
    let alt_gr = InputModifiers {
        ctrl: true,
        alt: true,
        ..none()
    };
    let call = cdp_call(&key_down("@", "KeyQ", Some("@"), alt_gr), false);
    assert_eq!(call.params["type"], "keyDown");
    assert_eq!(call.params["text"], "@");
}

#[test]
fn macos_editing_keys_name_their_command() {
    let meta = InputModifiers {
        meta: true,
        ..none()
    };
    let select_all = cdp_call(&key_down("a", "KeyA", Some("a"), meta), true);
    assert_eq!(select_all.params["commands"], json!(["selectAll"]));
    let back = cdp_call(&key_down("Backspace", "Backspace", None, none()), true);
    assert_eq!(back.params["commands"], json!(["deleteBackward"]));
    let shift = InputModifiers {
        shift: true,
        ..none()
    };
    let left = cdp_call(&key_down("ArrowLeft", "ArrowLeft", None, shift), true);
    assert_eq!(
        left.params["commands"],
        json!(["moveLeftAndModifySelection"])
    );
    let typed = cdp_call(&key_down("a", "KeyA", Some("a"), none()), true);
    assert!(typed.params.get("commands").is_none());
}

#[test]
fn key_up_and_insert_text() {
    let up = cdp_call(
        &BrowserInputEvent::KeyUp {
            key: "ArrowDown".into(),
            code: "ArrowDown".into(),
            modifiers: none(),
        },
        false,
    );
    assert_eq!(up.params["type"], "keyUp");
    assert_eq!(up.params["windowsVirtualKeyCode"], 40);
    let insert = cdp_call(
        &BrowserInputEvent::InsertText {
            text: "日本".into(),
        },
        false,
    );
    assert_eq!(insert.method, "Input.insertText");
    assert_eq!(insert.params, json!({ "text": "日本" }));
}

#[test]
fn virtual_key_codes_cover_letters_digits_and_named_keys() {
    assert_eq!(virtual_key_code("KeyZ"), Some(90));
    assert_eq!(virtual_key_code("Digit7"), Some(55));
    assert_eq!(virtual_key_code("Numpad3"), Some(99));
    assert_eq!(virtual_key_code("F5"), Some(116));
    assert_eq!(virtual_key_code("Tab"), Some(9));
    assert_eq!(virtual_key_code("Space"), Some(32));
    assert_eq!(virtual_key_code("Keyz"), None);
    assert_eq!(virtual_key_code("Unidentified"), None);
    // No code: the key names it.
    let call = cdp_call(&key_down("q", "", Some("q"), none()), false);
    assert_eq!(call.params["windowsVirtualKeyCode"], 81);
}

#[test]
fn a_batch_keeps_its_order() {
    let calls = cdp_calls(
        &[
            key_down("h", "KeyH", Some("h"), none()),
            BrowserInputEvent::InsertText { text: "i".into() },
        ],
        false,
    );
    assert_eq!(
        calls.iter().map(|c| c.method).collect::<Vec<_>>(),
        ["Input.dispatchKeyEvent", "Input.insertText"]
    );
}
