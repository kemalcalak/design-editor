//! The Position section's corner smoothing row (#389), under the radius
//! row (and the per-corner grid when it is open): a label, a 0–100 %
//! slider and its value field, Figma's "Corner smoothing". Shown for the
//! kinds that store `cornerSmoothing` — frames, rectangles, polygons and
//! stars, paths — once a corner has a radius (nothing to smooth before).
//!
//! A slider drag is one undo step: the first step snapshots history and
//! the host keeps the drag flag until release (the effect-amount slider's
//! rule).

use crate::theme::Theme;
use crate::widgets::property_panel_inputs::{
    paint_input_with_suffix_focused_state, INPUT_HEIGHT, PAD_X,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::{EditorCommand, EditorState, PropertyFocus};

/// Height the row adds to the Position section (row + gap above it).
pub const SMOOTHING_ROW_HEIGHT: f32 = INPUT_HEIGHT + 6.0;
/// Width of the percentage field.
const VALUE_W: f32 = 56.0;
/// Gap between the slider and the field.
const GAP: f32 = 8.0;

/// The row's parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothingRowRects {
    /// The label column (left half, like the X / rotation column).
    pub label: Rect,
    /// The slider's hit area.
    pub slider: Rect,
    /// The percentage field.
    pub value: Rect,
}

/// The row whose top (above its 6 px gap) is `y`.
pub fn row_rects(x: f32, y: f32, width: f32) -> SmoothingRowRects {
    let top = y + 6.0;
    let usable_w = width - PAD_X * 2.0;
    let half_w = (usable_w - 8.0) / 2.0;
    let right_x = x + PAD_X + half_w + 8.0;
    let value = Rect::xywh(right_x + half_w - VALUE_W, top, VALUE_W, INPUT_HEIGHT);
    SmoothingRowRects {
        label: Rect::xywh(x + PAD_X, top, half_w, INPUT_HEIGHT),
        slider: Rect::xywh(
            right_x,
            top + (INPUT_HEIGHT - 16.0) / 2.0,
            (value.origin.x - GAP - right_x).max(0.0),
            16.0,
        ),
        value,
    }
}

/// The percentage under `x` on `slider`, whole numbers in `0..=100`.
pub fn slider_value(slider: Rect, x: f32) -> f32 {
    crate::widgets::property_panel_effects::slider_value(slider, x)
}

/// Paint the row for `smoothing` (`0..=1`).
pub fn paint(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    label: &str,
    smoothing: f32,
    edit: &EditContext<'_>,
    rects: SmoothingRowRects,
) {
    crate::widgets::property_panel_stroke_style::paint_row_label(
        cx,
        theme,
        label,
        rects.label,
        rects.label.size.x,
    );
    let percent = (smoothing.clamp(0.0, 1.0) * 100.0).round();
    let track = Rect::xywh(
        rects.slider.origin.x,
        rects.slider.origin.y + (rects.slider.size.y - 4.0) / 2.0,
        rects.slider.size.x,
        4.0,
    );
    let filled = track.size.x * percent / 100.0;
    cx.backend.fill_round_rect(track, 2.0, theme.border);
    cx.backend.fill_round_rect(
        Rect::xywh(track.origin.x, track.origin.y, filled, 4.0),
        2.0,
        theme.primary,
    );
    cx.backend.fill_oval(
        Rect::xywh(
            track.origin.x + filled - 5.0,
            track.origin.y - 3.0,
            10.0,
            10.0,
        ),
        theme.primary,
    );
    let focus = PropertyFocus::CornerSmoothing;
    let value = format!("{}", percent as i32);
    paint_input_with_suffix_focused_state(
        cx,
        theme,
        rects.value,
        edit.value_for(focus, &value),
        "%",
        edit.focus == Some(focus),
        edit.caret_at(focus),
        edit.select_all_at(focus),
        edit.input_at(focus),
        edit.now_ms,
    );
}

/// Apply a slider step of `percent` to the selection. The first step of a
/// drag snapshots history; `dragging` stays set until the host's release.
pub fn apply_slider_step(state: &mut EditorState, dragging: &mut bool, percent: f32) {
    let id = state.selection.anchor.clone();
    if !id.is_real() || !percent.is_finite() {
        return;
    }
    if !*dragging {
        state.commit_history();
    }
    *dragging = true;
    let _ = state.apply(EditorCommand::SetNodeCorners {
        node_id: id,
        radii: None,
        smoothing: Some((percent / 100.0).clamp(0.0, 1.0)),
    });
}

#[cfg(test)]
#[path = "property_panel_corner_smoothing_tests.rs"]
mod tests;
