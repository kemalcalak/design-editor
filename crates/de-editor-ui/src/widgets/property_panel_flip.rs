//! Flip horizontal / vertical buttons on the right of the Position
//! header (#90). Shift+H / Shift+V run the same core flip.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel::NodeSnapshot;
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_inputs::PAD_X;
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::FlipAxis;

const BUTTON: f32 = 20.0;
const GAP: f32 = 2.0;
const GLYPH: f32 = 14.0;

/// The two buttons, right-aligned in the header row at `section_y`:
/// vertical at the edge, horizontal to its left.
pub(crate) fn flip_button_rects(x: f32, section_y: f32, width: f32) -> [(FlipAxis, Rect); 2] {
    let right = x + width - PAD_X;
    let top = section_y + 1.0;
    [
        (
            FlipAxis::Horizontal,
            Rect::xywh(right - BUTTON * 2.0 - GAP, top, BUTTON, BUTTON),
        ),
        (
            FlipAxis::Vertical,
            Rect::xywh(right - BUTTON, top, BUTTON, BUTTON),
        ),
    ]
}

pub(crate) fn push_flip_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x: f32,
    section_y: f32,
    width: f32,
) {
    for (axis, rect) in flip_button_rects(x, section_y, width) {
        out.push((PropertyPanelAction::FlipSelection(axis), rect));
    }
}

/// Paint both buttons; a mirrored anchor shows its axis active.
pub(crate) fn paint_flip_buttons(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    x: f32,
    section_y: f32,
    width: f32,
) {
    for (axis, rect) in flip_button_rects(x, section_y, width) {
        let (active, icon) = match axis {
            FlipAxis::Horizontal => (snapshot.flip_x, Icon::FlipHorizontal),
            FlipAxis::Vertical => (snapshot.flip_y, Icon::FlipVertical),
        };
        if active {
            cx.backend.fill_round_rect(rect, 5.0, theme.muted);
        }
        let inset = (BUTTON - GLYPH) / 2.0;
        draw_icon(
            cx.backend,
            icon,
            crate::Point2D::new(rect.origin.x + inset, rect.origin.y + inset),
            GLYPH,
            if active {
                theme.foreground
            } else {
                theme.muted_foreground
            },
            1.4,
        );
    }
}
