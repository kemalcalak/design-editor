//! [`BrowserInputEvent`]s as Chrome DevTools Protocol calls (#324, phase
//! 4): `Input.dispatchMouseEvent`, `Input.dispatchKeyEvent` and
//! `Input.insertText`. Pure: the desktop driver and the serve-web daemon
//! send the result on the node's own stream tab.
//!
//! Chrome needs a Windows virtual key code to act on a non-text key
//! (Backspace, arrows, Enter), so one is derived from the DOM `code`
//! (else the `key`). Chrome on macOS has no Cocoa key bindings in a
//! headless tab: editing keys and the Command shortcuts do nothing there
//! unless the event names its editing command, so `mac_editing` adds
//! them (pass whether the browser runs on macOS).

use serde_json::{json, Value};

use crate::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};

/// One protocol call.
#[derive(Debug, Clone, PartialEq)]
pub struct CdpCall {
    pub method: &'static str,
    pub params: Value,
}

/// The call that delivers `event` to the page. `mac_editing`: the browser
/// runs on macOS (see the module docs).
pub fn cdp_call(event: &BrowserInputEvent, mac_editing: bool) -> CdpCall {
    match event {
        BrowserInputEvent::MouseMove {
            x,
            y,
            buttons,
            modifiers,
        } => mouse(
            "mouseMoved",
            *x,
            *y,
            held_button(*buttons),
            *buttons,
            None,
            *modifiers,
        ),
        BrowserInputEvent::MouseDown {
            x,
            y,
            button,
            click_count,
            buttons,
            modifiers,
        } => mouse(
            "mousePressed",
            *x,
            *y,
            button_name(*button),
            *buttons | button.bit(),
            Some(*click_count),
            *modifiers,
        ),
        BrowserInputEvent::MouseUp {
            x,
            y,
            button,
            click_count,
            buttons,
            modifiers,
        } => mouse(
            "mouseReleased",
            *x,
            *y,
            button_name(*button),
            *buttons & !button.bit(),
            Some(*click_count),
            *modifiers,
        ),
        BrowserInputEvent::Wheel {
            x,
            y,
            delta_x,
            delta_y,
            modifiers,
        } => CdpCall {
            method: "Input.dispatchMouseEvent",
            params: json!({
                "type": "mouseWheel",
                "x": x,
                "y": y,
                "deltaX": delta_x,
                "deltaY": delta_y,
                "modifiers": modifiers.bits(),
            }),
        },
        BrowserInputEvent::KeyDown {
            key,
            code,
            text,
            modifiers,
        } => key_down(key, code, text.as_deref(), *modifiers, mac_editing),
        BrowserInputEvent::KeyUp {
            key,
            code,
            modifiers,
        } => {
            let mut params = json!({
                "type": "keyUp",
                "key": key,
                "code": code,
                "modifiers": modifiers.bits(),
            });
            add_key_code(&mut params, key, code);
            CdpCall {
                method: "Input.dispatchKeyEvent",
                params,
            }
        }
        BrowserInputEvent::InsertText { text } => CdpCall {
            method: "Input.insertText",
            params: json!({ "text": text }),
        },
    }
}

/// Every event of `events` as calls, in order.
pub fn cdp_calls(events: &[BrowserInputEvent], mac_editing: bool) -> Vec<CdpCall> {
    events.iter().map(|e| cdp_call(e, mac_editing)).collect()
}

fn mouse(
    kind: &str,
    x: f64,
    y: f64,
    button: &str,
    buttons: u8,
    click_count: Option<u32>,
    modifiers: InputModifiers,
) -> CdpCall {
    let mut params = json!({
        "type": kind,
        "x": x,
        "y": y,
        "button": button,
        "buttons": buttons,
        "modifiers": modifiers.bits(),
    });
    if let Some(count) = click_count {
        params["clickCount"] = json!(count.max(1));
    }
    CdpCall {
        method: "Input.dispatchMouseEvent",
        params,
    }
}

fn button_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Middle => "middle",
        MouseButton::Right => "right",
    }
}

/// The button a move drags with (CDP names one), from the held mask.
fn held_button(buttons: u8) -> &'static str {
    [MouseButton::Left, MouseButton::Right, MouseButton::Middle]
        .into_iter()
        .find(|b| buttons & b.bit() != 0)
        .map_or("none", button_name)
}

fn key_down(
    key: &str,
    code: &str,
    text: Option<&str>,
    modifiers: InputModifiers,
    mac_editing: bool,
) -> CdpCall {
    // A Command / Ctrl chord is a shortcut, not typing; but AltGr, which
    // Windows and Linux report as Ctrl+Alt, types (`@`, `{`, `€` on many
    // layouts).
    let alt_gr = modifiers.ctrl && modifiers.alt && !modifiers.meta;
    let shortcut = (modifiers.ctrl || modifiers.meta) && !alt_gr;
    let text = match text {
        _ if shortcut => None,
        Some(text) if !text.is_empty() && !text.chars().all(char::is_control) => Some(text),
        // Enter types a carriage return (a form submits, a textarea
        // breaks the line).
        _ if key == "Enter" => Some("\r"),
        _ => None,
    };
    let mut params = json!({
        "type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
        "key": key,
        "code": code,
        "modifiers": modifiers.bits(),
    });
    if let Some(text) = text {
        params["text"] = json!(text);
        params["unmodifiedText"] = json!(text);
    }
    add_key_code(&mut params, key, code);
    if mac_editing {
        if let Some(command) = mac_editing_command(key, modifiers) {
            params["commands"] = json!([command]);
        }
    }
    CdpCall {
        method: "Input.dispatchKeyEvent",
        params,
    }
}

fn add_key_code(params: &mut Value, key: &str, code: &str) {
    if let Some(vk) = virtual_key_code(code).or_else(|| virtual_key_code_for_key(key)) {
        params["windowsVirtualKeyCode"] = json!(vk);
        params["nativeVirtualKeyCode"] = json!(vk);
    }
}

/// The Windows virtual key code of the DOM `code`.
pub fn virtual_key_code(code: &str) -> Option<u32> {
    if let Some(letter) = code.strip_prefix("Key") {
        return single_ascii(letter)
            .filter(char::is_ascii_uppercase)
            .map(u32::from);
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return single_ascii(digit)
            .filter(char::is_ascii_digit)
            .map(u32::from);
    }
    if let Some(digit) = code.strip_prefix("Numpad") {
        if let Some(d) = single_ascii(digit).and_then(|c| c.to_digit(10)) {
            return Some(96 + d);
        }
    }
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&n).then_some(111 + n);
    }
    Some(match code {
        "Backspace" => 8,
        "Tab" => 9,
        "Enter" | "NumpadEnter" => 13,
        "ShiftLeft" | "ShiftRight" | "Shift" => 16,
        "ControlLeft" | "ControlRight" | "Control" => 17,
        "AltLeft" | "AltRight" | "Alt" => 18,
        "Pause" => 19,
        "CapsLock" => 20,
        "Escape" => 27,
        "Space" | " " => 32,
        "PageUp" => 33,
        "PageDown" => 34,
        "End" => 35,
        "Home" => 36,
        "ArrowLeft" => 37,
        "ArrowUp" => 38,
        "ArrowRight" => 39,
        "ArrowDown" => 40,
        "Insert" => 45,
        "Delete" => 46,
        "MetaLeft" | "Meta" => 91,
        "MetaRight" => 92,
        "ContextMenu" => 93,
        "NumpadMultiply" => 106,
        "NumpadAdd" => 107,
        "NumpadSubtract" => 109,
        "NumpadDecimal" => 110,
        "NumpadDivide" => 111,
        "Semicolon" => 186,
        "Equal" => 187,
        "Comma" => 188,
        "Minus" => 189,
        "Period" => 190,
        "Slash" => 191,
        "Backquote" => 192,
        "BracketLeft" => 219,
        "Backslash" => 220,
        "BracketRight" => 221,
        "Quote" => 222,
        _ => return None,
    })
}

/// A virtual key code from the DOM `key` when the `code` gave none (a
/// letter or digit, or a named key).
fn virtual_key_code_for_key(key: &str) -> Option<u32> {
    match single_ascii(key) {
        Some(c) if c.is_ascii_alphabetic() => Some(u32::from(c.to_ascii_uppercase())),
        Some(c) if c.is_ascii_digit() => Some(u32::from(c)),
        _ => virtual_key_code(key),
    }
}

fn single_ascii(text: &str) -> Option<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii() => Some(c),
        _ => None,
    }
}

/// The macOS editing command a key runs in a text field.
fn mac_editing_command(key: &str, modifiers: InputModifiers) -> Option<&'static str> {
    let select = modifiers.shift;
    if modifiers.meta {
        return Some(match (key.to_ascii_lowercase().as_str(), select) {
            ("a", _) => "selectAll",
            ("c", _) => "copy",
            ("x", _) => "cut",
            ("v", _) => "paste",
            ("z", false) => "undo",
            ("z", true) => "redo",
            ("arrowleft", false) => "moveToBeginningOfLine",
            ("arrowleft", true) => "moveToBeginningOfLineAndModifySelection",
            ("arrowright", false) => "moveToEndOfLine",
            ("arrowright", true) => "moveToEndOfLineAndModifySelection",
            ("arrowup", false) => "moveToBeginningOfDocument",
            ("arrowup", true) => "moveToBeginningOfDocumentAndModifySelection",
            ("arrowdown", false) => "moveToEndOfDocument",
            ("arrowdown", true) => "moveToEndOfDocumentAndModifySelection",
            ("backspace", _) => "deleteToBeginningOfLine",
            _ => return None,
        });
    }
    if modifiers.alt {
        return Some(match (key, select) {
            ("ArrowLeft", false) => "moveWordLeft",
            ("ArrowLeft", true) => "moveWordLeftAndModifySelection",
            ("ArrowRight", false) => "moveWordRight",
            ("ArrowRight", true) => "moveWordRightAndModifySelection",
            ("Backspace", _) => "deleteWordBackward",
            ("Delete", _) => "deleteWordForward",
            _ => return None,
        });
    }
    if modifiers.ctrl {
        return None;
    }
    Some(match (key, select) {
        ("Backspace", _) => "deleteBackward",
        ("Delete", _) => "deleteForward",
        ("ArrowLeft", false) => "moveLeft",
        ("ArrowLeft", true) => "moveLeftAndModifySelection",
        ("ArrowRight", false) => "moveRight",
        ("ArrowRight", true) => "moveRightAndModifySelection",
        ("ArrowUp", false) => "moveUp",
        ("ArrowUp", true) => "moveUpAndModifySelection",
        ("ArrowDown", false) => "moveDown",
        ("ArrowDown", true) => "moveDownAndModifySelection",
        ("Home", false) => "moveToBeginningOfLine",
        ("Home", true) => "moveToBeginningOfLineAndModifySelection",
        ("End", false) => "moveToEndOfLine",
        ("End", true) => "moveToEndOfLineAndModifySelection",
        _ => return None,
    })
}

#[cfg(test)]
#[path = "browser_input_cdp_tests.rs"]
mod tests;
