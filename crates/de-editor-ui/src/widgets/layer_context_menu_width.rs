//! The layer / page context menu's measured width (#435).
//!
//! The menu keeps [`LAYER_CONTEXT_MENU_WIDTH`] unless a row's label, at the
//! chrome font's real metrics, needs more
//! ([`LayerContextMenu::required_width`]). Measuring needs a backend, which
//! the shared press flow and the widget's own `hit` / `rect` do not get, so
//! the width is measured once and kept here, and every hit-test reads the
//! same number paint uses.
//!
//! One thread-local slot, keyed on what decides the labels: the locale and
//! the set of rows (each action has exactly one label key, so a bitmask of
//! the shown actions names the label set). A build whose key matches the
//! slot measures nothing and allocates nothing; the slot is measured again
//! only when the locale or the row set changes. Only one menu is ever
//! open, so a single slot suffices.
//!
//! Who fills it: paint ([`LayerContextMenu::measured_rect`]), and the
//! native host, which holds a measure-only backend outside the paint pass
//! and measures whenever it builds the menu for the right press that opens
//! it, for a hover or for a press. So a cursor move winit delivers after
//! the opening right press but before the first redraw already sees the
//! measured width. Only a caller with no backend at all (a unit test, a
//! host without a measuring backend) reads [`LAYER_CONTEXT_MENU_WIDTH`]
//! for a key that was never measured.
//!
//! [`LayerContextMenu::required_width`]: super::layer_context_menu::LayerContextMenu::required_width
//! [`LayerContextMenu::measured_rect`]: super::layer_context_menu::LayerContextMenu::measured_rect

use std::cell::Cell;

use de_editor_core::Locale;

use super::layer_context_menu::LayerContextAction;
#[cfg(doc)]
use super::layer_context_menu::LAYER_CONTEXT_MENU_WIDTH;

/// What decides the menu's labels: the locale and the shown actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WidthKey {
    locale: Locale,
    actions: u64,
}

impl WidthKey {
    pub(super) fn new(locale: Locale, actions: impl Iterator<Item = LayerContextAction>) -> Self {
        let actions = actions.fold(0u64, |mask, action| mask | 1u64 << action as u32);
        Self { locale, actions }
    }
}

thread_local! {
    static SLOT: Cell<Option<(WidthKey, f32)>> = const { Cell::new(None) };
}

/// The width measured for `key`, if it has been measured.
pub(super) fn cached(key: WidthKey) -> Option<f32> {
    SLOT.with(|slot| match slot.get() {
        Some((k, width)) if k == key => Some(width),
        _ => None,
    })
}

/// Keep `width` as the measured width for `key`.
pub(super) fn store(key: WidthKey, width: f32) {
    SLOT.with(|slot| slot.set(Some((key, width))));
}

/// Empty the slot, so a test starts from an unmeasured menu.
#[cfg(test)]
pub(super) fn reset() {
    SLOT.with(|slot| slot.set(None));
}
