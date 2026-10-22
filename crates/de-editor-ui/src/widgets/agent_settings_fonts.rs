//! Fonts tab of the settings modal.

use crate::theme::Theme;
use crate::widgets::editor_state_ext::translate;
use crate::widgets::missing_fonts_panel::{
    paint_missing_font_row, paint_text, row_button_rect, ROW_HEIGHT,
};
use crate::widgets::property_panel_typography::{
    font_picker_action_in_layout, font_picker_entries, font_picker_hit_in_layout,
    font_picker_layout_at_for_ui, paint_font_picker_at_for_ui, FontPickerAction, FontPickerLayout,
};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::missing_fonts::MissingFontEntry;
use jian_widgets::centered_text_baseline_y;

const TOP_PAD: f32 = 12.0;
const SECTION_TITLE_HEIGHT: f32 = 36.0;
const EMPTY_BODY_HEIGHT: f32 = 44.0;
const SECTION_GAP: f32 = 28.0;
const BOTTOM_PAD: f32 = 24.0;
const REMOVE_HEIGHT: f32 = 28.0;
const IMPORTED_ROW_HEIGHT: f32 = 44.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontsHit {
    ChooseFont(usize),
    SelectFont(usize),
    ImportFont(usize),
    ClosePicker,
    PickerInside,
    RemoveImportedFont(usize),
    None,
}

fn missing_entries(ui: &EditorUiState) -> &[MissingFontEntry] {
    ui.missing_fonts_prompt
        .as_ref()
        .map(|prompt| prompt.entries.as_slice())
        .unwrap_or_default()
}

fn missing_body_height(ui: &EditorUiState) -> f32 {
    let rows = missing_entries(ui).len();
    if rows == 0 {
        EMPTY_BODY_HEIGHT
    } else {
        rows as f32 * ROW_HEIGHT
    }
}

fn imported_section_top(content: Rect, ui: &EditorUiState) -> f32 {
    content.origin.y + TOP_PAD + SECTION_TITLE_HEIGHT + missing_body_height(ui) + SECTION_GAP
}

pub(crate) fn missing_row_rect(content: Rect, row: usize) -> Rect {
    Rect::xywh(
        content.origin.x,
        content.origin.y + TOP_PAD + SECTION_TITLE_HEIGHT + row as f32 * ROW_HEIGHT,
        content.size.x,
        ROW_HEIGHT,
    )
}

fn imported_row_rect(content: Rect, ui: &EditorUiState, row: usize) -> Rect {
    let row_height = IMPORTED_ROW_HEIGHT;
    Rect::xywh(
        content.origin.x,
        imported_section_top(content, ui) + SECTION_TITLE_HEIGHT + row as f32 * row_height,
        content.size.x,
        row_height,
    )
}

fn settings_row_button_rect(row: Rect, ui: &EditorUiState) -> Rect {
    row_button_rect(row, ui)
}

pub(crate) fn imported_remove_rect(content: Rect, ui: &EditorUiState, row: usize) -> Rect {
    let row = imported_row_rect(content, ui, row);
    let font_size = 11.0;
    let height = REMOVE_HEIGHT;
    let width =
        (super::missing_fonts_panel::fit_button_width(translate(ui, "common.delete"), font_size)
            + 0.0)
            .max(height);
    Rect::xywh(
        row.origin.x + row.size.x - width,
        row.origin.y + (row.size.y - height) / 2.0,
        width,
        height,
    )
}

pub(super) fn content_height(ui: &EditorUiState) -> f32 {
    TOP_PAD
        + SECTION_TITLE_HEIGHT
        + missing_body_height(ui)
        + SECTION_GAP
        + SECTION_TITLE_HEIGHT
        + ui.imported_font_families.len() as f32 * IMPORTED_ROW_HEIGHT
        + BOTTOM_PAD
}

fn picker_row(ui: &EditorUiState) -> Option<usize> {
    match ui.font_picker_purpose {
        Some(de_editor_core::FontPickerPurpose::MissingFont {
            row,
            surface: de_editor_core::MissingFontSurface::Settings,
        }) if ui.font_picker.open => Some(row),
        _ => None,
    }
}

pub(crate) fn picker_layout(
    panel: Rect,
    content: Rect,
    ui: &EditorUiState,
    scroll_y: f32,
) -> Option<FontPickerLayout> {
    let row = picker_row(ui)?;
    if missing_entries(ui).get(row)?.resolved {
        return None;
    }
    let entries = font_picker_entries(
        &ui.imported_font_families,
        &ui.bundled_font_families,
        &ui.system_font_families,
        &ui.font_picker_search,
    );
    let mut trigger = settings_row_button_rect(missing_row_rect(content, row), ui);
    trigger.origin.y -= scroll_y;
    Some(font_picker_layout_at_for_ui(
        trigger,
        300.0,
        picker_bounds(panel, content),
        &entries,
        ui.font_import_supported,
        true,
        ui.font_picker.scroll.offset,
    ))
}

fn picker_bounds(panel: Rect, content: Rect) -> Rect {
    let left = content.origin.x.max(panel.origin.x);
    let top = content.origin.y.max(panel.origin.y);
    let right = (content.origin.x + content.size.x).min(panel.origin.x + panel.size.x);
    let bottom = (content.origin.y + content.size.y).min(panel.origin.y + panel.size.y);
    Rect::xywh(left, top, (right - left).max(1.0), (bottom - top).max(1.0))
}

pub fn hit_test(
    panel: Rect,
    content: Rect,
    ui: &EditorUiState,
    point: Point2D,
    scroll_y: f32,
) -> FontsHit {
    if let Some(row) = picker_row(ui) {
        if let Some(layout) = picker_layout(panel, content, ui, scroll_y) {
            if let Some(action) = font_picker_action_in_layout(&layout, point) {
                return match action {
                    FontPickerAction::Select(index) => FontsHit::SelectFont(index),
                    FontPickerAction::Import => FontsHit::ImportFont(row),
                    FontPickerAction::Remove(index) => imported_index_for_picker_entry(ui, index)
                        .map_or(FontsHit::PickerInside, FontsHit::RemoveImportedFont),
                };
            }
            return match font_picker_hit_in_layout(&layout, point) {
                jian_widgets::components::select::SelectHit::Outside => FontsHit::ClosePicker,
                _ => FontsHit::PickerInside,
            };
        }
        return FontsHit::ClosePicker;
    }
    let scrolled = Point2D::new(point.x, point.y + scroll_y);
    for (row, entry) in missing_entries(ui).iter().enumerate() {
        if !entry.resolved
            && settings_row_button_rect(missing_row_rect(content, row), ui).contains(scrolled)
        {
            return FontsHit::ChooseFont(row);
        }
    }
    for row in 0..ui.imported_font_families.len() {
        if imported_remove_rect(content, ui, row).contains(scrolled) {
            return FontsHit::RemoveImportedFont(row);
        }
    }
    FontsHit::None
}

pub(super) fn paint_picker(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    panel: Rect,
    content: Rect,
    ui: &EditorUiState,
    scroll_y: f32,
    now_ms: u64,
) {
    let Some(row) = picker_row(ui) else {
        return;
    };
    let Some(entry) = missing_entries(ui).get(row) else {
        return;
    };
    let entries = font_picker_entries(
        &ui.imported_font_families,
        &ui.bundled_font_families,
        &ui.system_font_families,
        &ui.font_picker_search,
    );
    let mut trigger = settings_row_button_rect(missing_row_rect(content, row), ui);
    trigger.origin.y -= scroll_y;
    paint_font_picker_at_for_ui(
        cx,
        theme,
        trigger,
        300.0,
        picker_bounds(panel, content),
        ui.effective_locale(),
        &entries,
        ui.font_import_supported,
        true,
        &ui.font_picker_search,
        &ui.font_picker,
        ui.font_picker_import_hover,
        &entry.family,
        now_ms,
    );
}

fn imported_index_for_picker_entry(ui: &EditorUiState, entry_index: usize) -> Option<usize> {
    let entries = font_picker_entries(
        &ui.imported_font_families,
        &ui.bundled_font_families,
        &ui.system_font_families,
        &ui.font_picker_search,
    );
    let entry = entries.get(entry_index)?;
    entry.imported.then(|| {
        ui.imported_font_families
            .iter()
            .position(|family| family == &entry.family)
    })?
}

pub(super) fn paint_fonts_tab(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    content: Rect,
) {
    paint_section_title(
        cx,
        theme,
        translate(ui, "missingFonts.title"),
        content.origin.x,
        content.origin.y + TOP_PAD,
    );

    let entries = missing_entries(ui);
    if entries.is_empty() {
        paint_text(
            cx,
            translate(ui, "missingFonts.noneMissing"),
            Point2D::new(
                content.origin.x,
                content.origin.y + TOP_PAD + SECTION_TITLE_HEIGHT + 20.0,
            ),
            12.0,
            400,
            theme.muted_foreground,
        );
    } else {
        for (row, entry) in entries.iter().enumerate() {
            paint_settings_missing_row(
                cx,
                theme,
                ui,
                entry,
                row,
                missing_row_rect(content, row),
                row > 0,
            );
        }
    }

    let imported_top = imported_section_top(content, ui);
    paint_section_title(
        cx,
        theme,
        translate(ui, "missingFonts.importedSection"),
        content.origin.x,
        imported_top,
    );
    for (row, family) in ui.imported_font_families.iter().enumerate() {
        let row_rect = imported_row_rect(content, ui, row);
        let remove = imported_remove_rect(content, ui, row);
        if row > 0 {
            cx.backend.fill_rect(
                Rect::xywh(row_rect.origin.x, row_rect.origin.y, row_rect.size.x, 1.0),
                theme.border,
            );
        }
        let family_font = 13.0;
        let family_w = (remove.origin.x - row_rect.origin.x - 16.0).max(1.0);
        let family = crate::util::ellipsize_to_width(family, family_w, |text| {
            cx.backend
                .measure_text_family_styled(text, family_font, "system-ui", 500, false)
        });
        cx.backend.save();
        cx.backend.clip_rect(Rect::xywh(
            row_rect.origin.x,
            row_rect.origin.y + 1.0,
            family_w,
            (row_rect.size.y - 2.0).max(1.0),
        ));
        paint_text(
            cx,
            &family,
            Point2D::new(
                row_rect.origin.x,
                centered_text_baseline_y(row_rect, family_font),
            ),
            family_font,
            500,
            theme.foreground,
        );
        cx.backend.restore();
        let remove_hovered = ui.missing_fonts_hover
            == Some(de_editor_core::missing_fonts::MissingFontsHover::RemoveImported(row));
        let remove_bg = if remove_hovered {
            theme.border
        } else {
            theme.muted
        };
        cx.backend.fill_round_rect(remove, 6.0, remove_bg);
        paint_text(
            cx,
            translate(ui, "common.delete"),
            Point2D::new(
                remove.origin.x + 12.0,
                centered_text_baseline_y(remove, 11.0),
            ),
            11.0,
            500,
            theme.destructive,
        );
    }
}

fn paint_settings_missing_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    entry: &MissingFontEntry,
    row_index: usize,
    row: Rect,
    divider: bool,
) {
    paint_missing_font_row(cx, theme, ui, entry, row_index, row, divider);
}

fn paint_section_title(cx: &mut PaintCx<'_>, theme: &Theme, title: &str, x: f32, y: f32) {
    let font_size = 15.0;
    paint_text(
        cx,
        title,
        Point2D::new(x, y + 20.0),
        font_size,
        500,
        theme.foreground,
    );
}

#[cfg(test)]
#[path = "agent_settings_fonts_tests.rs"]
mod tab_tests;
