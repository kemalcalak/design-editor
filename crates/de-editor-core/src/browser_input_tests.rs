use super::*;

fn mods(shift: bool) -> InputModifiers {
    InputModifiers {
        shift,
        ..InputModifiers::default()
    }
}

fn mv(x: f64, y: f64, buttons: u8) -> BrowserInputEvent {
    BrowserInputEvent::MouseMove {
        x,
        y,
        buttons,
        modifiers: InputModifiers::default(),
    }
}

fn wheel(dy: f64, shift: bool) -> BrowserInputEvent {
    BrowserInputEvent::Wheel {
        x: 10.0,
        y: 20.0,
        delta_x: 0.0,
        delta_y: dy,
        modifiers: mods(shift),
    }
}

#[test]
fn modifier_bits_follow_cdp() {
    let all = InputModifiers {
        alt: true,
        ctrl: true,
        meta: true,
        shift: true,
    };
    assert_eq!(all.bits(), 15);
    assert_eq!(mods(true).bits(), 8);
    assert_eq!(InputModifiers::default().bits(), 0);
}

#[test]
fn consecutive_moves_fold_into_the_last() {
    let mut batch = InputBatch::new();
    batch.push(mv(1.0, 1.0, 0));
    batch.push(mv(2.0, 2.0, 0));
    batch.push(mv(3.0, 4.0, 0));
    assert_eq!(batch.events(), &[mv(3.0, 4.0, 0)]);
}

#[test]
fn a_move_with_other_buttons_is_kept() {
    let mut batch = InputBatch::new();
    batch.push(mv(1.0, 1.0, 0));
    batch.push(mv(2.0, 2.0, 1));
    assert_eq!(batch.len(), 2);
}

#[test]
fn presses_split_moves_and_are_never_dropped() {
    let mut batch = InputBatch::new();
    batch.push(mv(1.0, 1.0, 0));
    let down = BrowserInputEvent::MouseDown {
        x: 1.0,
        y: 1.0,
        button: MouseButton::Left,
        click_count: 1,
        buttons: 0,
        modifiers: InputModifiers::default(),
    };
    batch.push(down.clone());
    batch.push(mv(5.0, 5.0, 1));
    batch.push(mv(6.0, 6.0, 1));
    assert_eq!(batch.take(), vec![mv(1.0, 1.0, 0), down, mv(6.0, 6.0, 1)]);
    assert!(batch.is_empty());
}

#[test]
fn wheel_steps_add_up_per_modifier_set() {
    let mut batch = InputBatch::new();
    batch.extend([wheel(10.0, false), wheel(15.0, false), wheel(4.0, true)]);
    assert_eq!(batch.events(), &[wheel(25.0, false), wheel(4.0, true)]);
}

#[test]
fn keys_are_kept_in_order() {
    let key = |k: &str| BrowserInputEvent::KeyDown {
        key: k.into(),
        code: format!("Key{}", k.to_uppercase()),
        text: Some(k.into()),
        modifiers: InputModifiers::default(),
    };
    let mut batch = InputBatch::new();
    batch.extend([key("a"), key("a"), key("b")]);
    assert_eq!(batch.len(), 3);
}

#[test]
fn validation_refuses_bad_numbers_and_empty_keys() {
    assert!(mv(1.0, 2.0, 0).is_valid());
    assert!(!mv(f64::NAN, 2.0, 0).is_valid());
    assert!(!wheel(f64::INFINITY, false).is_valid());
    let empty_key = BrowserInputEvent::KeyUp {
        key: String::new(),
        code: "KeyA".into(),
        modifiers: InputModifiers::default(),
    };
    assert!(!empty_key.is_valid());
    let long = BrowserInputEvent::InsertText {
        text: "x".repeat(MAX_INPUT_TEXT_CHARS + 1),
    };
    assert!(!long.is_valid());
    assert!(BrowserInputEvent::InsertText { text: "é".into() }.is_valid());
}

#[test]
fn the_batch_round_trips_as_camel_case_json() {
    let batch = BrowserInputBatch {
        client: "c1".into(),
        key: "n1".into(),
        events: vec![
            BrowserInputEvent::MouseDown {
                x: 1.5,
                y: 2.0,
                button: MouseButton::Left,
                click_count: 2,
                buttons: 0,
                modifiers: mods(true),
            },
            wheel(3.0, false),
            BrowserInputEvent::KeyDown {
                key: "Enter".into(),
                code: "Enter".into(),
                text: None,
                modifiers: InputModifiers::default(),
            },
        ],
    };
    let json = serde_json::to_value(&batch).unwrap();
    assert_eq!(json["events"][0]["type"], "mouseDown");
    assert_eq!(json["events"][0]["clickCount"], 2);
    assert_eq!(json["events"][0]["modifiers"]["shift"], true);
    assert_eq!(json["events"][1]["deltaY"], 3.0);
    assert!(json["events"][2].get("text").is_none());
    let back: BrowserInputBatch = serde_json::from_value(json).unwrap();
    assert_eq!(back, batch);
    // A browser client may leave the defaults out.
    let lean: BrowserInputEvent =
        serde_json::from_str(r#"{"type":"mouseMove","x":1,"y":2}"#).unwrap();
    assert_eq!(lean, mv(1.0, 2.0, 0));
}
