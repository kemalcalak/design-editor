//! Page input against a real browser (`--ignored`): run with an installed
//! Chromium-based browser (or `DE_BROWSER_PATH`):
//! `cargo test -p de-browser-render -- --ignored`.

use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_core::browser_input_cdp::cdp_calls;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::*;

/// Open `html` in a real stream laid out at `viewport` and send `events`
/// through its input connection, the way the hosts do.
fn drive(html: &str, viewport: crate::Viewport, events: &[BrowserInputEvent]) -> Driven {
    let url = format!(
        "data:text/html,{}",
        html.replace(' ', "%20").replace('#', "%23")
    );
    // One browser for every test here: two starting at once can collide.
    let pool = crate::BrowserPool::shared();
    let stream = crate::PageStream::open(&pool, &url, viewport, crate::StreamSettings::default())
        .expect("stream opens");
    let mut input = stream.input().expect("input attaches");
    for call in cdp_calls(events, cfg!(target_os = "macos")) {
        input
            .dispatch(call.method, call.params)
            .expect("dispatched");
    }
    Driven { stream, input }
}

struct Driven {
    stream: crate::PageStream,
    input: PageInput,
}

impl Driven {
    fn read(&mut self, expression: &str) -> Value {
        self.input
            .tab_command(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true }),
            )
            .expect("evaluates")["result"]["value"]
            .clone()
    }

    fn stop(self) {
        drop(self.input);
        self.stream.stop();
    }
}

/// A left click (press and release) at `x`, `y`.
fn click(x: f64, y: f64) -> [BrowserInputEvent; 2] {
    let none = InputModifiers::default();
    [
        BrowserInputEvent::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            click_count: 1,
            buttons: 0,
            modifiers: none,
        },
        BrowserInputEvent::MouseUp {
            x,
            y,
            button: MouseButton::Left,
            click_count: 1,
            buttons: 1,
            modifiers: none,
        },
    ]
}

/// Typing `h`, `i`, Backspace and an inserted `!`: the field reads `h!`.
fn typing() -> Vec<BrowserInputEvent> {
    let none = InputModifiers::default();
    let mut events = Vec::new();
    for (key, code) in [("h", "KeyH"), ("i", "KeyI")] {
        events.push(BrowserInputEvent::KeyDown {
            key: key.into(),
            code: code.into(),
            text: Some(key.into()),
            modifiers: none,
        });
        events.push(BrowserInputEvent::KeyUp {
            key: key.into(),
            code: code.into(),
            modifiers: none,
        });
    }
    events.push(BrowserInputEvent::KeyDown {
        key: "Backspace".into(),
        code: "Backspace".into(),
        text: None,
        modifiers: none,
    });
    events.push(BrowserInputEvent::InsertText { text: "!".into() });
    events
}

#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_click_and_typing_reach_the_page() {
    let html = "<button id=b style='position:absolute;left:10px;top:10px;width:100px;height:40px'\
                onclick=\"alert('hi');document.title='clicked'\">Go</button>\
                <input id=f style='position:absolute;left:10px;top:80px;width:200px'>";
    let mut events: Vec<_> = click(50.0, 30.0).into();
    events.extend(click(60.0, 90.0));
    events.extend(typing());
    let viewport = crate::Viewport {
        width: 400,
        height: 300,
        scale: 1.0,
        mobile: false,
    };
    let mut page = drive(html, viewport, &events);
    assert_eq!(page.read("document.title"), json!("clicked"));
    assert_eq!(page.read("document.getElementById('f').value"), json!("h!"));
    page.stop();
}

/// The Tablet preset's tab (768 × 1024, mobile, touch emulated) on a page
/// with no `width=device-width` viewport: Chrome lays it out 980 px wide
/// and shows it scaled down (768 / 980), so the frame the user clicks on
/// is the visual viewport, not CSS px. A click aimed at a field where the
/// frame shows it focuses that field and typing reaches it.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_click_focuses_and_types_on_a_tablet_tab() {
    let html = "<div style='height:3000px'></div>\
                <input id=f style='position:absolute;left:600px;top:1000px;width:200px;height:30px'>";
    // Where the frame shows the field's inside (CSS 610, 1010).
    let shown = 768.0 / 980.0;
    let mut events: Vec<_> = click(610.0 * shown, 1010.0 * shown).into();
    events.extend(typing());
    let viewport = crate::Viewport {
        width: 768,
        height: 1024,
        scale: 2.0,
        mobile: true,
    };
    let mut page = drive(html, viewport, &events);
    assert_eq!(page.read("innerWidth"), json!(980), "laid out wider");
    assert_eq!(
        page.read("document.activeElement && document.activeElement.id"),
        json!("f"),
        "the click focused the field"
    );
    assert_eq!(page.read("document.getElementById('f').value"), json!("h!"));
    // A wheel step still scrolls the page, one frame px per px.
    let wheel = BrowserInputEvent::Wheel {
        x: 300.0,
        y: 300.0,
        delta_x: 0.0,
        delta_y: 200.0,
        modifiers: InputModifiers::default(),
    };
    for call in cdp_calls(&[wheel], false) {
        page.input
            .dispatch(call.method, call.params)
            .expect("dispatched");
    }
    // Wait until the scroll settles (two equal reads after it moved).
    let deadline = Instant::now() + Duration::from_secs(3);
    let (mut last, mut scrolled) = (-1.0, 0.0);
    while (scrolled == 0.0 || scrolled != last) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        last = scrolled;
        scrolled = page.read("scrollY").as_f64().unwrap_or(0.0);
    }
    // 200 frame px are 200 / (768 / 980) CSS px.
    assert!(
        (scrolled - 200.0 / shown).abs() < 2.0,
        "the page scrolled {scrolled}"
    );
    page.stop();
}
