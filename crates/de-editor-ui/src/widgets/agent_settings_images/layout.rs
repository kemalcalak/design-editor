//! Shared Images-tab geometry. Touch metrics live here so paint, hit-testing,
//! and scroll height all walk the same boxes while desktop keeps its legacy
//! pixel density.

use super::*;

pub(super) const DESKTOP_TITLE_H: f32 = 36.0;
pub(super) const DESKTOP_ADVANCED_ROW_H: f32 = 24.0;
pub(super) const DESKTOP_SECTION_TITLE_H: f32 = 36.0;
pub(super) const DESKTOP_ROW_H: f32 = 36.0;
pub(super) const DESKTOP_BTN_H: f32 = 28.0;
pub(super) const DESKTOP_PROFILE_ROW_H: f32 = 32.0;
pub(super) const DESKTOP_PROFILE_FIELD_H: f32 = 24.0;
pub(super) const DESKTOP_PROVIDER_OPTION_H: f32 = 24.0;

const SECTION_GAP: f32 = 28.0;
const DESKTOP_SUBTITLE_H: f32 = 22.0;
const DESKTOP_ROW_VGAP: f32 = 10.0;
const DESKTOP_LABEL_W: f32 = 110.0;
const DESKTOP_BODY_GAP: f32 = 14.0;
const DESKTOP_REGISTER_ROW_H: f32 = 36.0;
const DESKTOP_PROFILE_FORM_TOP: f32 = 40.0;
const DESKTOP_PROFILE_TEST_BTN_W: f32 = 56.0;
const DESKTOP_PROFILE_TEST_GAP: f32 = 8.0;

const PROFILE_ROW_GAP: f32 = 6.0;
const PROFILE_ROW_INSET_X: f32 = 8.0;
const PROFILE_LIST_TOP_GAP: f32 = 8.0;
const DESKTOP_TEST_BTN_W: f32 = 56.0;
const DESKTOP_ADD_BTN_W: f32 = 72.0;
const REGISTER_LINK_W: f32 = 220.0;

fn advanced_body_h() -> f32 {
    DESKTOP_SUBTITLE_H
        + DESKTOP_ROW_H
        + DESKTOP_ROW_VGAP
        + DESKTOP_ROW_H
        + DESKTOP_BODY_GAP
        + DESKTOP_REGISTER_ROW_H
}

pub(super) fn image_gen_section_top_for_ui(content: Rect, settings: &AgentSettings) -> f32 {
    let mut y = content.origin.y + DESKTOP_TITLE_H + DESKTOP_ADVANCED_ROW_H;
    if settings.images_advanced_open {
        y += advanced_body_h();
    }
    y + SECTION_GAP
}

pub(in crate::widgets) fn content_height_for_ui(settings: &AgentSettings) -> f32 {
    let mut h = DESKTOP_TITLE_H + DESKTOP_ADVANCED_ROW_H;
    if settings.images_advanced_open {
        h += advanced_body_h();
    }
    h + SECTION_GAP
        + DESKTOP_SECTION_TITLE_H
        + PROFILE_LIST_TOP_GAP
        + profile_list_h_for_ui(settings)
        + 24.0
}

pub(super) fn profile_list_h_for_ui(settings: &AgentSettings) -> f32 {
    if settings.image_gen_profiles.is_empty() {
        80.0
    } else {
        settings
            .image_gen_profiles
            .iter()
            .enumerate()
            .map(|(index, _)| profile_row_h_for_ui(settings, index))
            .sum::<f32>()
            + settings.image_gen_profiles.len().saturating_sub(1) as f32 * PROFILE_ROW_GAP
    }
}

pub(super) fn profile_row_h_for_ui(settings: &AgentSettings, index: usize) -> f32 {
    if !is_editing_profile(settings, index) {
        return DESKTOP_PROFILE_ROW_H;
    }
    DESKTOP_PROFILE_ROW_H + 8.0 + 5.0 * DESKTOP_ROW_H
}

pub(super) fn advanced_toggle_rect_for_ui(content: Rect) -> Rect {
    Rect::xywh(
        content.origin.x,
        content.origin.y + DESKTOP_TITLE_H,
        140.0,
        DESKTOP_ADVANCED_ROW_H,
    )
}

pub(super) fn search_label_rect_for_ui(content: Rect, index: usize) -> Rect {
    let y = content.origin.y + DESKTOP_TITLE_H + DESKTOP_ADVANCED_ROW_H + DESKTOP_SUBTITLE_H;
    let row_y = y + if index == 0 {
        0.0
    } else {
        DESKTOP_ROW_H + DESKTOP_ROW_VGAP
    };
    Rect::xywh(content.origin.x, row_y, DESKTOP_LABEL_W, DESKTOP_ROW_H)
}

pub(super) fn search_field_rect_for_ui(content: Rect, index: usize) -> Rect {
    let label = search_label_rect_for_ui(content, index);
    Rect::xywh(
        content.origin.x + DESKTOP_LABEL_W,
        label.origin.y,
        content.size.x - DESKTOP_LABEL_W,
        DESKTOP_ROW_H,
    )
}

fn register_link_y_for_ui(content: Rect) -> f32 {
    content.origin.y
        + DESKTOP_TITLE_H
        + DESKTOP_ADVANCED_ROW_H
        + DESKTOP_SUBTITLE_H
        + DESKTOP_ROW_H
        + DESKTOP_ROW_VGAP
        + DESKTOP_ROW_H
        + DESKTOP_BODY_GAP
}

pub(super) fn register_link_rect_for_ui(content: Rect) -> Rect {
    let reserved = 0.0;
    Rect::xywh(
        content.origin.x,
        register_link_y_for_ui(content),
        REGISTER_LINK_W.min((content.size.x - reserved).max(44.0)),
        DESKTOP_REGISTER_ROW_H,
    )
}

pub(super) fn test_btn_rect_for_ui(content: Rect, settings: &AgentSettings) -> Rect {
    if !settings.images_advanced_open {
        return Rect::xywh(0.0, 0.0, 0.0, 0.0);
    }
    let y = register_link_y_for_ui(content) + (DESKTOP_REGISTER_ROW_H - DESKTOP_BTN_H) / 2.0;
    Rect::xywh(
        content.origin.x + content.size.x - DESKTOP_TEST_BTN_W,
        y,
        DESKTOP_TEST_BTN_W,
        DESKTOP_BTN_H,
    )
}

pub(super) fn add_btn_rect_for_ui(content: Rect, settings: &AgentSettings) -> Rect {
    let top = image_gen_section_top_for_ui(content, settings);
    Rect::xywh(
        content.origin.x + content.size.x - DESKTOP_ADD_BTN_W,
        top + (DESKTOP_SECTION_TITLE_H - DESKTOP_BTN_H) / 2.0,
        DESKTOP_ADD_BTN_W,
        DESKTOP_BTN_H,
    )
}

pub(super) fn profile_row_rect_for_ui(
    content: Rect,
    settings: &AgentSettings,
    index: usize,
) -> Rect {
    let top = image_gen_section_top_for_ui(content, settings)
        + DESKTOP_SECTION_TITLE_H
        + PROFILE_LIST_TOP_GAP;
    let y = settings
        .image_gen_profiles
        .iter()
        .enumerate()
        .take(index)
        .fold(top, |acc, (i, _)| {
            acc + profile_row_h_for_ui(settings, i) + PROFILE_ROW_GAP
        });
    Rect::xywh(
        content.origin.x + PROFILE_ROW_INSET_X,
        y,
        (content.size.x - PROFILE_ROW_INSET_X * 2.0).max(0.0),
        profile_row_h_for_ui(settings, index),
    )
}

pub(super) fn profile_active_indicator_rect(row: Rect) -> Rect {
    let header_h = DESKTOP_PROFILE_ROW_H;
    let dot = 14.0;
    Rect::xywh(
        row.origin.x + 8.0,
        row.origin.y + (header_h - dot) / 2.0,
        dot,
        dot,
    )
}

pub(super) fn profile_active_target_rect(row: Rect) -> Rect {
    profile_active_indicator_rect(row)
}

pub(super) fn profile_remove_rect_for_ui(row: Rect) -> Rect {
    let h = DESKTOP_PROFILE_ROW_H;
    let w = 32.0;
    Rect::xywh(row.origin.x + row.size.x - w, row.origin.y, w, h)
}

pub(super) fn profile_remove_hover_rect_for_ui(row: Rect) -> Rect {
    let target = profile_remove_rect_for_ui(row);
    let inset = 2.0;
    Rect::xywh(
        target.origin.x + inset,
        target.origin.y + inset,
        target.size.x - inset * 2.0,
        target.size.y - inset * 2.0,
    )
}

pub(super) fn profile_chevron_rect_for_ui(row: Rect) -> Rect {
    let remove_w = 32.0;
    let size = 24.0;
    let header_h = DESKTOP_PROFILE_ROW_H;
    Rect::xywh(
        row.origin.x + row.size.x - remove_w - size,
        row.origin.y + (header_h - size) / 2.0,
        size,
        size,
    )
}

pub(super) fn profile_header_rect_for_ui(row: Rect) -> Rect {
    Rect::xywh(
        row.origin.x,
        row.origin.y,
        row.size.x,
        DESKTOP_PROFILE_ROW_H,
    )
}

fn profile_field_block_y(row: Rect, field_index: usize) -> f32 {
    row.origin.y + DESKTOP_PROFILE_FORM_TOP + field_index as f32 * DESKTOP_ROW_H
}

pub(super) fn profile_field_label_rect_for_ui(row: Rect, field_index: usize) -> Rect {
    let y = profile_field_block_y(row, field_index);
    Rect::xywh(
        row.origin.x + 12.0,
        y,
        DESKTOP_LABEL_W - 12.0,
        DESKTOP_PROFILE_FIELD_H,
    )
}

pub(super) fn profile_field_rect_for_ui(row: Rect, field_index: usize) -> Rect {
    let label = profile_field_label_rect_for_ui(row, field_index);
    Rect::xywh(
        row.origin.x + DESKTOP_LABEL_W,
        label.origin.y,
        row.size.x - DESKTOP_LABEL_W - 12.0,
        DESKTOP_PROFILE_FIELD_H,
    )
}

pub(super) fn profile_input_rect_for_ui(row: Rect, field: ImageGenField) -> Rect {
    let mut input = profile_field_rect_for_ui(row, profile_field_index(field));
    if matches!(field, ImageGenField::ApiKey) {
        input.size.x =
            (input.size.x - DESKTOP_PROFILE_TEST_GAP - DESKTOP_PROFILE_TEST_BTN_W).max(48.0);
    }
    input
}

pub(super) fn profile_test_btn_rect_for_ui(row: Rect) -> Rect {
    let input = profile_field_rect_for_ui(row, 2);
    Rect::xywh(
        input.origin.x + input.size.x - DESKTOP_PROFILE_TEST_BTN_W,
        input.origin.y,
        DESKTOP_PROFILE_TEST_BTN_W,
        DESKTOP_PROFILE_FIELD_H,
    )
}

pub(super) fn profile_provider_rect_for_ui(row: Rect) -> Rect {
    profile_field_rect_for_ui(row, 1)
}

pub(super) fn profile_provider_option_rect_for_ui(row: Rect, option_index: usize) -> Rect {
    let provider = profile_provider_rect_for_ui(row);
    let option_h = DESKTOP_PROVIDER_OPTION_H;
    Rect::xywh(
        provider.origin.x,
        provider.origin.y + provider.size.y + option_index as f32 * option_h,
        provider.size.x,
        option_h,
    )
}
