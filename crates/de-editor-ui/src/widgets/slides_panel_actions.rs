//! The slides rail's bottom action bar — the Transition row (#158), then
//! `▷ Present` and `Export PDF ⌄` — and the dropdown the second one
//! opens.
//!
//! Split out of `slides_panel.rs` at the 800-line ceiling. The bar is
//! the panel's only permanently-visible control surface, so it is
//! **pinned to the rail's bottom edge and never scrolls**: the list band
//! above it is laid out at `panel height − tab row − bar`, which is what
//! keeps the last thumbnail clear of it rather than sliding underneath.
//!
//! **The dropdown opens UPWARD.** It hangs off a control that is already
//! sitting on the rail's bottom edge, so there is no room below it; it
//! grows towards the list and paints OVER the thumbnails, as an overlay
//! drawn after everything else. That is also why its rects live on
//! [`SlidesActionLayout`] rather than being derived at paint time — the
//! hit-test has no backend and must land on exactly the rows that were
//! painted.
//!
//! Nothing here measures text to decide a rect. The bar splits its width
//! evenly between the two buttons and the menu takes the rail's full
//! inner width, so paint and hit-test agree at every rail width and in
//! every locale; labels that overrun their button are truncated at paint
//! time instead of moving the button.

use de_editor_core::SlidesPanelTarget;
use jian_ops_schema::motion::SlideTransitionKind;
use jian_widgets::centered_text_baseline_y;

use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::menu_paint;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout, Theme};

/// Height of the Transition row at the top of the bar (#158).
const TRANSITION_ROW_H: f32 = 50.0;
/// Height of the Transition label line above its cells.
const TRANSITION_LABEL_H: f32 = 18.0;
/// Height of the bar's button row.
const BUTTON_ROW_H: f32 = 48.0;
/// Height of the bar pinned to the rail's bottom edge.
pub const ACTION_BAR_HEIGHT: f32 = TRANSITION_ROW_H + BUTTON_ROW_H;
/// The Transition row's cells, left to right.
pub const TRANSITION_KINDS: [SlideTransitionKind; 4] = [
    SlideTransitionKind::None,
    SlideTransitionKind::Fade,
    SlideTransitionKind::Slide,
    SlideTransitionKind::Push,
];
/// Height of the two buttons inside it.
const BUTTON_H: f32 = 30.0;
/// Gap between the two buttons.
const BUTTON_GAP: f32 = 8.0;
const BUTTON_RADIUS: f32 = 6.0;
const LABEL_FONT: f32 = 12.0;
const PLAY_ICON: f32 = 13.0;
const LABEL_GAP: f32 = 6.0;
/// The dropdown chevron on the export button, and the gap before it.
const CHEVRON_SIZE: f32 = 12.0;
const CHEVRON_GAP: f32 = 4.0;
/// Padding inside a button, either side of its content.
const BUTTON_PAD_X: f32 = 8.0;
/// Side of the duration stepper's `−` and `+` targets on the Transition
/// label line (#190).
const STEPPER_BUTTON: f32 = 18.0;
/// Width of the stepper's value column between them (`5000 ms`).
const STEPPER_VALUE_W: f32 = 52.0;
/// Alpha of a stepper end a press would not move.
const STEPPER_DISABLED_ALPHA: f32 = 0.4;
/// Gap between the Transition label and the stepper.
const STEPPER_LABEL_GAP: f32 = 6.0;

/// Height of one dropdown row — the same 30 px the export quick menu
/// and the file menu use, so every menu in the app scans alike.
const MENU_ROW_H: f32 = 30.0;
const MENU_PAD_Y: f32 = 6.0;
const MENU_PAD_X: f32 = 12.0;
const MENU_RADIUS: f32 = 10.0;
/// Gap between the menu's bottom edge and the bar it hangs off.
const MENU_GAP: f32 = 6.0;
/// The dropdown's two rows, top to bottom.
const MENU_ROWS: [SlidesPanelTarget; 2] = [
    SlidesPanelTarget::ExportAllSlides,
    SlidesPanelTarget::ExportSelectedSlides,
];

/// Whether this build can export a SUBSET of a deck to PDF.
///
/// True on every host. `de_host_services::export_pdf::export_deck_pdf_boards`
/// writes the slide-per-page deck restricted to a board list, and
/// `FileAction::ExportDeckPdfSelection` carries the request to it — the
/// desktop resolves the boards at save time, the browser posts them to the
/// daemon (its selection cannot survive the document round-trip, so the ids
/// travel as data).
///
/// Kept as a function rather than inlined `true` because the row's other
/// gate — "is anything selected" — is a different question, and the two
/// should stay separately answerable: a host that one day cannot export
/// (no daemon reachable, say) turns THIS off without touching the
/// selection rule, and the widget, hit-test and tests already run both
/// ways.
pub fn selected_slides_export_supported() -> bool {
    true
}

/// Where the bar's two buttons and the open dropdown's rows sit.
///
/// One struct built once per event and per paint, so a button's painted
/// rect and its click target are the same rect by construction.
#[derive(Debug, Clone, PartialEq)]
pub struct SlidesActionLayout {
    /// The bar itself, pinned to the rail's bottom edge.
    pub bar: Rect,
    /// The `▷ Present` button.
    pub present: Rect,
    /// The `Export PDF ⌄` button.
    pub export: Rect,
    /// The Transition row's cells, in [`TRANSITION_KINDS`] order.
    pub transition_cells: Vec<Rect>,
    /// The kind every targeted slide shares (`None` = mixed): the lit cell.
    pub transition: Option<SlideTransitionKind>,
    /// The duration stepper's `−`, value and `+` rects, right-aligned on
    /// the Transition label line; `None` while it is hidden (mixed kinds
    /// or none).
    pub duration_stepper: Option<[Rect; 3]>,
    /// The duration it shows, in ms.
    pub transition_duration_ms: Option<u64>,
    /// The dropdown's surface, when it is open.
    pub menu: Option<Rect>,
    /// How many of the listed slides the selection covers. Drives the
    /// `(N)` in the second row's label, live at every count including 0.
    pub selected_slides: usize,
    /// Whether the "export selected" row can be hovered and activated —
    /// see [`selected_slides_export_supported`] and
    /// [`SlidesActionState`].
    pub selected_enabled: bool,
}

/// What the panel's owner knows about the bar that its geometry cannot
/// work out for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SlidesActionState {
    pub export_menu_open: bool,
    /// Slides covered by the current selection.
    pub selected_slides: usize,
    /// Whether a subset export exists to route the second row to. Passed
    /// in rather than read from [`selected_slides_export_supported`]
    /// directly so tests can exercise the enabled path that no host
    /// enables yet.
    pub selected_export_supported: bool,
    /// The slide transition the Transition row shows as set — see
    /// `EditorState::slides_transition_kind`.
    pub transition: Option<SlideTransitionKind>,
    /// The duration the stepper shows, `None` to hide it — see
    /// `EditorState::slides_transition_duration`.
    pub transition_duration_ms: Option<u64>,
}

impl SlidesActionLayout {
    /// Lay the bar across the bottom of `panel`, and the dropdown above
    /// it when it is open.
    ///
    /// `list_top` is the top of the scrolling band — the dropdown is
    /// clamped to it so a rail too short to hold the menu pushes it down
    /// over the thumbnails rather than up over the tab row, which is the
    /// one piece of chrome that must stay reachable (it is how the user
    /// gets back to the Layers tree).
    pub fn new(panel: Rect, list_top: f32, actions: SlidesActionState) -> Self {
        let bar = Rect {
            origin: Point2D::new(
                panel.origin.x,
                panel.origin.y + panel.size.y - ACTION_BAR_HEIGHT,
            ),
            size: Point2D::new(panel.size.x, ACTION_BAR_HEIGHT),
        };
        let inner_w = (panel.size.x - super::slides_panel::ROW_PAD_X * 2.0).max(0.0);
        // An even split, not a measured one: the widths must be identical
        // at hit-test time, where there is no shaper, and a locale whose
        // "Present" is three words must not move the export button out
        // from under the cursor.
        let half = ((inner_w - BUTTON_GAP) / 2.0).max(0.0);
        let x = panel.origin.x + super::slides_panel::ROW_PAD_X;
        let y = bar.origin.y + TRANSITION_ROW_H + (BUTTON_ROW_H - BUTTON_H) / 2.0;
        let transition_row = Rect {
            // The label sits on its own line above, so the four cells get
            // the rail's whole inner width in every locale.
            origin: Point2D::new(x, bar.origin.y + 6.0 + TRANSITION_LABEL_H),
            size: Point2D::new(inner_w, TRANSITION_ROW_H - 6.0 - TRANSITION_LABEL_H),
        };
        let transition_cells = crate::widgets::property_panel_segments::segment_rects(
            transition_row,
            TRANSITION_KINDS.len(),
        );
        let duration_stepper = actions.transition_duration_ms.map(|_| {
            // Ends where the label line ends, 2 px above the cells.
            let y = transition_row.origin.y - TRANSITION_LABEL_H - 2.0;
            let right = x + inner_w;
            let plus = Rect {
                origin: Point2D::new(right - STEPPER_BUTTON, y),
                size: Point2D::new(STEPPER_BUTTON, STEPPER_BUTTON),
            };
            let value = Rect {
                origin: Point2D::new(plus.origin.x - STEPPER_VALUE_W, y),
                size: Point2D::new(STEPPER_VALUE_W, STEPPER_BUTTON),
            };
            let minus = Rect {
                origin: Point2D::new(value.origin.x - STEPPER_BUTTON, y),
                size: Point2D::new(STEPPER_BUTTON, STEPPER_BUTTON),
            };
            [minus, value, plus]
        });
        let present = Rect {
            origin: Point2D::new(x, y),
            size: Point2D::new(half, BUTTON_H),
        };
        let export = Rect {
            origin: Point2D::new(x + half + BUTTON_GAP, y),
            size: Point2D::new(half, BUTTON_H),
        };
        let selected_enabled = actions.selected_slides > 0 && actions.selected_export_supported;
        let menu = actions.export_menu_open.then(|| {
            let height = MENU_PAD_Y * 2.0 + MENU_ROW_H * MENU_ROWS.len() as f32;
            // Upward: the bar is on the rail's bottom edge, so the menu
            // grows towards the list and covers it.
            let top = (bar.origin.y - MENU_GAP - height).max(list_top);
            Rect {
                // The rail's full inner width rather than the export
                // button's. A menu anchored to a half-width button would
                // be ~100 px across on a default rail and truncate both
                // of its labels; the rows are the content here, and the
                // rail is narrow enough that its inner width IS the
                // natural menu width.
                origin: Point2D::new(x, top),
                size: Point2D::new(inner_w, height),
            }
        });
        Self {
            bar,
            present,
            export,
            transition_cells,
            transition: actions.transition,
            duration_stepper,
            transition_duration_ms: actions.transition_duration_ms,
            menu,
            selected_slides: actions.selected_slides,
            selected_enabled,
        }
    }

    /// Rect of dropdown row `index`, top to bottom. `None` when the menu
    /// is closed or the index is past its rows.
    pub fn menu_row_rect(&self, index: usize) -> Option<Rect> {
        let menu = self.menu?;
        (index < MENU_ROWS.len()).then(|| Rect {
            origin: Point2D::new(
                menu.origin.x,
                menu.origin.y + MENU_PAD_Y + index as f32 * MENU_ROW_H,
            ),
            size: Point2D::new(menu.size.x, MENU_ROW_H),
        })
    }

    /// Whether `point` is on the open dropdown's own surface — rows AND
    /// the chrome around them.
    ///
    /// The hit-test uses this to STOP: the menu covers the thumbnails,
    /// so a press on its padding must be swallowed rather than fall
    /// through to whichever slide row happens to be underneath.
    pub fn over_menu(&self, point: Point2D) -> bool {
        self.menu.is_some_and(|menu| contains(menu, point))
    }

    /// Which dropdown row `point` lands on. `None` for menu chrome and
    /// for a row that is disabled — a disabled row is not a target, so
    /// it neither highlights on hover nor activates on release.
    pub fn menu_row_at(&self, point: Point2D) -> Option<SlidesPanelTarget> {
        MENU_ROWS.iter().enumerate().find_map(|(index, row)| {
            let rect = self.menu_row_rect(index)?;
            if !contains(rect, point) || !self.row_enabled(*row) {
                return None;
            }
            Some(*row)
        })
    }

    /// Which of the bar's own controls `point` lands on.
    pub fn button_at(&self, point: Point2D) -> Option<SlidesPanelTarget> {
        if let Some(kind) = TRANSITION_KINDS
            .iter()
            .zip(&self.transition_cells)
            .find_map(|(kind, cell)| contains(*cell, point).then_some(*kind))
        {
            return Some(SlidesPanelTarget::Transition(kind));
        }
        if let Some([minus, _, plus]) = self.duration_stepper {
            if contains(minus, point) {
                return Some(SlidesPanelTarget::TransitionDuration { longer: false });
            }
            if contains(plus, point) {
                return Some(SlidesPanelTarget::TransitionDuration { longer: true });
            }
        }
        if contains(self.present, point) {
            return Some(SlidesPanelTarget::Present);
        }
        contains(self.export, point).then_some(SlidesPanelTarget::ExportMenu)
    }

    fn row_enabled(&self, row: SlidesPanelTarget) -> bool {
        match row {
            SlidesPanelTarget::ExportSelectedSlides => self.selected_enabled,
            _ => true,
        }
    }
}

/// The labels the bar and its dropdown paint, resolved by the caller so
/// this module stays free of i18n — same discipline as the tab row.
#[derive(Debug, Clone, Copy)]
pub struct SlidesActionLabels<'a> {
    pub present: &'a str,
    pub export: &'a str,
    pub export_all: &'a str,
    /// Already has its `{{count}}` substituted.
    pub export_selected: &'a str,
    /// The Transition row's label and its cells, in [`TRANSITION_KINDS`]
    /// order.
    pub transition: &'a str,
    pub transition_kinds: [&'a str; 4],
}

/// Paint the bar. The dropdown is NOT painted here — it is an overlay
/// over the thumbnails, so it goes last, in [`paint_menu`].
pub fn paint_bar(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    layout: &SlidesActionLayout,
    labels: SlidesActionLabels<'_>,
    hover: Option<SlidesPanelTarget>,
) {
    cx.backend.fill_rect(layout.bar, theme.card);
    cx.backend.fill_rect(
        Rect {
            origin: layout.bar.origin,
            size: Point2D::new(layout.bar.size.x, 1.0),
        },
        theme.border,
    );
    paint_transition_row(cx, theme, layout, labels);
    paint_present_button(cx, theme, layout, labels.present, hover);
    paint_export_button(cx, theme, layout, labels.export, hover);
}

/// `Transition  None | Fade | Slide | Push` — what Present plays when it
/// arrives on the targeted slides.
fn paint_transition_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    layout: &SlidesActionLayout,
    labels: SlidesActionLabels<'_>,
) {
    let (Some(first), Some(last)) = (
        layout.transition_cells.first(),
        layout.transition_cells.last(),
    ) else {
        return;
    };
    let row = Rect {
        origin: first.origin,
        size: Point2D::new(last.origin.x + last.size.x - first.origin.x, first.size.y),
    };
    // The label stops short of the duration stepper when it shows.
    let label_right = layout
        .duration_stepper
        .map_or(row.origin.x + row.size.x, |[minus, ..]| {
            minus.origin.x - STEPPER_LABEL_GAP
        });
    let label_rect = Rect {
        origin: Point2D::new(row.origin.x, row.origin.y - TRANSITION_LABEL_H),
        size: Point2D::new(
            (label_right - row.origin.x).max(0.0),
            TRANSITION_LABEL_H - 2.0,
        ),
    };
    let label = crate::widgets::file_menu::truncate_to_width(
        cx,
        labels.transition,
        LABEL_FONT,
        label_rect.size.x.max(0.0),
    );
    cx.backend.draw_text(
        &TextLayout::single_run(
            &label,
            "system-ui",
            LABEL_FONT,
            theme.muted_foreground.to_jian(),
            Point2D::ZERO,
        ),
        Point2D::new(
            label_rect.origin.x,
            centered_text_baseline_y(label_rect, LABEL_FONT),
        ),
    );
    paint_duration_stepper(cx, theme, layout);
    let active = layout
        .transition
        .and_then(|kind| TRANSITION_KINDS.iter().position(|k| *k == kind));
    crate::widgets::property_panel_segments::paint_segment_row(
        cx,
        theme,
        row,
        &labels.transition_kinds,
        active,
        11.0,
    );
}

/// `−  400 ms  +` on the Transition label line (#190).
fn paint_duration_stepper(cx: &mut PaintCx<'_>, theme: &Theme, layout: &SlidesActionLayout) {
    let (Some([minus, value, plus]), Some(ms)) =
        (layout.duration_stepper, layout.transition_duration_ms)
    else {
        return;
    };
    const GLYPH: f32 = 12.0;
    // Same look as the mesh Rows / Columns stepper: one muted well, and a
    // dimmed end that a press would not move.
    let well = Rect {
        origin: minus.origin,
        size: Point2D::new(plus.origin.x + plus.size.x - minus.origin.x, minus.size.y),
    };
    cx.backend.fill_round_rect(well, BUTTON_RADIUS, theme.muted);
    for (icon, rect, enabled) in [
        (
            Icon::Minus,
            minus,
            ms > de_editor_core::slide_transition::SLIDE_TRANSITION_MIN_MS,
        ),
        (
            Icon::Plus,
            plus,
            ms < de_editor_core::slide_transition::SLIDE_TRANSITION_MAX_MS,
        ),
    ] {
        let color = if enabled {
            theme.foreground
        } else {
            crate::util::alpha(theme.muted_foreground, STEPPER_DISABLED_ALPHA)
        };
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                rect.origin.x + (rect.size.x - GLYPH) / 2.0,
                rect.origin.y + (rect.size.y - GLYPH) / 2.0,
            ),
            GLYPH,
            color,
            1.4,
        );
    }
    let text = format!("{ms} ms");
    let text_w = text_metrics::measure_chrome(cx.backend, &text, LABEL_FONT);
    cx.backend.draw_text(
        &TextLayout::single_run(
            &text,
            "system-ui",
            LABEL_FONT,
            theme.foreground.to_jian(),
            Point2D::ZERO,
        ),
        Point2D::new(
            value.origin.x + (value.size.x - text_w) / 2.0,
            centered_text_baseline_y(value, LABEL_FONT),
        ),
    );
}

/// Paint the open dropdown, over everything else in the rail.
///
/// Called after the thumbnails and after `paint_overlay`, because the
/// whole point of an upward menu on a bottom-pinned control is that it
/// covers the list it grew into.
pub fn paint_menu(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    layout: &SlidesActionLayout,
    labels: SlidesActionLabels<'_>,
    hover: Option<SlidesPanelTarget>,
) {
    let Some(menu) = layout.menu else {
        return;
    };
    cx.backend.fill_round_rect(menu, MENU_RADIUS, theme.card);
    cx.backend
        .stroke_round_rect(menu, MENU_RADIUS, theme.border, 1.0);
    for (index, row) in MENU_ROWS.iter().enumerate() {
        let Some(rect) = layout.menu_row_rect(index) else {
            continue;
        };
        let enabled = layout.row_enabled(*row);
        if enabled && hover == Some(*row) {
            menu_paint::paint_row_tint(
                cx,
                theme,
                rect.origin.x,
                rect.origin.y,
                rect.size.x,
                rect.size.y,
            );
        }
        let label = match row {
            SlidesPanelTarget::ExportSelectedSlides => labels.export_selected,
            _ => labels.export_all,
        };
        // A disabled row reads as unavailable rather than absent: it is
        // what tells the user that selecting slides is the missing step.
        let color = if enabled {
            theme.foreground
        } else {
            fade(theme.muted_foreground, 0.55)
        };
        let label = crate::widgets::file_menu::truncate_to_width(
            cx,
            label,
            13.0,
            (rect.size.x - MENU_PAD_X * 2.0).max(0.0),
        );
        cx.backend.draw_text(
            &TextLayout::single_run(&label, "system-ui", 13.0, color.to_jian(), Point2D::ZERO),
            Point2D::new(
                rect.origin.x + MENU_PAD_X,
                centered_text_baseline_y(rect, 13.0),
            ),
        );
    }
}

/// `▷ Present` — the bar's primary action, so it keeps the filled
/// treatment the footer's full-width button had.
fn paint_present_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    layout: &SlidesActionLayout,
    label: &str,
    hover: Option<SlidesPanelTarget>,
) {
    let button = layout.present;
    if button.size.x <= 0.0 {
        return;
    }
    let hovered = hover == Some(SlidesPanelTarget::Present);
    cx.backend.fill_round_rect(
        button,
        BUTTON_RADIUS,
        if hovered {
            theme.primary
        } else {
            Color {
                a: 0.86,
                ..theme.primary
            }
        },
    );
    let label = crate::widgets::file_menu::truncate_to_width(
        cx,
        label,
        LABEL_FONT,
        (button.size.x - BUTTON_PAD_X * 2.0 - PLAY_ICON - LABEL_GAP).max(0.0),
    );
    let label_w = text_metrics::measure_chrome_weighted(cx.backend, &label, LABEL_FONT, 600);
    let content_w = PLAY_ICON + LABEL_GAP + label_w;
    let icon_x = button.origin.x + (button.size.x - content_w) / 2.0;
    draw_icon(
        cx.backend,
        Icon::Play,
        Point2D::new(icon_x, button.origin.y + (button.size.y - PLAY_ICON) / 2.0),
        PLAY_ICON,
        theme.primary_foreground,
        1.6,
    );
    if !label.is_empty() {
        cx.backend.draw_text(
            &TextLayout::single_run(
                &label,
                "system-ui",
                LABEL_FONT,
                theme.primary_foreground.to_jian(),
                Point2D::ZERO,
            )
            .with_font_weight(600),
            Point2D::new(
                icon_x + PLAY_ICON + LABEL_GAP,
                centered_text_baseline_y(button, LABEL_FONT),
            ),
        );
    }
}

/// `Export PDF ⌄` — the secondary action, so it is an outlined button
/// rather than a second filled one competing with Present.
fn paint_export_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    layout: &SlidesActionLayout,
    label: &str,
    hover: Option<SlidesPanelTarget>,
) {
    let button = layout.export;
    if button.size.x <= 0.0 {
        return;
    }
    // Held open counts as engaged, not just hovered: the button is the
    // menu's anchor and has to look pressed while the menu is showing.
    let lit = hover == Some(SlidesPanelTarget::ExportMenu) || layout.menu.is_some();
    cx.backend.fill_round_rect(
        button,
        BUTTON_RADIUS,
        if lit { theme.button_hover } else { theme.muted },
    );
    cx.backend
        .stroke_round_rect(button, BUTTON_RADIUS, theme.border, 1.0);
    let label = crate::widgets::file_menu::truncate_to_width(
        cx,
        label,
        LABEL_FONT,
        (button.size.x - BUTTON_PAD_X * 2.0 - CHEVRON_SIZE - CHEVRON_GAP).max(0.0),
    );
    let label_w = text_metrics::measure_chrome_weighted(cx.backend, &label, LABEL_FONT, 500);
    let content_w = label_w + CHEVRON_GAP + CHEVRON_SIZE;
    let text_x = button.origin.x + (button.size.x - content_w) / 2.0;
    if !label.is_empty() {
        cx.backend.draw_text(
            &TextLayout::single_run(
                &label,
                "system-ui",
                LABEL_FONT,
                theme.foreground.to_jian(),
                Point2D::ZERO,
            )
            .with_font_weight(500),
            Point2D::new(text_x, centered_text_baseline_y(button, LABEL_FONT)),
        );
    }
    // Points the way the menu opens, so the glyph is not lying about
    // where the rows will appear.
    draw_icon(
        cx.backend,
        if layout.menu.is_some() {
            Icon::ChevronDown
        } else {
            Icon::ChevronUp
        },
        Point2D::new(
            text_x + label_w + CHEVRON_GAP,
            button.origin.y + (button.size.y - CHEVRON_SIZE) / 2.0,
        ),
        CHEVRON_SIZE,
        theme.muted_foreground,
        1.6,
    );
}

fn fade(color: Color, alpha: f32) -> Color {
    Color {
        a: color.a * alpha,
        ..color
    }
}

fn contains(rect: Rect, point: Point2D) -> bool {
    super::slides_panel::contains(rect, point)
}

#[cfg(test)]
#[path = "slides_panel_actions_tests.rs"]
mod slides_panel_actions_tests;
