//! Wheel / trackpad routing shared by the native and web hosts (#303).
//!
//! Both hosts feed every scroll input through [`route_wheel`] so a mouse
//! wheel, a Windows precision touchpad, a Mac trackpad and a browser
//! wheel all land on the same Figma-style mapping: no modifier pans
//! (vertical, or horizontal with Shift), Ctrl/Cmd zooms. Deltas use the
//! winit / Jian sign convention (positive y = content moves down, i.e.
//! the user scrolled up); the web host flips W3C deltas before calling.
//!
//! Line-unit deltas are converted with [`line_delta_px`] first, so panels
//! scroll by `line height × lines per notch` instead of a flat 16 px.

/// CSS-pixel height of one wheel line on Windows, Linux and the browser
/// host. Chromium's value: with the default three lines per notch one
/// notch moves 100 px, which is what Figma (desktop and web) and every
/// Chromium page do.
pub const WHEEL_LINE_PX: f32 = 100.0 / 3.0;

/// Logical-pixel height of one line of a macOS non-precise wheel event.
/// macOS already applies the user's wheel speed and acceleration to the
/// line count it reports, so this is a plain per-line height (Chromium's
/// `kScrollbarPixelsPerCocoaTick`).
pub const MACOS_WHEEL_LINE_PX: f32 = 40.0;

/// Lines per notch when the OS reports no setting (Windows' default).
pub const DEFAULT_WHEEL_LINES_PER_NOTCH: f32 = 3.0;

/// Canvas zoom delta per wheel notch for Ctrl/Cmd+wheel on the native
/// host. Unchanged from the pre-#303 plain-wheel zoom so zoom speed does
/// not move with this fix (zoom performance is #218).
pub const WHEEL_ZOOM_PER_NOTCH: f32 = 16.0;

/// How far one wheel notch moves, as the platform reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotchScale {
    /// The notch scrolls this many lines.
    Lines(f32),
    /// The notch scrolls one page (Windows "one screen at a time").
    Page,
}

/// What a wheel / trackpad delta should do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelRoute {
    /// Scroll the surface under the cursor; over the canvas, pan it.
    Pan { dx: f32, dy: f32 },
    /// Zoom the canvas around the cursor by this delta.
    Zoom { delta_y: f32 },
}

/// Convert a line-unit delta (`notches` as the OS reports it) to pixels.
///
/// `page_px` is the extent of the scrolled axis, used when the platform
/// asks for page scrolling. Non-finite input yields `0.0` so a bogus
/// event can never poison the viewport.
pub fn line_delta_px(notches: f32, scale: NotchScale, line_px: f32, page_px: f32) -> f32 {
    if !notches.is_finite() {
        return 0.0;
    }
    let px = match scale {
        NotchScale::Lines(lines) if lines.is_finite() && lines > 0.0 => notches * lines * line_px,
        NotchScale::Lines(_) => notches * DEFAULT_WHEEL_LINES_PER_NOTCH * line_px,
        NotchScale::Page if page_px.is_finite() && page_px > 0.0 => notches * page_px,
        NotchScale::Page => notches * DEFAULT_WHEEL_LINES_PER_NOTCH * line_px,
    };
    if px.is_finite() {
        px
    } else {
        0.0
    }
}

/// Route a pixel delta: `zoom` (Ctrl/Cmd, or a browser pinch, which
/// arrives as Ctrl+wheel) zooms; otherwise it pans. Shift turns a
/// vertical-only delta horizontal — Windows and the browser keep a
/// Shift+wheel on the y axis, while macOS already moves it to x.
pub fn route_wheel(dx: f32, dy: f32, shift: bool, zoom: bool) -> WheelRoute {
    if zoom {
        WheelRoute::Zoom { delta_y: dy }
    } else if shift_moves_to_x(dx, shift, zoom) {
        WheelRoute::Pan { dx: dy, dy: 0.0 }
    } else {
        WheelRoute::Pan { dx, dy }
    }
}

/// Whether [`route_wheel`] turns this wheel's y delta into x. Hosts that
/// convert units per axis (page width vs height, Windows lines vs chars)
/// ask first so the y delta is converted as the axis it will move.
pub fn shift_moves_to_x(dx: f32, shift: bool, zoom: bool) -> bool {
    shift && !zoom && dx == 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_default_notch_scrolls_100_px() {
        let px = line_delta_px(1.0, NotchScale::Lines(3.0), WHEEL_LINE_PX, 800.0);
        assert!((px - 100.0).abs() < 1e-3, "{px}");
        let down = line_delta_px(-2.0, NotchScale::Lines(3.0), WHEEL_LINE_PX, 800.0);
        assert!((down + 200.0).abs() < 1e-3, "{down}");
    }

    #[test]
    fn notch_distance_follows_lines_setting_and_page_mode() {
        let px = line_delta_px(1.0, NotchScale::Lines(6.0), WHEEL_LINE_PX, 800.0);
        assert!((px - 200.0).abs() < 1e-3, "{px}");
        assert_eq!(
            line_delta_px(1.0, NotchScale::Page, WHEEL_LINE_PX, 800.0),
            800.0
        );
        // A precision touchpad sends fractions of a notch.
        let fine = line_delta_px(0.25, NotchScale::Lines(3.0), WHEEL_LINE_PX, 800.0);
        assert!((fine - 25.0).abs() < 1e-3, "{fine}");
    }

    #[test]
    fn macos_line_is_one_line_height() {
        let px = line_delta_px(1.0, NotchScale::Lines(1.0), MACOS_WHEEL_LINE_PX, 800.0);
        assert_eq!(px, 40.0);
    }

    #[test]
    fn bogus_settings_and_input_fall_back_safely() {
        let zero_lines = line_delta_px(1.0, NotchScale::Lines(0.0), WHEEL_LINE_PX, 800.0);
        assert!((zero_lines - 100.0).abs() < 1e-3);
        let no_page = line_delta_px(1.0, NotchScale::Page, WHEEL_LINE_PX, 0.0);
        assert!((no_page - 100.0).abs() < 1e-3);
        assert_eq!(
            line_delta_px(f32::NAN, NotchScale::Lines(3.0), WHEEL_LINE_PX, 800.0),
            0.0
        );
        assert_eq!(
            line_delta_px(f32::MAX, NotchScale::Lines(3.0), WHEEL_LINE_PX, 800.0),
            0.0
        );
    }

    #[test]
    fn plain_wheel_pans_vertically() {
        assert_eq!(
            route_wheel(0.0, 100.0, false, false),
            WheelRoute::Pan { dx: 0.0, dy: 100.0 }
        );
    }

    #[test]
    fn shift_wheel_pans_horizontally() {
        assert_eq!(
            route_wheel(0.0, -100.0, true, false),
            WheelRoute::Pan {
                dx: -100.0,
                dy: 0.0
            }
        );
        // macOS already reports Shift+wheel on x: keep it as is.
        assert_eq!(
            route_wheel(40.0, 0.0, true, false),
            WheelRoute::Pan { dx: 40.0, dy: 0.0 }
        );
    }

    #[test]
    fn trackpad_two_axis_pan_is_kept() {
        assert_eq!(
            route_wheel(12.0, -7.0, false, false),
            WheelRoute::Pan { dx: 12.0, dy: -7.0 }
        );
    }

    #[test]
    fn modifier_zooms_even_with_shift() {
        assert_eq!(
            route_wheel(0.0, 16.0, false, true),
            WheelRoute::Zoom { delta_y: 16.0 }
        );
        assert_eq!(
            route_wheel(0.0, -16.0, true, true),
            WheelRoute::Zoom { delta_y: -16.0 }
        );
    }
}
