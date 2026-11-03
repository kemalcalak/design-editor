//! Mouse-wheel / trackpad dispatch for `DesktopApp` (#303).
//!
//! Every `MouseScrollDelta` goes through the shared
//! `de_editor_ui::widgets::wheel_flow` mapping, the same one the web host
//! uses: no modifier pans the canvas (Shift = horizontal) and scrolls
//! panels, Cmd/Ctrl zooms. Line deltas become pixels first, so a Windows
//! wheel notch or precision-touchpad stroke moves panels and the canvas
//! by the system's lines per notch instead of a flat 16 px. What stays
//! here is the platform part: how many pixels one line is.

use crate::DesktopApp;
use de_editor_ui::widgets::wheel_flow::{
    line_delta_px, route_wheel, NotchScale, WheelRoute, WHEEL_ZOOM_PER_NOTCH,
};
use winit::event::MouseScrollDelta;

/// Which wheel axis a line delta belongs to (Windows keeps separate
/// lines-per-notch and chars-per-notch settings).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WheelAxis {
    Vertical,
    Horizontal,
}

impl DesktopApp {
    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let route = wheel_route(
            delta,
            self.shift_modifier,
            self.zoom_modifier,
            self.dpi,
            (self.viewport_width, self.viewport_height),
            platform_notch_scale,
        );
        let consumed = match route {
            WheelRoute::Pan { dx, dy } => self.host.apply_pan_gesture(
                self.cursor_x,
                self.cursor_y,
                dx,
                dy,
                self.viewport_width,
                self.viewport_height,
            ),
            WheelRoute::Zoom { delta_y } => self.host.apply_pinch_gesture(
                self.cursor_x,
                self.cursor_y,
                delta_y,
                self.viewport_width,
                self.viewport_height,
            ),
        };
        if consumed {
            self.request_redraw(true);
        }
    }
}

/// Map one winit scroll event to a pan or zoom in logical pixels.
///
/// `notch_scale` is injected so tests can pin the platform setting.
/// A zooming line delta keeps the pre-#303 16 per notch, so Ctrl/Cmd+wheel
/// zooms exactly as fast as the plain wheel used to.
pub(crate) fn wheel_route(
    delta: MouseScrollDelta,
    shift: bool,
    zoom: bool,
    dpi: f32,
    (viewport_width, viewport_height): (f32, f32),
    notch_scale: impl Fn(WheelAxis) -> (NotchScale, f32),
) -> WheelRoute {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => {
            let px = |notches: f32, axis: WheelAxis, page: f32| {
                let (scale, line_px) = notch_scale(axis);
                line_delta_px(notches, scale, line_px, page)
            };
            let dx = px(x, WheelAxis::Horizontal, viewport_width);
            let dy = px(y, WheelAxis::Vertical, viewport_height);
            match route_wheel(dx, dy, shift, zoom) {
                WheelRoute::Zoom { .. } => WheelRoute::Zoom {
                    delta_y: y * WHEEL_ZOOM_PER_NOTCH,
                },
                pan => pan,
            }
        }
        MouseScrollDelta::PixelDelta(p) => {
            let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
            route_wheel(p.x as f32 / dpi, p.y as f32 / dpi, shift, zoom)
        }
    }
}

/// macOS reports non-precise wheels in lines that already carry the
/// user's wheel speed, so one line is one `MACOS_WHEEL_LINE_PX`.
#[cfg(target_os = "macos")]
pub(crate) fn platform_notch_scale(_axis: WheelAxis) -> (NotchScale, f32) {
    use de_editor_ui::widgets::wheel_flow::MACOS_WHEEL_LINE_PX;
    (NotchScale::Lines(1.0), MACOS_WHEEL_LINE_PX)
}

/// Windows reports whole notches (`WHEEL_DELTA` = 1.0; a precision
/// touchpad sends fractions) and keeps lines per notch in the system
/// settings, read per event so a change applies at once.
#[cfg(target_os = "windows")]
pub(crate) fn platform_notch_scale(axis: WheelAxis) -> (NotchScale, f32) {
    use de_editor_ui::widgets::wheel_flow::{DEFAULT_WHEEL_LINES_PER_NOTCH, WHEEL_LINE_PX};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES,
    };
    /// `WHEEL_PAGESCROLL`: "one screen at a time".
    const PAGE_SCROLL: u32 = u32::MAX;
    let action = match axis {
        WheelAxis::Vertical => SPI_GETWHEELSCROLLLINES,
        WheelAxis::Horizontal => SPI_GETWHEELSCROLLCHARS,
    };
    let mut lines: u32 = 0;
    // SAFETY: both actions write one `u32` through `pvparam`.
    let ok = unsafe { SystemParametersInfoW(action, 0, (&mut lines as *mut u32).cast(), 0) };
    let scale = match (ok != 0, lines) {
        (true, PAGE_SCROLL) => NotchScale::Page,
        (true, n) if n > 0 => NotchScale::Lines(n as f32),
        _ => NotchScale::Lines(DEFAULT_WHEEL_LINES_PER_NOTCH),
    };
    (scale, WHEEL_LINE_PX)
}

/// Linux and the rest report whole notches with no system setting
/// winit exposes; use the Windows / Chromium default of three lines.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn platform_notch_scale(_axis: WheelAxis) -> (NotchScale, f32) {
    use de_editor_ui::widgets::wheel_flow::{DEFAULT_WHEEL_LINES_PER_NOTCH, WHEEL_LINE_PX};
    (
        NotchScale::Lines(DEFAULT_WHEEL_LINES_PER_NOTCH),
        WHEEL_LINE_PX,
    )
}

#[cfg(test)]
#[path = "wheel_events_tests.rs"]
mod tests;
