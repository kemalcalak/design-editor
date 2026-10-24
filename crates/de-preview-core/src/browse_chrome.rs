//! Host-shared flow for the APP MODE browser bar (#153).
//!
//! Both hosts paint, hover, press and release the bar — and resolve the
//! preview cursor, a section link's scroll and a switched page's Canvas
//! framing — through these functions so the two arms cannot drift: a
//! host only supplies its
//! canvas rect, its theme and the session. The press / hover / release
//! contract matches the device switcher's: a press arms a control (and
//! seeds hover, so a first click with no move before it still counts),
//! and the release activates it only while the pointer is still on it.

use de_editor_core::editor_ui_state::groups::{PreviewBrowserControl, PreviewState};
use de_editor_ui::widgets::{
    PaintCx, PreviewBrowserBar, PreviewDeviceSwitcher, ScreenSwitcherPills,
};
use de_editor_ui::{Point2D, Rect, RenderBackend, Theme};

use crate::{InvalidationKind, PageScrollRequest, PreviewCursor, PreviewSession};

/// Whether the bar is on screen for this session (APP MODE only).
pub fn visible(session: Option<&PreviewSession>) -> bool {
    session.is_some_and(PreviewSession::is_app_mode)
}

/// Paint the bar. A no-op outside APP MODE.
pub fn paint(
    session: Option<&PreviewSession>,
    state: &PreviewState,
    backend: &mut dyn RenderBackend,
    canvas: Rect,
    theme: &Theme,
) {
    let Some(session) = session.filter(|s| s.is_app_mode()) else {
        return;
    };
    let Some(address) = session.browse_address() else {
        return;
    };
    let bar = PreviewBrowserBar {
        title: &address.title,
        route: &address.route,
        can_go_back: session.can_go_back(),
        can_go_forward: session.can_go_forward(),
        hover: state.browser_hover,
        pressed: state.browser_pressed,
    };
    let mut cx = PaintCx { backend };
    bar.paint(&mut cx, canvas, theme);
}

/// Whether `point` is over the bar.
pub fn contains(session: Option<&PreviewSession>, canvas: Rect, point: Point2D) -> bool {
    visible(session) && PreviewBrowserBar::contains(canvas, point)
}

/// Press at `point`. Returns `true` when the press landed on the bar,
/// so the caller swallows it (the address field included).
pub fn press(
    session: Option<&PreviewSession>,
    state: &mut PreviewState,
    canvas: Rect,
    point: Point2D,
) -> bool {
    if !contains(session, canvas, point) {
        return false;
    }
    let hit = PreviewBrowserBar::hit_test(canvas, point);
    state.browser_pressed = hit;
    if hit.is_some() {
        state.browser_hover = hit;
    }
    true
}

/// Release. Returns `true` when a bar press was armed (consumed),
/// stepping the history when the pointer is still on the pressed button.
pub fn release(session: Option<&mut PreviewSession>, state: &mut PreviewState) -> bool {
    let Some(pressed) = state.browser_pressed.take() else {
        return false;
    };
    if state.browser_hover == Some(pressed) {
        if let Some(session) = session {
            step(session, pressed == PreviewBrowserControl::Forward);
        }
    }
    true
}

/// Track hover. Returns whether the hovered control changed.
pub fn hover(
    session: Option<&PreviewSession>,
    state: &mut PreviewState,
    canvas: Rect,
    point: Point2D,
) -> bool {
    let hit = visible(session)
        .then(|| PreviewBrowserBar::hit_test(canvas, point))
        .flatten();
    if state.browser_hover == hit {
        return false;
    }
    state.browser_hover = hit;
    true
}

/// The pointer shape at `point`: a pointer over the bar's buttons, the
/// default over the rest of the bar, and the session's hover cursor over
/// the previewed page (`over_page`: the host's own "is this the canvas"
/// answer). `None` when the point is neither on the bar nor the page.
pub fn cursor(
    session: &PreviewSession,
    canvas: Rect,
    point: Point2D,
    over_page: bool,
) -> Option<PreviewCursor> {
    if contains(Some(session), canvas, point) {
        return Some(if PreviewBrowserBar::hit_test(canvas, point).is_some() {
            PreviewCursor::Pointer
        } else {
            PreviewCursor::Default
        });
    }
    over_page.then(|| session.preview_cursor())
}

/// How far below the canvas top (canvas-local px) a page or section is
/// placed in Canvas mode, so it starts below the floating preview chrome
/// (device switcher, and in APP MODE the browser bar and screen pills).
fn canvas_top_inset(session: &PreviewSession, canvas: Rect) -> f32 {
    const GAP: f32 = 12.0;
    let bottom = if session.is_app_mode() {
        let row = ScreenSwitcherPills::row_rect(canvas, session.screen_switcher_entries().len());
        row.origin.y + row.size.y
    } else {
        let pill = PreviewDeviceSwitcher::pill_rect(canvas);
        pill.origin.y + pill.size.y
    };
    bottom - canvas.origin.y + GAP
}

/// The canvas a framed (Phone / Desktop) preview fits its device frame
/// into. In APP MODE the browser bar and screen pills sit above the page
/// like a browser's toolbar, so the frame starts below them instead of
/// under them, where they would cover (and swallow clicks on) a site
/// header's links. Outside APP MODE the canvas is unchanged.
pub fn frame_canvas(session: Option<&PreviewSession>, canvas: Rect) -> Rect {
    let Some(session) = session.filter(|s| s.is_app_mode()) else {
        return canvas;
    };
    let inset = canvas_top_inset(session, canvas).min(canvas.size.y);
    Rect {
        origin: Point2D::new(canvas.origin.x, canvas.origin.y + inset),
        size: Point2D::new(canvas.size.x, canvas.size.y - inset),
    }
}

/// Canvas-mode viewport pan `(pan_x, pan_y)` for a newly opened page: centred
/// horizontally; centred vertically when it fits under the chrome, else its
/// top under the chrome — a website opens at its top.
pub fn canvas_pan_for_page(
    session: &PreviewSession,
    page: Rect,
    canvas: Rect,
    zoom: f32,
) -> (f32, f32) {
    let inset = canvas_top_inset(session, canvas);
    let pan_x = canvas.size.x / 2.0 - (page.origin.x + page.size.x / 2.0) * zoom;
    let pan_y = if page.size.y * zoom <= canvas.size.y - inset * 2.0 {
        canvas.size.y / 2.0 - (page.origin.y + page.size.y / 2.0) * zoom
    } else {
        inset - page.origin.y * zoom
    };
    (pan_x, pan_y)
}

/// Canvas-mode `pan_y` that puts a section link's target just below the
/// floating chrome.
pub fn canvas_pan_y_for_section(
    session: &PreviewSession,
    request: &PageScrollRequest,
    canvas: Rect,
    zoom: f32,
) -> f32 {
    canvas_top_inset(session, canvas) - (request.page_rect.origin.y + request.offset) * zoom
}

/// The screen-px delta a framed host feeds `apply_device_scroll` (which
/// subtracts `delta / fit` and clamps) so the page lands at the request.
pub fn device_scroll_delta(request: &PageScrollRequest, current_scroll: f32, fit: f32) -> f32 {
    (current_scroll - request.offset) * fit
}

/// One history step — the bar buttons, `Cmd/Ctrl+[` / `]`, and the mouse
/// back / forward buttons all land here.
pub fn step(session: &mut PreviewSession, forward: bool) -> InvalidationKind {
    if forward {
        session.go_forward()
    } else {
        session.go_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> Rect {
        Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(1200.0, 800.0),
        }
    }

    #[test]
    fn nothing_is_hit_without_an_app_mode_session() {
        let mut state = PreviewState::default();
        let back = PreviewBrowserBar::button_rect(canvas(), PreviewBrowserControl::Back);
        assert!(!press(None, &mut state, canvas(), back.origin));
        assert!(!hover(None, &mut state, canvas(), back.origin));
        assert_eq!(state.browser_pressed, None);
    }

    #[test]
    fn release_without_a_press_is_not_consumed() {
        let mut state = PreviewState::default();
        assert!(!release(None, &mut state));
    }

    #[test]
    fn release_off_the_pressed_button_only_disarms() {
        let mut state = PreviewState {
            browser_pressed: Some(PreviewBrowserControl::Back),
            browser_hover: Some(PreviewBrowserControl::Forward),
            ..PreviewState::default()
        };
        assert!(release(None, &mut state));
        assert_eq!(state.browser_pressed, None);
    }
}
