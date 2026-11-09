//! A browser node in interaction mode takes the window's input (#324,
//! phase 4). `window_event` offers every event here first; while a node
//! is interactive, pointer, wheel and key events over its page go to the
//! page (through `WidgetHostNative`'s shared routing) instead of the
//! editor, so the canvas cannot pan, zoom or drag the node from inside.
//! Escape or any press off the page (canvas or chrome) leaves the mode. The
//! queued input is handed to the node's stream driver, which sends it over
//! CDP off the UI thread. IME composition is not enabled for the page yet
//! (the window keeps IME off while no editor field has focus), so keys
//! arrive with their text and go to the page as key events; an IME commit
//! that does arrive becomes `insertText`.

use de_editor_core::browser_input::MouseButton as PageButton;
use de_editor_core::host_wheel_routing::WheelRoute;
use de_editor_ui::widgets::browser_interact_flow::{ButtonOutcome, KeyOutcome};
use winit::event::{ElementState, MouseButton, WindowEvent};

use super::wheel_events::{platform_notch_scale, wheel_route};
use crate::browser_input_keys::{input_modifiers, key_input};
use crate::DesktopApp;

#[path = "browser_preview_events.rs"]
mod preview;

impl DesktopApp {
    /// Route `event` to a preview page (in Preview) or to the interactive
    /// browser page (canvas interaction mode). True when the page took it
    /// (the editor must not see it).
    pub(super) fn intercept_browser_interaction(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::ModifiersChanged(mods) = event {
            // Tracked always; never consumed (the editor tracks its own).
            self.page_modifiers = input_modifiers(mods.state());
            return false;
        }
        if self.host.preview_pages_active() {
            // Preview pages are embedded pages: no mode (#324).
            let consumed = self.intercept_preview_page(event);
            self.flush_browser_input();
            return consumed;
        }
        if !self.host.browser_interaction_active() {
            return false;
        }
        let consumed = match event {
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x as f32 / self.dpi, position.y as f32 / self.dpi);
                let taken = self.host.browser_pointer_move(
                    x,
                    y,
                    self.viewport_width,
                    self.viewport_height,
                    self.page_modifiers,
                );
                if taken {
                    self.cursor_x = x;
                    self.cursor_y = y;
                    // The page's own cursor is not mirrored: a plain arrow
                    // says the pointer drives the page, not the canvas.
                    if let Some(window) = self.window.as_ref() {
                        window.set_cursor(winit::window::CursorIcon::Default);
                    }
                }
                taken
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some(button) = page_button(*button) else {
                    return false;
                };
                let outcome = self.host.browser_pointer_button(
                    *state == ElementState::Pressed,
                    button,
                    self.cursor_x,
                    self.cursor_y,
                    self.viewport_width,
                    self.viewport_height,
                    self.page_modifiers,
                );
                if outcome == ButtonOutcome::Left {
                    self.request_redraw(true);
                }
                outcome == ButtonOutcome::Page
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Over the page a wheel always scrolls it: Cmd / Ctrl does
                // not zoom the canvas there.
                let route = wheel_route(
                    *delta,
                    self.shift_modifier,
                    false,
                    self.dpi,
                    (self.viewport_width, self.viewport_height),
                    platform_notch_scale,
                );
                let WheelRoute::Pan { dx, dy } = route else {
                    return false;
                };
                self.host.browser_wheel(
                    self.cursor_x,
                    self.cursor_y,
                    dx,
                    dy,
                    self.viewport_width,
                    self.viewport_height,
                    self.page_modifiers,
                )
            }
            // A pinch over the page must not zoom the canvas under it.
            WindowEvent::PinchGesture { .. } => self.host.browser_over_page(
                self.cursor_x,
                self.cursor_y,
                self.viewport_width,
                self.viewport_height,
            ),
            WindowEvent::KeyboardInput { event, .. } => {
                let input = key_input(event, self.page_modifiers);
                let pressed = event.state == ElementState::Pressed;
                match self.host.browser_key(input, pressed) {
                    KeyOutcome::Editor => false,
                    KeyOutcome::Page => true,
                    KeyOutcome::Left => {
                        self.request_redraw(true);
                        true
                    }
                }
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.host.browser_insert_text(text)
            }
            _ => false,
        };
        // A press that ended the mode may have queued releases too.
        self.flush_browser_input();
        consumed
    }

    /// Hand the queued page input to the node's stream driver.
    pub(super) fn flush_browser_input(&mut self) {
        if let Some((node_id, events)) = self.host.take_browser_input() {
            self.browser_live.send_input(&node_id, events);
        }
    }
}

fn page_button(button: MouseButton) -> Option<PageButton> {
    match button {
        MouseButton::Left => Some(PageButton::Left),
        MouseButton::Right => Some(PageButton::Right),
        MouseButton::Middle => Some(PageButton::Middle),
        _ => None,
    }
}
