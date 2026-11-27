//! The chat's permission prompt (#398): when "Ask before destructive agent
//! actions" is on and the agent calls a tool that deletes or overwrites
//! something, its call waits here — a sentence naming what it would do and
//! Allow / Deny buttons — in the band above the input that the MCP notice
//! uses (the prompt wins when both apply: the turn is blocked on it).
//!
//! The buttons are sized from their labels, so the layout is written once
//! against `LabelWidths`: paint measures, the backend-less hit-test reads
//! the kept widths.

use crate::widgets::label_widths::LabelWidths;
use crate::widgets::{text_metrics, PaintCx};
use crate::{Point2D, Rect, TextLayout, Theme};

const PAD_X: f32 = 8.0;
const GAP: f32 = 6.0;
const BUTTON_H: f32 = 22.0;
const BUTTON_PAD_X: f32 = 10.0;
const FONT_SIZE: f32 = 11.0;
const BUTTON_FONT: f32 = 11.5;
const RADIUS: f32 = 6.0;

/// What the prompt shows for one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PermissionView {
    pub(crate) sentence: String,
    pub(crate) allow: String,
    pub(crate) deny: String,
}

/// The prompt for the active chat tab, or `None` when no call is waiting
/// (an answered prompt is gone from view until the host consumes it).
pub(crate) fn permission_view(state: &de_editor_core::EditorState) -> Option<PermissionView> {
    let prompt = state
        .chat
        .permission_prompt
        .as_ref()
        .filter(|prompt| prompt.decision.is_none())?;
    let locale = state.editor_ui.effective_locale();
    Some(PermissionView {
        sentence: prompt.sentence(locale),
        allow: de_i18n::translate(locale, "chat.permission.allow").to_string(),
        deny: de_i18n::translate(locale, "chat.permission.deny").to_string(),
    })
}

/// Where the two buttons sit inside the band `row`, right-aligned: Deny,
/// then Allow at the far right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PermissionLayout {
    pub(crate) allow: Rect,
    pub(crate) deny: Rect,
    /// Room left for the sentence, from the row's left padding.
    pub(crate) text_right: f32,
}

pub(crate) fn permission_layout_with(
    row: Rect,
    view: &PermissionView,
    widths: &mut LabelWidths<'_>,
) -> PermissionLayout {
    let button = |label: &str, widths: &mut LabelWidths<'_>| {
        widths.chrome(label, BUTTON_FONT) + BUTTON_PAD_X * 2.0
    };
    let allow_w = button(&view.allow, widths);
    let deny_w = button(&view.deny, widths);
    let y = row.origin.y + (row.size.y - BUTTON_H) / 2.0;
    let right = row.origin.x + row.size.x - PAD_X;
    let allow = Rect::xywh(right - allow_w, y, allow_w, BUTTON_H);
    let deny = Rect::xywh(allow.origin.x - GAP - deny_w, y, deny_w, BUTTON_H);
    PermissionLayout {
        allow,
        deny,
        text_right: deny.origin.x - GAP,
    }
}

/// `Some(true)` on Allow, `Some(false)` on Deny, `None` elsewhere.
pub(crate) fn permission_hit(row: Rect, view: &PermissionView, point: Point2D) -> Option<bool> {
    let layout = permission_layout_with(row, view, &mut LabelWidths::cached());
    if layout.allow.contains(point) {
        Some(true)
    } else if layout.deny.contains(point) {
        Some(false)
    } else {
        None
    }
}

fn label(cx: &mut PaintCx<'_>, text: &str, size: f32, color: crate::Color, x: f32, rect: Rect) {
    let layout = TextLayout::single_run(
        text,
        text_metrics::CHROME_FONT_FAMILY,
        size,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(x, jian_widgets::centered_text_baseline_y(rect, size)),
    );
}

/// Paint the band: the sentence on an accent-tinted surface, Deny as an
/// outlined button and Allow as the filled one.
pub(crate) fn paint_permission_prompt(
    cx: &mut PaintCx<'_>,
    row: Rect,
    theme: Theme,
    view: &PermissionView,
) {
    if row.size.x <= 0.0 || row.size.y <= 0.0 {
        return;
    }
    let layout = permission_layout_with(row, view, &mut LabelWidths::measuring(&mut *cx.backend));
    cx.backend
        .fill_round_rect(row, RADIUS, theme.accent.with_alpha(0.10));
    cx.backend
        .stroke_round_rect(row, RADIUS, theme.accent.with_alpha(0.45), 1.0);
    let text_x = row.origin.x + PAD_X;
    let room = (layout.text_right - text_x).max(0.0);
    let sentence = text_metrics::fit_chrome(cx.backend, &view.sentence, room, FONT_SIZE);
    label(cx, &sentence, FONT_SIZE, theme.foreground, text_x, row);

    cx.backend
        .stroke_round_rect(layout.deny, RADIUS, theme.border, 1.0);
    let deny_x = text_metrics::centered_text_x(cx.backend, &view.deny, BUTTON_FONT, layout.deny);
    label(
        cx,
        &view.deny,
        BUTTON_FONT,
        theme.foreground,
        deny_x,
        layout.deny,
    );

    cx.backend
        .fill_round_rect(layout.allow, RADIUS, theme.accent);
    let allow_x = text_metrics::centered_text_x(cx.backend, &view.allow, BUTTON_FONT, layout.allow);
    label(
        cx,
        &view.allow,
        BUTTON_FONT,
        theme.accent_foreground,
        allow_x,
        layout.allow,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::test_family_gap_backend::FamilyGapBackend;

    fn view() -> PermissionView {
        PermissionView {
            sentence: "Let the agent delete layer “Hero”?".into(),
            allow: "Allow".into(),
            deny: "Deny".into(),
        }
    }

    #[test]
    fn painted_buttons_are_the_ones_hit_tested() {
        crate::widgets::label_widths::reset();
        let row = Rect::xywh(10.0, 100.0, 340.0, 30.0);
        let mut backend = FamilyGapBackend::default();
        let painted =
            permission_layout_with(row, &view(), &mut LabelWidths::measuring(&mut backend));
        let cached = permission_layout_with(row, &view(), &mut LabelWidths::cached());
        assert_eq!(painted, cached);
        let centre = |r: Rect| Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + 10.0);
        assert_eq!(
            permission_hit(row, &view(), centre(painted.allow)),
            Some(true)
        );
        assert_eq!(
            permission_hit(row, &view(), centre(painted.deny)),
            Some(false)
        );
        assert_eq!(
            permission_hit(row, &view(), Point2D::new(row.origin.x + 20.0, 110.0)),
            None
        );
        assert!(painted.allow.origin.x + painted.allow.size.x <= row.origin.x + row.size.x);
        assert!(painted.deny.origin.x + painted.deny.size.x < painted.allow.origin.x);
    }
}
