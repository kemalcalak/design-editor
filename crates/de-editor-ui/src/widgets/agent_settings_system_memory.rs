//! The System tab's Memory row (#456): the undo history, the editor's
//! caches and the process memory in use, from the reading the desktop
//! publishes on a timer (`de_editor_core::memory_reading`).
//!
//! Paint neither measures nor formats on every frame: the formatted line
//! is kept per reading generation and locale, and rebuilt only when either
//! moves. The history figure comes first, so a narrow panel in a long
//! locale cuts the least telling figure.

use std::cell::RefCell;

use crate::theme::Theme;
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_rows::paint_row_label;
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::memory_reading::{self, MemoryReading};
use de_editor_core::Locale;

thread_local! {
    /// The last formatted line: (reading generation, locale, line).
    static LINE: RefCell<Option<(u64, Locale, String)>> = const { RefCell::new(None) };
}

pub(super) fn paint_memory_row(cx: &mut PaintCx<'_>, theme: &Theme, ui: &EditorUiState, row: Rect) {
    let generation = memory_reading::generation();
    let locale = ui.effective_locale();
    LINE.with(|cached| {
        let mut cached = cached.borrow_mut();
        let stale = !cached
            .as_ref()
            .is_some_and(|(at, l, _)| *at == generation && *l == locale);
        if stale {
            let line = memory_line(ui, &memory_reading::latest());
            *cached = Some((generation, locale, line));
        }
        let line = cached.as_ref().map(|(_, _, line)| line.as_str());
        paint_row_label(
            cx,
            theme,
            row,
            t_settings(ui, "settings.system.memory"),
            line,
            16.0,
        );
    });
}

/// The row's description: the localized line with its figures, or
/// "Measuring…" before the host published anything.
pub(super) fn memory_line(ui: &EditorUiState, reading: &MemoryReading) -> String {
    let locale = ui.effective_locale();
    match memory_reading::row_figures(reading) {
        Some((used, caches, history)) => t_settings(ui, "settings.system.memoryUsage")
            .replace(
                "{history}",
                &localize(locale, history.as_deref().unwrap_or("…")),
            )
            .replace("{caches}", &localize(locale, &caches))
            .replace("{used}", &localize(locale, &used)),
        None => t_settings(ui, "settings.system.memoryMeasuring").to_string(),
    }
}

/// `1.2 GB` with the locale's decimal separator.
fn localize(locale: Locale, figure: &str) -> String {
    let comma = matches!(
        locale,
        Locale::Fr
            | Locale::Es
            | Locale::De
            | Locale::Pt
            | Locale::Ru
            | Locale::Tr
            | Locale::Id
            | Locale::Vi
    );
    if comma {
        figure.replace('.', ",")
    } else {
        figure.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::memory_report::{CacheUsage, MemoryReport};
    use de_editor_core::process_memory::ProcessMemory;

    #[test]
    fn the_memory_row_reads_the_published_figures() {
        let mut ui = EditorUiState {
            locale: Locale::EnUs,
            ..Default::default()
        };
        assert_eq!(memory_line(&ui, &MemoryReading::default()), "Measuring…");
        let mut caches = MemoryReport::default();
        caches.push(CacheUsage::exact("decoded_images", 20 << 20, 3, None));
        let mut reading = MemoryReading {
            process: ProcessMemory {
                resident: Some(900 << 20),
                footprint: Some(1 << 30),
            },
            caches,
            history: None,
            measured: true,
        };
        assert_eq!(
            memory_line(&ui, &reading),
            "Undo history … · editor caches 20 MB · 1.0 GB in use"
        );
        reading.history = Some(CacheUsage::estimate("undo_history", 550 << 20, 20, None));
        ui.locale = Locale::Tr;
        assert_eq!(
            memory_line(&ui, &reading),
            "Geri alma geçmişi 550 MB · düzenleyici önbellekleri 20 MB · 1,0 GB kullanımda"
        );
    }
}
