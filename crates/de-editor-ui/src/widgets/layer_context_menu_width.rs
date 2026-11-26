//! The layer / page context menu's measured width (#435).
//!
//! The menu keeps [`LAYER_CONTEXT_MENU_WIDTH`] unless a row's label, at the
//! chrome font's real metrics, needs more. Only paint holds a backend to
//! measure with, while the press and cursor-move hit-tests build the menu
//! without one, so the width paint measured is kept here and every
//! hit-test reads the same number.
//!
//! One thread-local slot, keyed on what decides the labels: the locale and
//! the set of rows (each action has exactly one label key, so a bitmask of
//! the shown actions names the label set). A frame whose key matches the
//! slot measures nothing and allocates nothing; the slot is measured again
//! only when the locale or the row set changes. Only one menu is ever
//! open, so a single slot suffices.
//!
//! Before the first paint of a key the slot has no width and hit-test uses
//! [`LAYER_CONTEXT_MENU_WIDTH`]. Nothing can be aimed at a menu the user
//! has not seen yet, and the host paints it (filling the slot) before the
//! next pointer event can see it.

use std::cell::Cell;

use de_editor_core::Locale;

use super::layer_context_menu::{
    Row, LABEL_RIGHT_PAD, LABEL_X, LAYER_CONTEXT_MENU_WIDTH, ROW_FONT,
};
use super::text_metrics;
use crate::RenderBackend;

/// What decides the menu's labels: the locale and the shown actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WidthKey {
    locale: Locale,
    actions: u64,
}

impl WidthKey {
    pub(super) fn new(locale: Locale, rows: &[Row]) -> Self {
        let actions = rows
            .iter()
            .fold(0u64, |mask, row| mask | 1u64 << row.action as u32);
        Self { locale, actions }
    }
}

thread_local! {
    static SLOT: Cell<Option<(WidthKey, f32)>> = const { Cell::new(None) };
}

/// The width paint measured for `key`, if it has been painted.
pub(super) fn cached(key: WidthKey) -> Option<f32> {
    SLOT.with(|slot| match slot.get() {
        Some((k, width)) if k == key => Some(width),
        _ => None,
    })
}

/// The menu width for `rows` in `locale`: the cached width when `key`
/// matches, else measured through the chrome family and cached.
pub(super) fn measure(
    backend: &mut dyn RenderBackend,
    key: WidthKey,
    locale: Locale,
    rows: &[Row],
) -> f32 {
    if let Some(width) = cached(key) {
        return width;
    }
    let width = rows
        .iter()
        .map(|row| {
            LABEL_X
                + text_metrics::measure_chrome(
                    backend,
                    de_i18n::translate(locale, row.label_key),
                    ROW_FONT,
                )
                + LABEL_RIGHT_PAD
        })
        .fold(LAYER_CONTEXT_MENU_WIDTH, f32::max)
        .ceil();
    SLOT.with(|slot| slot.set(Some((key, width))));
    width
}

/// Empty the slot, so a test starts from an unpainted menu.
#[cfg(test)]
pub(super) fn reset() {
    SLOT.with(|slot| slot.set(None));
}
