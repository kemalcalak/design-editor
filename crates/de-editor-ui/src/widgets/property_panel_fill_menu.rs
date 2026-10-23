//! The fill row's `⋯` menu (#83): fill type, blend mode, reorder and
//! remove, moved off the row so each fill is a single line. The type
//! and blend rows are the triggers the existing fill-type and blend
//! pickers anchor to (they look their trigger up in the action walker),
//! so the pickers open from here unchanged.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel_action::{CompositingTarget, PropertyPanelAction};
use crate::widgets::property_panel_inputs::{INPUT_HEIGHT, PAD_X};
use crate::widgets::property_panel_snapshot::FillSummary;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

const MENU_W: f32 = 200.0;
const MENU_PAD: f32 = 6.0;
const ROW_GAP: f32 = 4.0;
const ICON_SIZE: f32 = 14.0;

/// Rects inside an open fill menu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FillMenuRects {
    pub(crate) frame: Rect,
    pub(crate) type_trigger: Rect,
    pub(crate) blend_trigger: Rect,
    pub(crate) move_up: Rect,
    pub(crate) move_down: Rect,
    pub(crate) remove: Rect,
}

/// Menu geometry anchored under the row's `⋯` button, right-aligned to it.
pub(crate) fn fill_menu_rects(menu_button: Rect, panel_width: f32) -> FillMenuRects {
    let w = MENU_W.min(panel_width - PAD_X * 2.0);
    let h = MENU_PAD * 2.0 + INPUT_HEIGHT * 3.0 + ROW_GAP * 2.0;
    let frame = Rect::xywh(
        menu_button.origin.x + menu_button.size.x - w,
        menu_button.origin.y + menu_button.size.y + 4.0,
        w,
        h,
    );
    let inner_x = frame.origin.x + MENU_PAD;
    let inner_w = w - MENU_PAD * 2.0;
    let row_y = |i: f32| frame.origin.y + MENU_PAD + i * (INPUT_HEIGHT + ROW_GAP);
    let button_w = inner_w / 3.0;
    let button = |i: f32| Rect::xywh(inner_x + i * button_w, row_y(2.0), button_w, INPUT_HEIGHT);
    FillMenuRects {
        frame,
        type_trigger: Rect::xywh(inner_x, row_y(0.0), inner_w, INPUT_HEIGHT),
        blend_trigger: Rect::xywh(inner_x, row_y(1.0), inner_w, INPUT_HEIGHT),
        move_up: button(0.0),
        move_down: button(1.0),
        remove: button(2.0),
    }
}

/// True for the actions an open fill menu emits (not its `⋯` toggle).
pub fn is_fill_menu_row(action: &PropertyPanelAction) -> bool {
    matches!(
        action,
        PropertyPanelAction::ToggleFillTypePicker(_)
            | PropertyPanelAction::ToggleCompositingPicker(CompositingTarget::FillBlend(_))
            | PropertyPanelAction::MoveFill { .. }
            | PropertyPanelAction::RemoveFill(_)
    )
}

/// True for every action the fill menu owns, its toggle included.
/// Hosts apply these instead of treating the press as an outside click.
pub fn is_fill_menu_action(action: &PropertyPanelAction) -> bool {
    is_fill_menu_row(action) || matches!(action, PropertyPanelAction::ToggleFillMenu(_))
}

/// Push the open menu's targets for fill `index` of `count`. Reorder
/// targets only exist where the move is possible.
pub(crate) fn push_fill_menu_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    index: usize,
    count: usize,
    menu_button: Rect,
    panel_width: f32,
) {
    let rects = fill_menu_rects(menu_button, panel_width);
    out.push((
        PropertyPanelAction::ToggleFillTypePicker(index),
        rects.type_trigger,
    ));
    out.push((
        PropertyPanelAction::ToggleCompositingPicker(CompositingTarget::FillBlend(index)),
        rects.blend_trigger,
    ));
    if index > 0 {
        out.push((
            PropertyPanelAction::MoveFill {
                from: index,
                to: index - 1,
            },
            rects.move_up,
        ));
    }
    if index + 1 < count {
        out.push((
            PropertyPanelAction::MoveFill {
                from: index,
                to: index + 1,
            },
            rects.move_down,
        ));
    }
    out.push((PropertyPanelAction::RemoveFill(index), rects.remove));
}

/// Paint the open menu for `fill` (row `index` of `count`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_fill_menu(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    locale: de_editor_core::Locale,
    fill: &FillSummary,
    index: usize,
    count: usize,
    menu_button: Rect,
    panel_width: f32,
) {
    let rects = fill_menu_rects(menu_button, panel_width);
    cx.backend.fill_round_rect(rects.frame, 8.0, theme.popover);
    cx.backend
        .stroke_round_rect(rects.frame, 8.0, theme.border, 1.0);
    crate::widgets::property_panel_compositing::paint_trigger(
        cx,
        theme,
        rects.type_trigger,
        crate::widgets::property_panel_fill::fill_type_label(locale, fill.fill_type),
    );
    crate::widgets::property_panel_compositing::paint_fill_trigger(
        cx,
        theme,
        fill.blend_mode.as_ref(),
        locale,
        rects.blend_trigger,
    );
    let buttons = [
        (Icon::ArrowUp, rects.move_up, index > 0),
        (Icon::ArrowDown, rects.move_down, index + 1 < count),
        (Icon::Trash, rects.remove, true),
    ];
    for (icon, rect, enabled) in buttons {
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                rect.origin.x + (rect.size.x - ICON_SIZE) / 2.0,
                rect.origin.y + (rect.size.y - ICON_SIZE) / 2.0,
            ),
            ICON_SIZE,
            if enabled {
                theme.foreground
            } else {
                theme.muted_subtle
            },
            1.4,
        );
    }
}
