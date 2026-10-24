//! Hover / press feedback on the panel's icon-only action buttons (#104).
//! The wash paints after the section content, so under the editor chrome
//! theme it has to stay translucent or it hides the button's icon.

use super::{color_eq, RoundFillBackend};
use crate::util::over;
use crate::widgets::home_surface::HomePalette;
use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::visible_for;
use crate::widgets::{PaintCx, Widget};
use crate::{Color, Point2D, Rect};
use de_editor_core::{EditorState, NodeId, ThemeMode};

/// Paint the panel with the Add interaction action hovered and
/// optionally pressed; returns the wash colour painted over it, plus the
/// panel so callers can read its theme.
fn add_button_wash(mode: ThemeMode, pressed: bool) -> (Color, PropertyPanel) {
    let mut state = EditorState::sample();
    state.set_single_selection(NodeId::new("n10"));
    state.editor_ui.theme_mode = mode;
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1600.0),
    };
    let mut panel = PropertyPanel::for_selection(&state).expect("frame panel");
    let (index, button) = sections::action_button_rects_with_fill_picker(
        rect,
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    )
    .into_iter()
    .enumerate()
    .find_map(|(i, (action, r))| {
        matches!(action, PropertyPanelAction::ToggleInteractionMenu(_)).then_some((i, r))
    })
    .expect("frame panel has an Add interaction action");
    panel.action_hover = Some(index);
    panel.action_pressed = pressed.then_some(index);

    let mut backend = RoundFillBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        rect,
    );
    let wash = backend
        .fills
        .iter()
        .rev()
        .find(|(r, _)| *r == button)
        .map(|(_, c)| *c)
        .expect("hovered action paints its feedback wash");
    (wash, panel)
}

#[test]
fn icon_button_wash_stays_translucent_in_both_chrome_themes() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let (hover, panel) = add_button_wash(mode, false);
        assert!(
            color_eq(hover, panel.theme.button_hover),
            "{mode:?}: hover paints the theme's button_hover wash"
        );
        // Pressed scales the alpha by 1.8; keep both well under the point
        // where the wash starts to dim the icon underneath.
        assert!(
            hover.a <= 0.08,
            "{mode:?}: the wash paints over the icon, so it must stay faint (a = {})",
            hover.a
        );

        let (pressed, _) = add_button_wash(mode, true);
        assert!(
            pressed.a > hover.a,
            "{mode:?}: pressed must read darker than hover"
        );

        // Over the rail surface the wash still lands on Home's `subtle`,
        // so rows that paint it under their content keep their look.
        let p = HomePalette::for_mode(mode);
        let shown = over(panel.theme.card, hover);
        for (got, want) in [
            (shown.r, p.subtle.r),
            (shown.g, p.subtle.g),
            (shown.b, p.subtle.b),
        ] {
            assert!(
                (got - want).abs() <= 3.0 / 255.0,
                "{mode:?}: hover over surface drifted from subtle ({got} vs {want})"
            );
        }
    }
}
