use de_editor_core::browser_input::InputModifiers;

use super::*;

fn mv(x: f64) -> BrowserInputEvent {
    BrowserInputEvent::MouseMove {
        x,
        y: 1.0,
        buttons: 0,
        modifiers: InputModifiers::default(),
    }
}

fn key(name: &str) -> BrowserInputEvent {
    BrowserInputEvent::KeyDown {
        key: name.into(),
        code: String::new(),
        text: Some(name.into()),
        modifiers: InputModifiers::default(),
    }
}

fn live(_node: &str) -> Option<(String, String)> {
    Some(("c1".into(), "b1#1".into()))
}

fn posted(next: NextSend) -> BrowserInputBatch {
    match next {
        NextSend::Post(batch) => batch,
        other => panic!("expected a post: {other:?}"),
    }
}

#[test]
fn one_request_in_flight_and_the_rest_folds_into_the_next() {
    let mut outbox = InputOutbox::default();
    assert_eq!(outbox.next(live), NextSend::Idle, "nothing waits");
    outbox.queue("b1".into(), vec![key("a")]);
    let first = posted(outbox.next(live));
    assert_eq!((first.client.as_str(), first.key.as_str()), ("c1", "b1#1"));
    assert_eq!(first.events, [key("a")]);

    outbox.queue("b1".into(), vec![mv(1.0), mv(2.0)]);
    outbox.queue("b1".into(), vec![mv(3.0), key("b")]);
    assert_eq!(outbox.next(live), NextSend::Idle, "one in flight");
    outbox.done();
    let second = posted(outbox.next(live));
    assert_eq!(second.events, [mv(3.0), key("b")], "moves folded");
}

#[test]
fn input_waits_for_the_stream_then_gives_up() {
    let mut outbox = InputOutbox::default();
    outbox.queue("b1".into(), vec![key("a")]);
    assert_eq!(outbox.next(|_| None), NextSend::Retry);
    assert_eq!(posted(outbox.next(live)).events, [key("a")]);
    outbox.done();

    outbox.queue("b1".into(), vec![key("b")]);
    for _ in 0..MAX_WAITS {
        assert_eq!(outbox.next(|_| None), NextSend::Retry);
    }
    assert_eq!(outbox.next(|_| None), NextSend::Idle, "dropped");
    assert_eq!(outbox.next(live), NextSend::Idle);
}

#[test]
fn a_batch_never_exceeds_the_daemons_cap_and_a_new_node_drops_stale_input() {
    let mut outbox = InputOutbox::default();
    let many: Vec<_> = (0..MAX_INPUT_EVENTS + 5)
        .map(|i| key(&i.to_string()))
        .collect();
    outbox.queue("b1".into(), many);
    assert_eq!(posted(outbox.next(live)).events.len(), MAX_INPUT_EVENTS);
    outbox.done();
    assert_eq!(posted(outbox.next(live)).events.len(), 5);
    outbox.done();

    outbox.queue("b1".into(), vec![key("old")]);
    outbox.queue("b2".into(), vec![key("new")]);
    assert_eq!(posted(outbox.next(live)).events, [key("new")]);
}
