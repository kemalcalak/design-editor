//! The left rail's tab row (#30) — the header that switches the rail
//! between the Agent conversation, the Layers tree, the component
//! browser, the document's images and (for a deck) the slides navigator.
//!
//! Split out of `slides_panel.rs` at the 800-line ceiling.
//! `SlidesPanelTabs` is re-exported from the parent, so every import
//! path is unchanged.
//!
//! The row matches the design: the tab on show is a soft pill with its
//! glyph and label, every other tab is its glyph alone (named by a hover
//! tooltip). Which tabs EXIST is the flow's decision ([`SlidesTabRow`]):
//! the Chat tab needs an editor-owned conversation, the slides tab needs
//! boards; Layers, Components and Images are always there.

use de_editor_core::{LeftPanelTab, SlidesPanelTarget};

use super::{
    contains, TAB_FONT, TAB_GAP, TAB_ICON_GAP, TAB_ICON_SIZE, TAB_INSET_X, TAB_INSET_Y, TAB_PAD_X,
    TAB_RADIUS, TAB_ROW_HEIGHT,
};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout, Theme};

/// Most tabs the row can show.
const MAX_TABS: usize = 5;

/// The row's labels plus which of the optional tabs exist.
///
/// Resolved once by the flow for a document, then fed to layout AND
/// paint, so the rects the hit-test uses can never be laid out from a
/// different tab set than the one painted.
pub struct SlidesTabRow<'a> {
    pub chat_label: &'a str,
    pub layers_label: &'a str,
    pub components_label: &'a str,
    pub images_label: &'a str,
    pub slides_label: &'a str,
    /// Whether the Chat tab is offered.
    pub chat_available: bool,
    /// The slides tab needs boards to list — see the flow's
    /// `slides_tab_available`.
    pub slides_available: bool,
}

impl SlidesTabRow<'_> {
    /// The tabs on show, in row order: Agent · Layers · Components ·
    /// Images · Slides.
    fn tabs(&self) -> Vec<LeftPanelTab> {
        let mut tabs = Vec::with_capacity(MAX_TABS);
        if self.chat_available {
            tabs.push(LeftPanelTab::Chat);
        }
        tabs.extend([
            LeftPanelTab::Layers,
            LeftPanelTab::Components,
            LeftPanelTab::Images,
        ]);
        if self.slides_available {
            tabs.push(LeftPanelTab::Slides);
        }
        tabs
    }

    pub fn label(&self, tab: LeftPanelTab) -> &str {
        match tab {
            LeftPanelTab::Chat => self.chat_label,
            LeftPanelTab::Layers => self.layers_label,
            LeftPanelTab::Components => self.components_label,
            LeftPanelTab::Images => self.images_label,
            LeftPanelTab::Slides => self.slides_label,
        }
    }
}

/// The glyph a tab paints.
fn tab_icon(tab: LeftPanelTab) -> Icon {
    match tab {
        LeftPanelTab::Chat => Icon::MessageCircle,
        LeftPanelTab::Layers => Icon::LayersStack,
        LeftPanelTab::Components => Icon::Component,
        LeftPanelTab::Images => Icon::Image,
        LeftPanelTab::Slides => Icon::PresentationScreen,
    }
}

/// Rects of the tab row that heads the rail.
///
/// Resolved on its own (not only as part of the full slides layout)
/// because the row also paints — and takes clicks — while another tab
/// owns the rest of the rail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlidesPanelTabs {
    pub row: Rect,
    /// The tabs on show with their rects, in row order; unused slots are
    /// `None`.
    rects: [Option<(LeftPanelTab, Rect)>; MAX_TABS],
    /// The tab on show — the one that is a labelled pill. Stored because
    /// it is wider than the others, so the rects depend on it.
    pub active: LeftPanelTab,
}

impl SlidesPanelTabs {
    /// Lay the tab row across the top of `panel`, left to right from the
    /// rail's inset: the active tab as `[glyph label]`, the rest as their
    /// glyph. Each tab is capped at what its predecessors left, so even a
    /// very narrow rail never paints past its edge.
    ///
    /// The label width uses [`estimated_text_width`], not the backend's
    /// real shaper, because this rect has to be identical at paint time
    /// and at hit-test time and the hit-test has no backend; paint fits
    /// the real glyphs into it.
    pub fn new(panel: Rect, active: LeftPanelTab, tabs: &SlidesTabRow<'_>) -> Self {
        let row = Rect {
            origin: panel.origin,
            size: Point2D::new(panel.size.x, TAB_ROW_HEIGHT),
        };
        let inner_w = (row.size.x - TAB_INSET_X * 2.0).max(0.0);
        let x = row.origin.x + TAB_INSET_X;
        let y = row.origin.y + TAB_INSET_Y;
        let h = (row.size.y - TAB_INSET_Y * 2.0).max(0.0);
        let icon_only_w = TAB_ICON_SIZE + TAB_PAD_X * 2.0;
        let labelled_w = |label: &str| {
            TAB_ICON_SIZE + TAB_ICON_GAP + estimated_text_width(label, TAB_FONT) + TAB_PAD_X * 2.0
        };
        let mut rects = [None; MAX_TABS];
        let mut cursor = 0.0_f32;
        for (slot, tab) in rects.iter_mut().zip(tabs.tabs()) {
            let remaining = (inner_w - cursor).max(0.0);
            let wanted = if tab == active {
                labelled_w(tabs.label(tab))
            } else {
                icon_only_w
            };
            let w = wanted.min(remaining);
            *slot = Some((tab, Rect::xywh(x + cursor, y, w, h)));
            cursor += w + TAB_GAP;
        }
        Self { row, rects, active }
    }

    fn shown(&self) -> impl Iterator<Item = (LeftPanelTab, Rect)> + '_ {
        self.rects.iter().flatten().copied()
    }

    /// The rect `tab` occupies, or `Rect::ZERO` when this document does
    /// not show it.
    pub fn rect_of(&self, tab: LeftPanelTab) -> Rect {
        self.shown()
            .find(|(candidate, _)| *candidate == tab)
            .map_or(Rect::ZERO, |(_, rect)| rect)
    }

    /// Which tab `point` lands on, or `None` when it is off the row or
    /// between tabs.
    pub fn hit(&self, point: Point2D) -> Option<SlidesPanelTarget> {
        if !contains(self.row, point) {
            return None;
        }
        self.shown()
            .find(|(_, rect)| rect.size.x > 0.0 && contains(*rect, point))
            .map(|(tab, _)| tab.target())
    }

    /// The rail below the tab row — what the tab on show gets.
    pub fn content_rect(&self, panel: Rect) -> Rect {
        let row_height = self.row.size.y;
        Rect {
            origin: Point2D::new(panel.origin.x, panel.origin.y + row_height),
            size: Point2D::new(panel.size.x, (panel.size.y - row_height).max(0.0)),
        }
    }

    /// Paint the tab row. `hover` is whichever tab the cursor is over,
    /// if any.
    ///
    /// The labels MUST be the same set `new` was given — they decide
    /// the pill's width there and are drawn inside it here.
    pub fn paint(
        &self,
        cx: &mut PaintCx<'_>,
        theme: &Theme,
        hover: Option<SlidesPanelTarget>,
        tabs: &SlidesTabRow<'_>,
    ) {
        cx.backend.fill_rect(self.row, theme.card);
        for (tab, rect) in self.shown() {
            if rect.size.x <= 0.0 {
                continue;
            }
            let selected = self.active == tab;
            if selected {
                cx.backend.fill_round_rect(rect, TAB_RADIUS, theme.muted);
            } else if hover == Some(tab.target()) {
                cx.backend
                    .fill_round_rect(rect, TAB_RADIUS, theme.button_hover);
            }
            let color = if selected || hover == Some(tab.target()) {
                theme.foreground
            } else {
                theme.muted_foreground
            };
            let icon = tab_icon(tab);
            let icon_y = rect.origin.y + (rect.size.y - TAB_ICON_SIZE) / 2.0;
            if !selected {
                // Glyph alone, centred in its square.
                draw_icon(
                    cx.backend,
                    icon,
                    Point2D::new(rect.origin.x + (rect.size.x - TAB_ICON_SIZE) / 2.0, icon_y),
                    TAB_ICON_SIZE,
                    color,
                    1.6,
                );
                continue;
            }
            // The one labelled pill: glyph then label, the pair centred
            // together so the pill does not look left-heavy.
            let label = text_metrics::fit_chrome(
                cx.backend,
                tabs.label(tab),
                (rect.size.x - TAB_PAD_X * 2.0 - TAB_ICON_SIZE - TAB_ICON_GAP).max(0.0),
                TAB_FONT,
            );
            let label_w = text_metrics::measure_chrome_weighted(cx.backend, &label, TAB_FONT, 600);
            let content_w = TAB_ICON_SIZE + TAB_ICON_GAP + label_w;
            let icon_x = rect.origin.x + (rect.size.x - content_w) / 2.0;
            draw_icon(
                cx.backend,
                icon,
                Point2D::new(icon_x, icon_y),
                TAB_ICON_SIZE,
                color,
                1.6,
            );
            if !label.is_empty() {
                cx.backend.draw_text(
                    &TextLayout::single_run(
                        &label,
                        text_metrics::CHROME_FONT_FAMILY,
                        TAB_FONT,
                        color.to_jian(),
                        Point2D::ZERO,
                    )
                    .with_font_weight(600),
                    Point2D::new(
                        icon_x + TAB_ICON_SIZE + TAB_ICON_GAP,
                        jian_widgets::centered_text_baseline_y(rect, TAB_FONT),
                    ),
                );
            }
        }
    }
}

/// Backend-free width estimate, deliberately conservative: ASCII is
/// charged 0.68 em and everything else a full em, so the answer runs a
/// little wide of what the shaper will produce. The row needs the SAME
/// number at paint time and at hit-test time and has no backend at
/// hit-test time — over-estimating drops the row to icons a few pixels
/// early rather than clipping a label.
pub(crate) fn estimated_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|c| {
            if c.is_ascii() {
                font_size * 0.68
            } else {
                font_size
            }
        })
        .sum()
}
