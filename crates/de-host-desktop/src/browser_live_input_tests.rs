use std::cell::RefCell;
use std::rc::Rc;

use de_editor_core::browser_input::{InputModifiers, MouseButton};

use super::*;

/// Records each call's `type`; fails the call at `fail_at` (counted over
/// every connection) with a closed connection.
#[derive(Clone, Default)]
struct Fake {
    sent: Rc<RefCell<Vec<String>>>,
    fail_at: Option<usize>,
    /// Calls of this `type` are refused by the page (a protocol error).
    refuse: Option<&'static str>,
}

impl InputConnection for Fake {
    fn dispatch(
        &mut self,
        _method: &str,
        params: serde_json::Value,
    ) -> Result<(), BrowserRenderError> {
        let mut sent = self.sent.borrow_mut();
        if self.refuse.is_some() && params["type"].as_str() == self.refuse {
            return Err(BrowserRenderError::Protocol {
                method: "Input.dispatchMouseEvent".into(),
                message: "refused".into(),
            });
        }
        if self.fail_at == Some(sent.len()) {
            self.fail_at = None;
            return Err(BrowserRenderError::Closed);
        }
        sent.push(params["type"].as_str().unwrap_or("").to_string());
        Ok(())
    }
}

fn press(down: bool) -> BrowserInputEvent {
    let (x, y, button, click_count, buttons) = (1.0, 2.0, MouseButton::Left, 1, 0);
    let modifiers = InputModifiers::default();
    if down {
        BrowserInputEvent::MouseDown {
            x,
            y,
            button,
            click_count,
            buttons,
            modifiers,
        }
    } else {
        BrowserInputEvent::MouseUp {
            x,
            y,
            button,
            click_count,
            buttons,
            modifiers,
        }
    }
}

fn wheel() -> BrowserInputEvent {
    BrowserInputEvent::Wheel {
        x: 1.0,
        y: 1.0,
        delta_x: 0.0,
        delta_y: 5.0,
        modifiers: InputModifiers::default(),
    }
}

#[test]
fn a_failed_call_keeps_the_release_for_the_next_connection() {
    let fake = Fake {
        fail_at: Some(1),
        ..Fake::default()
    };
    let sent = Rc::clone(&fake.sent);
    let mut sender = InputSender::<Fake>::default();
    // The press goes out, the wheel step fails: the release must follow.
    sender.queue(vec![press(true), wheel(), press(false)]);
    sender.send("n1", || Ok(fake.clone()));
    assert_eq!(*sent.borrow(), ["mousePressed"]);
    assert_eq!(sender.pending(), &[press(false)]);
    // Not before the pause.
    sender.send("n1", || panic!("too soon"));
    sender.retry_at = Some(Instant::now());
    let healthy = Fake {
        sent: Rc::clone(&sent),
        ..Fake::default()
    };
    sender.send("n1", || Ok(healthy));
    assert_eq!(*sent.borrow(), ["mousePressed", "mouseReleased"]);
    assert!(sender.pending().is_empty());
}

#[test]
fn a_connection_that_does_not_open_keeps_the_release() {
    let mut sender = InputSender::<Fake>::default();
    sender.queue(vec![wheel(), press(false)]);
    sender.send("n1", || Err(BrowserRenderError::Closed));
    assert_eq!(sender.pending(), &[press(false)]);
    // A later stream gets it; a new connection is opened for it.
    sender.disconnect();
    let fake = Fake::default();
    let sent = Rc::clone(&fake.sent);
    sender.send("n1", || Ok(fake.clone()));
    assert_eq!(*sent.borrow(), ["mouseReleased"]);
}

#[test]
fn a_full_queue_keeps_the_release() {
    let mut sender = InputSender::<Fake>::default();
    sender.queue(vec![press(true)]);
    let mut wheels: Vec<_> = (0..MAX_QUEUED_INPUT_EVENTS)
        .map(|i| {
            // Alternating modifiers keep the steps from folding together.
            let mut step = wheel();
            if let BrowserInputEvent::Wheel { modifiers, .. } = &mut step {
                modifiers.shift = i % 2 == 0;
            }
            step
        })
        .collect();
    wheels.push(press(false));
    sender.queue(wheels);
    // The click was never sent: its release alone is left.
    assert_eq!(sender.pending(), &[press(false)]);
    // A release whose press already went out is kept.
    let mut sender = InputSender::<Fake>::default();
    let mut many: Vec<_> = (0..MAX_QUEUED_INPUT_EVENTS)
        .map(|i| BrowserInputEvent::InsertText {
            text: i.to_string(),
        })
        .collect();
    many.push(press(false));
    sender.queue(many);
    assert_eq!(sender.pending(), &[press(false)]);
}

#[test]
fn a_call_the_page_refuses_is_skipped_not_retried() {
    let fake = Fake {
        refuse: Some("mouseWheel"),
        ..Fake::default()
    };
    let sent = Rc::clone(&fake.sent);
    let mut sender = InputSender::<Fake>::default();
    sender.queue(vec![press(true), wheel(), press(false)]);
    sender.send("n1", || Ok(fake.clone()));
    assert_eq!(*sent.borrow(), ["mousePressed", "mouseReleased"]);
    assert!(sender.pending().is_empty());
    assert!(sender.retry_at.is_none());
}
