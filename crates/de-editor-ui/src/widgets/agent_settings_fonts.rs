//! Fonts tab of the settings modal.

use crate::theme::Theme;
use crate::widgets::editor_state_ext::translate;
use crate::widgets::missing_fonts_panel::{
    paint_missing_font_row, paint_text, row_button_rect, row_download_hit_rect, ROW_HEIGHT,
};
use crate::widgets::property_panel_typography::{
    font_picker_action_in_layout, font_picker_entries, font_picker_hit_in_layout,
    font_picker_layout_at_for_ui, paint_font_picker_at_for_ui, FontPickerAction, FontPickerLayout,
};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::missing_fonts::{MissingFontEntry, MissingFontsHover};
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
    /// Download this missing row's family from Google Fonts.
    Download(usize),
    SelectFont(usize),
    ImportFont(usize),
    ClosePicker,
    PickerInside,
    /// Index into [`user_imported_families`].
    RemoveImportedFont(usize),
    /// Index into `downloaded_font_families`.
    RemoveDownloadedFont(usize),
    None,
}

/// Families the user imported from disk: `imported_font_families` minus the
/// ones that came from Google Fonts downloads, which get their own section.
pub(crate) fn user_imported_families(ui: &EditorUiState) -> Vec<String> {
    ui.imported_font_families
        .iter()
        .filter(|family| {
            !ui.downloaded_font_families
                .iter()
                .any(|downloaded| downloaded.eq_ignore_ascii_case(family))
        })
        .cloned()
        .collect()
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

fn downloaded_section_top(content: Rect, ui: &EditorUiState) -> f32 {
    content.origin.y + TOP_PAD + SECTION_TITLE_HEIGHT + missing_body_height(ui) + SECTION_GAP
}

/// Height of the downloaded section, title included; the section is left
/// out entirely while nothing has been downloaded.
fn downloaded_section_height(ui: &EditorUiState) -> f32 {
    match ui.downloaded_font_families.len() {
        0 => 0.0,
        rows => SECTION_TITLE_HEIGHT + rows as f32 * IMPORTED_ROW_HEIGHT + SECTION_GAP,
    }
}

fn imported_section_top(content: Rect, ui: &EditorUiState) -> f32 {
    downloaded_section_top(content, ui) + downloaded_section_height(ui)
}

pub(crate) fn missing_row_rect(content: Rect, row: usize) -> Rect {
    Rect::xywh(
        content.origin.x,
        content.origin.y + TOP_PAD + SECTION_TITLE_HEIGHT + row as f32 * ROW_HEIGHT,
        content.size.x,
        ROW_HEIGHT,
    )
}

/// One row of a family list section whose title sits at `section_top`.
fn family_row_rect(content: Rect, section_top: f32, row: usize) -> Rect {
    Rect::xywh(
        content.origin.x,
        section_top + SECTION_TITLE_HEIGHT + row as f32 * IMPORTED_ROW_HEIGHT,
        content.size.x,
        IMPORTED_ROW_HEIGHT,
    )
}

fn imported_row_rect(content: Rect, ui: &EditorUiState, row: usize) -> Rect {
    family_row_rect(content, imported_section_top(content, ui), row)
}

fn downloaded_row_rect(content: Rect, ui: &EditorUiState, row: usize) -> Rect {
    family_row_rect(content, downloaded_section_top(content, ui), row)
}

fn settings_row_button_rect(row: Rect, ui: &EditorUiState) -> Rect {
    row_button_rect(row, ui)
}

pub(crate) fn imported_remove_rect(content: Rect, ui: &EditorUiState, row: usize) -> Rect {
    remove_rect_in(imported_row_rect(content, ui, row), ui)
}

pub(crate) fn downloaded_remove_rect(content: Rect, ui: &EditorUiState, row: usize) -> Rect {
    remove_rect_in(downloaded_row_rect(content, ui, row), ui)
}

fn remove_rect_in(row: Rect, ui: &EditorUiState) -> Rect {
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
        + downloaded_section_height(ui)
        + SECTION_TITLE_HEIGHT
        + user_imported_families(ui).len() as f32 * IMPORTED_ROW_HEIGHT
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
                    FontPickerAction::Remove(index) => {
                        remove_hit_for_picker_entry(ui, index).unwrap_or(FontsHit::PickerInside)
                    }
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
        if entry.resolved {
            continue;
        }
        let row_rect = missing_row_rect(content, row);
        if settings_row_button_rect(row_rect, ui).contains(scrolled) {
            return FontsHit::ChooseFont(row);
        }
        if row_download_hit_rect(row_rect, ui, entry).is_some_and(|rect| rect.contains(scrolled)) {
            return FontsHit::Download(row);
        }
    }
    for row in 0..ui.downloaded_font_families.len() {
        if downloaded_remove_rect(content, ui, row).contains(scrolled) {
            return FontsHit::RemoveDownloadedFont(row);
        }
    }
    for row in 0..user_imported_families(ui).len() {
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

/// The section remove a picker row's remove button stands for: a
/// downloaded family maps into the downloaded section, any other imported
/// family into the imported section.
fn remove_hit_for_picker_entry(ui: &EditorUiState, entry_index: usize) -> Option<FontsHit> {
    let entries = font_picker_entries(
        &ui.imported_font_families,
        &ui.bundled_font_families,
        &ui.system_font_families,
        &ui.font_picker_search,
    );
    let entry = entries.get(entry_index).filter(|entry| entry.imported)?;
    if let Some(index) = ui
        .downloaded_font_families
        .iter()
        .position(|family| family.eq_ignore_ascii_case(&entry.family))
    {
        return Some(FontsHit::RemoveDownloadedFont(index));
    }
    user_imported_families(ui)
        .iter()
        .position(|family| family == &entry.family)
        .map(FontsHit::RemoveImportedFont)
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

    if !ui.downloaded_font_families.is_empty() {
        let top = downloaded_section_top(content, ui);
        paint_section_title(
            cx,
            theme,
            translate(ui, "missingFonts.downloadedSection"),
            content.origin.x,
            top,
        );
        paint_family_rows(
            cx,
            theme,
            ui,
            content,
            top,
            &ui.downloaded_font_families,
            MissingFontsHover::RemoveDownloaded,
        );
    }

    let imported_top = imported_section_top(content, ui);
    paint_section_title(
        cx,
        theme,
        translate(ui, "missingFonts.importedSection"),
        content.origin.x,
        imported_top,
    );
    paint_family_rows(
        cx,
        theme,
        ui,
        content,
        imported_top,
        &user_imported_families(ui),
        MissingFontsHover::RemoveImported,
    );
}

/// Rows of family name + Delete button under the section title at `top`.
fn paint_family_rows(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    content: Rect,
    top: f32,
    families: &[String],
    hover_of: fn(usize) -> MissingFontsHover,
) {
    for (row, family) in families.iter().enumerate() {
        let row_rect = family_row_rect(content, top, row);
        let remove = remove_rect_in(row_rect, ui);
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
        let remove_hovered = ui.missing_fonts_hover == Some(hover_of(row));
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
