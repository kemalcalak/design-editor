//! Host-shared flow for the APP MODE browser bar (#153).
//!
//! Both hosts paint, hover, press and release the bar through these
//! functions so the two arms cannot drift: a host only supplies its
//! canvas rect, its theme and the session. The press / hover / release
//! contract matches the device switcher's: a press arms a control (and
//! seeds hover, so a first click with no move before it still counts),
//! and the release activates it only while the pointer is still on it.

use de_editor_core::editor_ui_state::groups::{PreviewBrowserControl, PreviewState};
use de_editor_ui::widgets::{PaintCx, PreviewBrowserBar};
use de_editor_ui::{Point2D, Rect, RenderBackend, Theme};

use crate::{InvalidationKind, PreviewSession};

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
