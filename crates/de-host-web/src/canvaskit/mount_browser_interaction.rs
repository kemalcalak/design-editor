//! DOM input for a browser node in interaction mode (#324, phase 4), the
//! web twin of the desktop's `app_handler/browser_interaction_events.rs`.
//!
//! Every listener here sits on the window in the capture phase, so it runs
//! before the editor's own canvas and window listeners. While a node is
//! interactive, a pointer, wheel or key event the page takes (the shared
//! `BrowserInteraction`, through the host's `browser_*` methods) has its
//! default prevented and its propagation stopped: the canvas cannot pan,
//! zoom or drag the node from inside, and typing reaches no tool or
//! shortcut. Escape and any press off the page leave the mode; that press
//! goes on to the editor as usual. Keys go out in DOM terms (`key`,
//! `code`, the one character a key types); keys inside an IME
//! composition are left to the IME, and its commit (`compositionend` on
//! the hidden input, which the repaint focuses while a node is
//! interactive) goes to the page as `insertText`. A blur releases what the
//! page holds. What the page took is sent at once
//! (`widget_host::browser_input_post::flush`).

use std::cell::RefCell;
use std::rc::Rc;

use de_editor_core::browser_input::{InputModifiers, MouseButton};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CompositionEvent, Event, KeyboardEvent, MouseEvent, WheelEvent};

use super::inner::CkInner;
use crate::listener::{now_ms_perf, now_unix_secs, Listener};
use crate::widget_host::browser_input_post;
use crate::widget_host::PageRoute;

pub(super) fn register_browser_interaction_listeners(
    inner: &Rc<RefCell<CkInner>>,
    window: &web_sys::Window,
    listeners: &mut Vec<Listener>,
) -> Result<(), JsValue> {
    for name in ["mousedown", "mouseup"] {
        let inner = Rc::clone(inner);
        add_capture::<MouseEvent, _>(window, name, listeners, move |evt| {
            on_button(&inner, &evt, name == "mousedown");
        })?;
    }
    {
        let inner = Rc::clone(inner);
        add_capture::<MouseEvent, _>(window, "mousemove", listeners, move |evt| {
            on_move(&inner, &evt);
        })?;
    }
    {
        let inner = Rc::clone(inner);
        add_capture::<WheelEvent, _>(window, "wheel", listeners, move |evt| {
            on_wheel(&inner, &evt);
        })?;
    }
    for name in ["keydown", "keyup"] {
        let inner = Rc::clone(inner);
        add_capture::<KeyboardEvent, _>(window, name, listeners, move |evt| {
            on_key(&inner, &evt, name == "keydown");
        })?;
    }
    {
        let inner = Rc::clone(inner);
        add_capture::<CompositionEvent, _>(window, "compositionend", listeners, move |evt| {
            let text = evt.data().unwrap_or_default();
            if with_active(&inner, |b| b.host.browser_insert_text(&text)) == Some(true) {
                taken(&evt);
            }
        })?;
    }
    {
        // Typing reaches the page through its keys and IME commits; the
        // hidden input's own text must not type into the editor too.
        let inner = Rc::clone(inner);
        add_capture::<Event, _>(window, "beforeinput", listeners, move |evt| {
            if with_active(&inner, |_| ()).is_some() {
                taken(&evt);
            }
        })?;
    }
    {
        let inner = Rc::clone(inner);
        add_capture::<Event, _>(window, "blur", listeners, move |evt| {
            // The window's own blur; a capture listener also sees every
            // element's.
            if evt.event_phase() != Event::AT_TARGET {
                return;
            }
            with_active(&inner, |b| b.host.browser_release_pointer());
        })?;
    }
    Ok(())
}

/// Register `handler` for `name` on `window` in the capture phase, not
/// passive (a wheel listener on the window is passive by default, which
/// would let the browser zoom the tab).
fn add_capture<E, F>(
    window: &web_sys::Window,
    name: &'static str,
    listeners: &mut Vec<Listener>,
    mut handler: F,
) -> Result<(), JsValue>
where
    E: JsCast + 'static,
    F: FnMut(E) + 'static,
{
    let closure: Closure<dyn FnMut(JsValue)> = Closure::new(move |raw: JsValue| {
        if let Ok(event) = raw.dyn_into::<E>() {
            handler(event);
        }
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_capture(true);
    options.set_passive(false);
    window.add_event_listener_with_callback_and_add_event_listener_options(
        name,
        closure.as_ref().unchecked_ref(),
        &options,
    )?;
    listeners.push(Listener { closure });
    Ok(())
}

/// Run `f` while a node is interactive (and the shell is free), then send
/// what the page took. `None` when no node is interactive.
fn with_active<T>(inner: &Rc<RefCell<CkInner>>, f: impl FnOnce(&mut CkInner) -> T) -> Option<T> {
    let mut b = inner.try_borrow_mut().ok()?;
    if !b.host.browser_interaction_active() {
        return None;
    }
    b.host.set_clocks(now_ms_perf(), now_unix_secs());
    let out = f(&mut b);
    // A press that ended the mode may have queued releases too.
    browser_input_post::flush(&mut b.host);
    Some(out)
}

/// The page took `evt`: nothing else sees it, nor does the browser act.
fn taken(evt: &Event) {
    evt.prevent_default();
    evt.stop_immediate_propagation();
}

fn modifiers(alt: bool, ctrl: bool, meta: bool, shift: bool) -> InputModifiers {
    InputModifiers {
        alt,
        ctrl,
        meta,
        shift,
    }
}

fn mouse_modifiers(evt: &MouseEvent) -> InputModifiers {
    modifiers(
        evt.alt_key(),
        evt.ctrl_key(),
        evt.meta_key(),
        evt.shift_key(),
    )
}

/// `evt`'s position in the host's logical coordinates (the canvas fills
/// the window; a captured drag can leave it).
fn logical_point(b: &CkInner, evt: &MouseEvent) -> (f32, f32) {
    let rect = b.canvas.get_bounding_client_rect();
    b.event_offset_to_logical(
        (f64::from(evt.client_x()) - rect.left()) as f32,
        (f64::from(evt.client_y()) - rect.top()) as f32,
    )
}

fn page_button(button: i16) -> Option<MouseButton> {
    match button {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Middle),
        2 => Some(MouseButton::Right),
        _ => None,
    }
}

fn on_button(inner: &Rc<RefCell<CkInner>>, evt: &MouseEvent, pressed: bool) {
    let Some(button) = page_button(evt.button()) else {
        return;
    };
    let route = with_active(inner, |b| {
        let (w, h) = b.backend.logical_size();
        let (x, y) = logical_point(b, evt);
        let mods = mouse_modifiers(evt);
        b.host
            .browser_pointer_button(pressed, button, x, y, w, h, mods)
    });
    match route {
        Some(PageRoute::Page) => taken(evt),
        Some(PageRoute::Left) => crate::repaint_coalescer::request(),
        Some(PageRoute::Editor) | None => {}
    }
}

fn on_move(inner: &Rc<RefCell<CkInner>>, evt: &MouseEvent) {
    let took = with_active(inner, |b| {
        let (w, h) = b.backend.logical_size();
        let (x, y) = logical_point(b, evt);
        let mods = mouse_modifiers(evt);
        b.host.browser_pointer_move(x, y, w, h, mods)
    });
    if took == Some(true) {
        taken(evt);
    }
}

fn on_wheel(inner: &Rc<RefCell<CkInner>>, evt: &WheelEvent) {
    use crate::event::pointer::{classify_wheel_intent, WheelIntent};

    let took = with_active(inner, |b| {
        let (w, h) = b.backend.logical_size();
        let (x, y) = logical_point(b, evt);
        // Over the page a wheel always scrolls it: Cmd / Ctrl does not
        // zoom the canvas there, so only Shift (horizontal) is routed.
        let mut routing = de_editor_ui::Modifiers::empty();
        routing.set(de_editor_ui::Modifiers::SHIFT, evt.shift_key());
        let intent = classify_wheel_intent(
            evt.delta_x() as f32,
            evt.delta_y() as f32,
            evt.delta_mode(),
            w,
            h,
            routing,
        );
        let WheelIntent::Pan { dx, dy } = intent else {
            return false;
        };
        let mods = mouse_modifiers(evt);
        b.host.browser_wheel(x, y, dx, dy, w, h, mods)
    });
    if took == Some(true) {
        taken(evt);
    }
}

/// The text a key types: its `key` when that is one printable character.
fn typed_text(key: &str) -> Option<String> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(key.to_string()),
        _ => None,
    }
}

fn on_key(inner: &Rc<RefCell<CkInner>>, evt: &KeyboardEvent, pressed: bool) {
    // A composition owns its keys; its commit arrives as `compositionend`.
    // They still must not reach the editor's shortcuts.
    if evt.is_composing() || evt.key() == "Process" {
        if with_active(inner, |_| ()).is_some() {
            evt.stop_immediate_propagation();
        }
        return;
    }
    let route = with_active(inner, |b| {
        let key = evt.key();
        let text = typed_text(&key);
        let mods = modifiers(
            evt.alt_key(),
            evt.ctrl_key(),
            evt.meta_key(),
            evt.shift_key(),
        );
        b.host.browser_key(key, evt.code(), text, mods, pressed)
    });
    match route {
        Some(PageRoute::Page) => taken(evt),
        Some(PageRoute::Left) => {
            taken(evt);
            crate::repaint_coalescer::request();
        }
        Some(PageRoute::Editor) | None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::typed_text;

    #[test]
    fn a_key_types_only_one_printable_character() {
        assert_eq!(typed_text("a").as_deref(), Some("a"));
        assert_eq!(typed_text("é").as_deref(), Some("é"));
        assert_eq!(typed_text(" ").as_deref(), Some(" "));
        assert_eq!(typed_text("Enter"), None);
        assert_eq!(typed_text("ArrowLeft"), None);
        assert_eq!(typed_text("Dead"), None);
        assert_eq!(typed_text(""), None);
    }
}
