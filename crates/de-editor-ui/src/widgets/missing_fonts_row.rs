//! One missing-font row: geometry and paint shared by the one-shot modal
//! (`missing_fonts_panel.rs`) and the Settings Fonts tab, so the status,
//! mismatch and action affordances cannot drift apart.

use crate::theme::Theme;
use crate::widgets::editor_state_ext::translate;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::missing_fonts::MissingFontEntry;
use de_editor_core::EditorUiState;
use jian_widgets::centered_text_baseline_y;

pub(crate) const ROW_HEIGHT: f32 = 72.0;
pub(crate) const BUTTON_HEIGHT: f32 = 28.0;
pub(crate) const TEXT_BUTTON_GAP: f32 = 12.0;
const FAMILY_FONT_SIZE: f32 = 13.0;
const FAMILY_FONT_WEIGHT: u16 = 600;
const FAMILY_LINE_HEIGHT: f32 = 16.0;

/// Shared choose-file geometry used by the modal and Settings Fonts tab.
/// Fit-content pill width: label width + 12px padding each side.
pub(crate) fn fit_button_width(label: &str, font_size: f32) -> f32 {
    // CJK glyphs are full-width (~1.0 x font size); a flat Latin
    // factor undershoots them and clips the label.
    let text: f32 = label
        .chars()
        .map(|c| {
            if (c as u32) > 0x2E7F {
                font_size
            } else {
                font_size * 0.55
            }
        })
        .sum();
    text + 24.0
}

pub(crate) fn row_button_rect(row: Rect, ui: &EditorUiState) -> Rect {
    let width = fit_button_width(translate(ui, "missingFonts.chooseFont"), 11.0);
    Rect {
        origin: Point2D::new(
            row.origin.x + row.size.x - width,
            row.origin.y + (row.size.y - BUTTON_HEIGHT) / 2.0,
        ),
        size: Point2D::new(width, BUTTON_HEIGHT),
    }
}

pub(crate) fn row_text_rect(row: Rect, ui: &EditorUiState) -> Rect {
    let button = row_button_rect(row, ui);
    Rect {
        origin: Point2D::new(row.origin.x, row.origin.y + 1.0),
        size: Point2D::new(
            (button.origin.x - row.origin.x - TEXT_BUTTON_GAP).max(1.0),
            (row.size.y - 2.0).max(1.0),
        ),
    }
}

/// Lay a font-family label out on at most two lines, preferring CSS stack commas.
/// Unbroken names use a character boundary; only the second line is ellipsized.
pub(crate) fn family_label_lines(
    text: &str,
    max_width: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    if measure(text) <= max_width {
        return vec![text.to_owned()];
    }

    let mut fitting_end = 0;
    for (start, ch) in text.char_indices() {
        let end = start + ch.len_utf8();
        if measure(&text[..end]) > max_width {
            break;
        }
        fitting_end = end;
    }
    if fitting_end == 0 {
        return vec!["…".to_owned()];
    }

    let fitting = &text[..fitting_end];
    let mut fitting_break = |separator: fn(char) -> bool| {
        fitting
            .char_indices()
            .filter(|(_, ch)| separator(*ch))
            .map(|(start, ch)| start + ch.len_utf8())
            .next_back()
            .filter(|end| measure(text[..*end].trim_end()) >= max_width * 0.45)
    };
    let preferred_end = fitting_break(|ch| ch == ',')
        .or_else(|| fitting_break(char::is_whitespace))
        .unwrap_or(fitting_end);
    let first = text[..preferred_end].trim_end().to_owned();
    let remaining = text[preferred_end..].trim_start();
    if remaining.is_empty() {
        return vec![first];
    }
    let second = crate::util::ellipsize_to_width(remaining, max_width, &mut measure);
    vec![first, second]
}

pub(crate) fn paint_text(
    cx: &mut PaintCx<'_>,
    text: &str,
    origin: Point2D,
    font_size: f32,
    weight: u16,
    color: crate::Color,
) {
    let layout = TextLayout::single_run(
        text,
        "system-ui",
        font_size,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    )
    .with_font_weight(weight);
    cx.backend.draw_text(&layout, origin);
}

/// Paint one missing-font row. Both settings and the one-shot modal call this
/// so status, mismatch, and choose-file affordances cannot drift apart.
pub(crate) fn paint_missing_font_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    entry: &MissingFontEntry,
    row_index: usize,
    row: Rect,
    divider: bool,
) {
    let choose_hovered = ui.missing_fonts_hover
        == Some(de_editor_core::missing_fonts::MissingFontsHover::ChooseFile(row_index));
    if divider {
        cx.backend.fill_rect(
            Rect {
                origin: row.origin,
                size: Point2D::new(row.size.x, 1.0),
            },
            theme.border,
        );
    }
    let text_rect = row_text_rect(row, ui);
    let family_lines = family_label_lines(&entry.family, text_rect.size.x, |text| {
        cx.backend
            .measure_text_weighted(text, FAMILY_FONT_SIZE, FAMILY_FONT_WEIGHT)
    });
    let usage = translate(ui, "missingFonts.usage").replace("{n}", &entry.run_count.to_string());
    let usage = crate::util::ellipsize_to_width(&usage, text_rect.size.x, |text| {
        cx.backend.measure_text_weighted(text, 11.0, 400)
    });
    let mismatch_note = entry.mismatch_note.as_deref().map(|note| {
        crate::util::ellipsize_to_width(note, text_rect.size.x, |text| {
            cx.backend.measure_text_weighted(text, 11.0, 400)
        })
    });

    // Guard backend shaping differences so glyphs cannot overlap the action.
    cx.backend.save();
    cx.backend.clip_rect(text_rect);
    let family_first_y = if family_lines.len() > 1 { 17.0 } else { 22.0 };
    for (line, text) in family_lines.iter().enumerate() {
        paint_text(
            cx,
            text,
            Point2D::new(
                row.origin.x,
                row.origin.y + family_first_y + line as f32 * FAMILY_LINE_HEIGHT,
            ),
            FAMILY_FONT_SIZE,
            FAMILY_FONT_WEIGHT,
            theme.foreground,
        );
    }
    paint_text(
        cx,
        &usage,
        Point2D::new(
            row.origin.x,
            row.origin.y + if family_lines.len() > 1 { 51.0 } else { 44.0 },
        ),
        11.0,
        400,
        theme.muted_foreground,
    );
    if let Some(note) = &mismatch_note {
        paint_text(
            cx,
            note,
            Point2D::new(row.origin.x, row.origin.y + 67.0),
            11.0,
            400,
            theme.destructive,
        );
    }
    cx.backend.restore();

    let action = row_button_rect(row, ui);
    if entry.resolved {
        let chip = Rect {
            origin: Point2D::new(action.origin.x + action.size.x - 76.0, action.origin.y),
            size: Point2D::new(76.0, action.size.y),
        };
        cx.backend
            .fill_round_rect(chip, 6.0, theme.row_selected_primary);
        paint_text(
            cx,
            translate(ui, "missingFonts.resolved"),
            Point2D::new(chip.origin.x + 12.0, centered_text_baseline_y(chip, 11.0)),
            11.0,
            500,
            theme.primary,
        );
    } else {
        // Use `border` because `accent` and `muted` are identical in both palettes.
        let bg = if choose_hovered {
            theme.border
        } else {
            theme.muted
        };
        cx.backend.fill_round_rect(action, 6.0, bg);
        paint_text(
            cx,
            translate(ui, "missingFonts.chooseFont"),
            Point2D::new(
                action.origin.x + 12.0,
                centered_text_baseline_y(action, 11.0),
            ),
            11.0,
            500,
            theme.foreground,
        );
    }
}
