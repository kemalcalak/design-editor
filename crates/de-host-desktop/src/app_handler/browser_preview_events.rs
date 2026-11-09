//! Preview pages take the window's input like embedded pages (#324).
//! In Preview every live browser page on screen is directly interactive:
//! `intercept_browser_interaction` offers each event here first, and
//! pointer, wheel and key events over a page (not over the preview's
//! chrome) go to it through `WidgetHostNative`'s shared routing instead
//! of the preview. The page last pressed has the keyboard until a press
//! elsewhere or Escape. The caller hands the queued input to the node's
//! stream driver.

use de_editor_core::host_wheel_routing::WheelRoute;
use de_editor_ui::widgets::browser_interact_flow::KeyOutcome;
use winit::event::{ElementState, WindowEvent};

use super::super::wheel_events::{platform_notch_scale, wheel_route};
use super::page_button;
use crate::browser_input_keys::key_input;
use crate::DesktopApp;

impl DesktopApp {
    /// Route `event` to a preview page. True when a page took it (the
    /// preview must not see it).
    pub(super) fn intercept_preview_page(&mut self, event: &WindowEvent) -> bool {
        let (vw, vh) = (self.viewport_width, self.viewport_height);
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x as f32 / self.dpi, position.y as f32 / self.dpi);
                let taken = self
                    .host
                    .preview_page_pointer_move(x, y, vw, vh, self.page_modifiers);
                if taken {
                    self.cursor_x = x;
                    self.cursor_y = y;
                    // A plain arrow: the pointer drives the page.
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
                let taken = self.host.preview_page_pointer_button(
                    *state == ElementState::Pressed,
                    button,
                    self.cursor_x,
                    self.cursor_y,
                    vw,
                    vh,
                    self.page_modifiers,
                );
                if taken {
                    self.request_redraw(true);
                }
                taken
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Over a page a wheel always scrolls it.
                let route = wheel_route(
                    *delta,
                    self.shift_modifier,
                    false,
                    self.dpi,
                    (vw, vh),
                    platform_notch_scale,
                );
                let WheelRoute::Pan { dx, dy } = route else {
                    return false;
                };
                self.host.preview_page_wheel(
                    self.cursor_x,
                    self.cursor_y,
                    dx,
                    dy,
                    vw,
                    vh,
                    self.page_modifiers,
                )
            }
            WindowEvent::PinchGesture { .. } => {
                self.host
                    .preview_over_page(self.cursor_x, self.cursor_y, vw, vh)
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let input = key_input(event, self.page_modifiers);
                let pressed = event.state == ElementState::Pressed;
                match self.host.preview_page_key(input, pressed) {
                    KeyOutcome::Editor => false,
                    KeyOutcome::Page | KeyOutcome::Left => true,
                }
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.host.preview_page_insert_text(text)
            }
            _ => false,
        }
    }
}
