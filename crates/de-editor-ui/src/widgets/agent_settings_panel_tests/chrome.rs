//! Modal chrome: the close button and the top tab strip.
//!
//! Split out of `agent_settings_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;
use crate::widgets::agent_settings_panel_geometry::{close_rect, nav_item_rect};

fn tab_center(panel: Rect, index: usize, count: usize) -> Point2D {
    let pill = nav_item_rect(panel, index, count);
    Point2D::new(
        pill.origin.x + pill.size.x / 2.0,
        pill.origin.y + pill.size.y / 2.0,
    )
}

#[test]
fn close_button_paints_after_scrollable_content() {
    let state = EditorState::default();
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    let close_origin = close_rect(rect).origin;
    let close_idx = backend
        .icon_strokes
        .iter()
        .find_map(|(at, size, idx)| {
            ((at.x - close_origin.x).abs() < 0.01
                && (at.y - close_origin.y).abs() < 0.01
                && (*size - 16.0).abs() < 0.01)
                .then_some(*idx)
        })
        .expect("close icon should paint");
    let restore_idx = backend
        .ops
        .iter()
        .rposition(|op| *op == "restore")
        .expect("content clip should restore");

    assert!(
        close_idx > restore_idx,
        "close button must paint above clipped, scrollable content"
    );
}

#[test]
fn close_button_hover_paints_visible_wash() {
    let mut state = EditorState::default();
    state.editor_ui.agent_settings.hover_agent_settings_close = true;
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    let close = close_rect(rect);
    assert!(
        backend
            .round_fills
            .iter()
            .any(|(fill, color)| *fill == close && color_eq(*color, panel.theme.button_hover)),
        "hovered close button should paint a visible hover wash"
    );
}

#[test]
fn pressed_close_button_uses_shared_button_feedback() {
    let mut state = EditorState::default();
    state.editor_ui.pressed_button =
        Some(ButtonPressTarget::AgentSettings(AgentSettingsButton::Close));
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    let close = close_rect(rect);
    let expected = panel
        .theme
        .button_hover
        .with_alpha(panel.theme.button_hover.a * 1.8);
    assert!(
        backend
            .round_fills
            .iter()
            .any(|(fill, color)| *fill == close && color_eq(*color, expected)),
        "pressed close button should paint the shared pressed feedback token"
    );
}

#[test]
fn agents_tab_icon_uses_ts_pen_glyph_not_pencil() {
    const PEN_PATH: &str =
        "M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z";
    let state = EditorState::default();
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    // The glyph is centred with its label inside the pill, so assert it
    // lands anywhere inside the first pill rather than re-deriving the
    // centring maths here.
    let pill = nav_item_rect(rect, 0, 5);
    let strokes: Vec<_> = backend
        .svg_strokes
        .iter()
        .filter(|(_, at, size)| (*size - 16.0).abs() < 0.01 && pill.contains(*at))
        .collect();

    assert_eq!(strokes.len(), 1, "TS settings nav uses lucide Pen");
    assert_eq!(strokes[0].0, PEN_PATH);
}

#[test]
fn top_tab_strip_selects_by_pill() {
    let state = EditorState::default();
    let panel = AgentSettingsPanel::for_editor(&state);
    let rect = panel.rect(1200.0, 800.0);

    assert_eq!(
        panel.hit_test(rect, tab_center(rect, 0, 5)),
        AgentSettingsHit::SelectTab(AgentSettingsTab::Agents)
    );
    assert_eq!(
        panel.hit_test(rect, tab_center(rect, 1, 5)),
        AgentSettingsHit::SelectTab(AgentSettingsTab::Mcp)
    );
    // The strip is centred with clear space on both flanks — a press out
    // there stays inside the modal without selecting anything.
    assert_eq!(
        panel.hit_test(
            rect,
            Point2D::new(rect.origin.x + 12.0, rect.origin.y + 30.0)
        ),
        AgentSettingsHit::Inside
    );
}
