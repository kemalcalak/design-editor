//! The API draft's provider dropdown (#26): every preset, for the ones
//! the Agents list does not feature. Split out of
//! `agent_settings_api_detail.rs` at the 800-line cap.

use super::{ApiLayout, ApiTarget};
use crate::theme::Theme;
use crate::widgets::agent_settings_detail_chrome::{self as chrome, VALUE_FONT};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{AgentSettings, BuiltinAgentPresetMenuTarget};
use de_editor_core::{BuiltinAgentPresetKey, BUILTIN_AGENT_PRESETS};

const PRESET_ROW_H: f32 = 30.0;
const PRESET_MAX_ROWS: usize = 8;
const PRESET_PAD: f32 = 4.0;

/// Whether the draft's provider dropdown is open.
pub(super) fn preset_menu_open(settings: &AgentSettings, target: ApiTarget<'_>) -> bool {
    target.index.is_none()
        && settings.builtin_preset_menu_open == Some(BuiltinAgentPresetMenuTarget::Draft)
}

/// The draft's provider dropdown, under its select.
pub(crate) fn preset_menu_rect(layout: &ApiLayout) -> Option<Rect> {
    let select = layout.provider?;
    let rows = BUILTIN_AGENT_PRESETS.len().min(PRESET_MAX_ROWS) as f32;
    Some(Rect::xywh(
        select.origin.x,
        select.origin.y + select.size.y + 4.0,
        select.size.x,
        rows * PRESET_ROW_H + PRESET_PAD * 2.0,
    ))
}

/// How far the provider dropdown scrolls.
pub(crate) fn preset_scroll_max() -> f32 {
    BUILTIN_AGENT_PRESETS.len().saturating_sub(PRESET_MAX_ROWS) as f32 * PRESET_ROW_H
}

pub(super) fn preset_row_rect(menu: Rect, index: usize, scroll: f32) -> Rect {
    Rect::xywh(
        menu.origin.x + PRESET_PAD,
        menu.origin.y + PRESET_PAD + index as f32 * PRESET_ROW_H - scroll,
        menu.size.x - PRESET_PAD * 2.0,
        PRESET_ROW_H,
    )
}

/// The preset under `point` in the open dropdown.
pub(crate) fn preset_at(
    settings: &AgentSettings,
    layout: &ApiLayout,
    point: Point2D,
) -> Option<BuiltinAgentPresetKey> {
    let menu = preset_menu_rect(layout)?;
    if !menu.contains(point) {
        return None;
    }
    let scroll = settings.builtin_preset_menu_scroll.offset;
    BUILTIN_AGENT_PRESETS
        .iter()
        .enumerate()
        .find(|(index, _)| preset_row_rect(menu, *index, scroll).contains(point))
        .map(|(_, preset)| preset.key)
}

pub(super) fn paint_preset_menu(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    layout: &ApiLayout,
) {
    let Some(menu) = preset_menu_rect(layout) else {
        return;
    };
    cx.backend.fill_round_rect(menu, 8.0, theme.popover);
    cx.backend.stroke_round_rect(menu, 8.0, theme.border, 1.0);
    let scroll = settings.builtin_preset_menu_scroll.offset;
    cx.backend.save();
    cx.backend.clip_rect(Rect::xywh(
        menu.origin.x,
        menu.origin.y + PRESET_PAD,
        menu.size.x,
        menu.size.y - PRESET_PAD * 2.0,
    ));
    let current = settings
        .builtin_agent_draft
        .as_ref()
        .map(|draft| draft.preset);
    for (index, preset) in BUILTIN_AGENT_PRESETS.iter().enumerate() {
        let row = preset_row_rect(menu, index, scroll);
        if settings.builtin_preset_menu_hover == Some(preset.key) {
            cx.backend.fill_round_rect(row, 6.0, theme.muted);
        }
        chrome::text(
            cx,
            preset.display_name,
            VALUE_FONT,
            theme.foreground,
            if current == Some(preset.key) {
                600
            } else {
                400
            },
            crate::widgets::agent_settings_rows::SETTINGS_FONT_FAMILY,
            row.origin.x + 10.0,
            chrome::centred_baseline(row, VALUE_FONT),
        );
        if current == Some(preset.key) {
            draw_icon(
                cx.backend,
                Icon::Check,
                Point2D::new(
                    row.origin.x + row.size.x - 24.0,
                    row.origin.y + (row.size.y - 14.0) / 2.0,
                ),
                14.0,
                theme.primary,
                1.8,
            );
        }
    }
    cx.backend.restore();
}
