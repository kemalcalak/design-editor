//! Rename-layers preview (#383): a centred modal listing every layer the
//! rules (or the agent) propose a name for. Each row has a tick (rename or
//! keep), the current name, and the proposed name, which a click turns
//! into an inline text field. Rename applies the ticked rows as one undo
//! step; Cancel, Escape or the scrim leave the document untouched.

use crate::theme::Theme;
use crate::widgets::chrome_buttons::{fit_button_width, measure_button_labels};
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics;
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::rename_layers::{RenameLayersDialog, RenameLayersHover, RenameLayersSource};
use de_editor_core::{EditorState, EditorUiState};
use jian_widgets::centered_text_baseline_y;

const PANEL_WIDTH: f32 = 560.0;
const HEADER_HEIGHT: f32 = 72.0;
const ROW_HEIGHT: f32 = 34.0;
const ROWS_MAX_HEIGHT: f32 = 360.0;
const EMPTY_HEIGHT: f32 = 56.0;
const FOOTER_HEIGHT: f32 = 60.0;
const PAD: f32 = 16.0;
const TOGGLE_SIZE: f32 = 16.0;
const CURRENT_X: f32 = PAD + TOGGLE_SIZE + 12.0;
const CURRENT_WIDTH: f32 = 170.0;
const NAME_X: f32 = CURRENT_X + CURRENT_WIDTH + 24.0;
const NAME_HEIGHT: f32 = 26.0;
const BUTTON_HEIGHT: f32 = 30.0;
const FONT: f32 = 12.0;

/// What a press landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameLayersHit {
    Toggle(usize),
    Name(usize),
    Apply,
    Cancel,
    /// On the card but on no control.
    Inside,
    /// On the scrim. The dialog is modal, so the press is still consumed.
    Outside,
}

pub struct RenameLayersPanel<'a> {
    id: WidgetId,
    theme: Theme,
    ui: &'a EditorUiState,
    dialog: &'a RenameLayersDialog,
}

impl<'a> RenameLayersPanel<'a> {
    /// `None` while the preview is closed.
    pub fn for_editor(state: &'a EditorState) -> Option<Self> {
        let ui = &state.editor_ui;
        Some(Self {
            id: WidgetId::new(5480),
            theme: theme_for(ui),
            dialog: ui.rename_layers.as_ref()?,
            ui,
        })
    }

    fn rows_viewport_height(&self) -> f32 {
        if self.dialog.rows.is_empty() {
            return EMPTY_HEIGHT;
        }
        (self.dialog.rows.len() as f32 * ROW_HEIGHT).min(ROWS_MAX_HEIGHT)
    }

    fn height(&self) -> f32 {
        HEADER_HEIGHT + self.rows_viewport_height() + FOOTER_HEIGHT
    }

    /// Centred, clamped below the window strip on a short viewport.
    pub fn rect(&self, viewport_w: f32, viewport_h: f32) -> Rect {
        let width = PANEL_WIDTH.min((viewport_w - 32.0).max(320.0));
        let height = self.height();
        Rect {
            origin: Point2D::new(
                ((viewport_w - width) / 2.0).max(0.0),
                ((viewport_h - height) / 2.0).max(crate::widgets::WINDOW_STRIP_H + 16.0),
            ),
            size: Point2D::new(width, height),
        }
    }

    pub fn rows_rect(&self, panel: Rect) -> Rect {
        Rect {
            origin: Point2D::new(panel.origin.x, panel.origin.y + HEADER_HEIGHT),
            size: Point2D::new(panel.size.x, self.rows_viewport_height()),
        }
    }

    pub fn max_rows_scroll(&self) -> f32 {
        let content = self.dialog.rows.len() as f32 * ROW_HEIGHT;
        (content - self.rows_viewport_height()).max(0.0)
    }

    fn scroll(&self) -> f32 {
        self.dialog.scroll.offset.clamp(0.0, self.max_rows_scroll())
    }

    fn row_rect(&self, panel: Rect, index: usize) -> Rect {
        let rows = self.rows_rect(panel);
        Rect {
            origin: Point2D::new(
                rows.origin.x,
                rows.origin.y + index as f32 * ROW_HEIGHT - self.scroll(),
            ),
            size: Point2D::new(rows.size.x, ROW_HEIGHT),
        }
    }

    fn toggle_rect(row: Rect) -> Rect {
        Rect {
            origin: Point2D::new(
                row.origin.x + PAD,
                row.origin.y + (ROW_HEIGHT - TOGGLE_SIZE) / 2.0,
            ),
            size: Point2D::new(TOGGLE_SIZE, TOGGLE_SIZE),
        }
    }

    fn name_rect(row: Rect) -> Rect {
        Rect {
            origin: Point2D::new(
                row.origin.x + NAME_X,
                row.origin.y + (ROW_HEIGHT - NAME_HEIGHT) / 2.0,
            ),
            size: Point2D::new((row.size.x - NAME_X - PAD).max(40.0), NAME_HEIGHT),
        }
    }

    fn apply_label(&self) -> &'static str {
        translate(self.ui, "renameLayers.apply")
    }

    pub fn apply_rect(&self, panel: Rect) -> Rect {
        let width = fit_button_width(self.apply_label(), 13.0);
        Rect {
            origin: Point2D::new(
                panel.origin.x + panel.size.x - PAD - width,
                panel.origin.y + panel.size.y - (FOOTER_HEIGHT + BUTTON_HEIGHT) / 2.0,
            ),
            size: Point2D::new(width, BUTTON_HEIGHT),
        }
    }

    pub fn cancel_rect(&self, panel: Rect) -> Rect {
        let width = fit_button_width(translate(self.ui, "common.cancel"), 13.0);
        let apply = self.apply_rect(panel);
        Rect {
            origin: Point2D::new(apply.origin.x - 8.0 - width, apply.origin.y),
            size: Point2D::new(width, BUTTON_HEIGHT),
        }
    }

    pub fn hit_test(&self, panel: Rect, point: Point2D) -> RenameLayersHit {
        if !panel.contains(point) {
            return RenameLayersHit::Outside;
        }
        if self.apply_rect(panel).contains(point) {
            return RenameLayersHit::Apply;
        }
        if self.cancel_rect(panel).contains(point) {
            return RenameLayersHit::Cancel;
        }
        let rows = self.rows_rect(panel);
        if rows.contains(point) && !self.dialog.rows.is_empty() {
            let index = ((point.y - rows.origin.y + self.scroll()) / ROW_HEIGHT).floor();
            if index >= 0.0 && (index as usize) < self.dialog.rows.len() {
                let index = index as usize;
                let row = self.row_rect(panel, index);
                if point.x < row.origin.x + NAME_X - 12.0 {
                    return RenameLayersHit::Toggle(index);
                }
                if Self::name_rect(row).contains(point) {
                    return RenameLayersHit::Name(index);
                }
            }
        }
        RenameLayersHit::Inside
    }

    pub fn hover_at(&self, panel: Rect, point: Point2D) -> Option<RenameLayersHover> {
        match self.hit_test(panel, point) {
            RenameLayersHit::Toggle(index) => Some(RenameLayersHover::Toggle(index)),
            RenameLayersHit::Name(index) => Some(RenameLayersHover::Name(index)),
            RenameLayersHit::Apply => Some(RenameLayersHover::Apply),
            RenameLayersHit::Cancel => Some(RenameLayersHover::Cancel),
            RenameLayersHit::Inside | RenameLayersHit::Outside => None,
        }
    }

    /// `3 layers to rename · Offline rules`.
    pub fn summary_text(&self) -> String {
        let locale = self.ui.effective_locale();
        let accepted = self.dialog.accepted_count();
        let key = match de_i18n::plural_category(locale, accepted as i64) {
            de_i18n::PluralCategory::One => "renameLayers.summary_one",
            _ => "renameLayers.summary_other",
        };
        let count = accepted.to_string();
        let summary = de_i18n::translate_with(locale, key, &[("count", count.as_str())]);
        let source = translate(
            self.ui,
            match self.dialog.source {
                RenameLayersSource::Rules => "renameLayers.sourceRules",
                RenameLayersSource::AiRunning => "renameLayers.sourceAiRunning",
                RenameLayersSource::Ai => "renameLayers.sourceAi",
                RenameLayersSource::AiFailed => "renameLayers.sourceAiFailed",
            },
        );
        format!("{summary} · {source}")
    }
}

fn paint_text(
    cx: &mut PaintCx<'_>,
    text: &str,
    origin: Point2D,
    size: f32,
    weight: u16,
    color: Color,
) {
    let layout = TextLayout::single_run(
        text,
        text_metrics::CHROME_FONT_FAMILY,
        size,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    )
    .with_font_weight(weight);
    cx.backend.draw_text(&layout, origin);
}

fn paint_clipped(cx: &mut PaintCx<'_>, clip: Rect, text: &str, color: Color) {
    cx.backend.save();
    cx.backend.clip_rect(clip);
    paint_text(
        cx,
        text,
        Point2D::new(clip.origin.x, centered_text_baseline_y(clip, FONT)),
        FONT,
        400,
        color,
    );
    cx.backend.restore();
}

impl RenameLayersPanel<'_> {
    fn paint_row(&self, cx: &mut PaintCx<'_>, row: Rect, index: usize, now_ms: u64) {
        let Some(item) = self.dialog.rows.get(index) else {
            return;
        };
        let theme = &self.theme;
        let hover = self.dialog.hover;
        if index > 0 {
            cx.backend.fill_rect(
                Rect {
                    origin: Point2D::new(row.origin.x + PAD, row.origin.y),
                    size: Point2D::new(row.size.x - PAD * 2.0, 1.0),
                },
                theme.border,
            );
        }
        let toggle = Self::toggle_rect(row);
        if item.accepted {
            cx.backend.fill_round_rect(toggle, 4.0, theme.primary);
            draw_icon(
                cx.backend,
                Icon::Check,
                Point2D::new(toggle.origin.x + 2.0, toggle.origin.y + 2.0),
                TOGGLE_SIZE - 4.0,
                theme.primary_foreground,
                2.0,
            );
        } else {
            let stroke = if hover == Some(RenameLayersHover::Toggle(index)) {
                theme.foreground
            } else {
                theme.border
            };
            cx.backend.stroke_round_rect(toggle, 4.0, stroke, 1.0);
        }
        let current = Rect {
            origin: Point2D::new(row.origin.x + CURRENT_X, row.origin.y),
            size: Point2D::new(CURRENT_WIDTH, ROW_HEIGHT),
        };
        let current_text = if item.current.is_empty() {
            "—"
        } else {
            item.current.as_str()
        };
        paint_clipped(cx, current, current_text, theme.muted_foreground);
        paint_text(
            cx,
            "→",
            Point2D::new(
                row.origin.x + CURRENT_X + CURRENT_WIDTH + 6.0,
                centered_text_baseline_y(row, FONT),
            ),
            FONT,
            400,
            theme.muted_foreground,
        );
        let field = Self::name_rect(row);
        let editing = self.dialog.editing == Some(index);
        if editing {
            cx.backend.fill_round_rect(field, 6.0, theme.background);
            cx.backend.stroke_round_rect(field, 6.0, theme.ring, 1.0);
        } else if hover == Some(RenameLayersHover::Name(index)) {
            cx.backend.fill_round_rect(field, 6.0, theme.accent);
        }
        let inner = Rect {
            origin: Point2D::new(field.origin.x + 8.0, field.origin.y),
            size: Point2D::new(field.size.x - 16.0, field.size.y),
        };
        if editing {
            let text = self.dialog.input.text();
            paint_clipped(cx, inner, text, theme.foreground);
            if self.dialog.input.caret_visible(now_ms) {
                let caret = self.dialog.input.caret().min(text.len());
                let prefix = text.get(..caret).unwrap_or(text);
                let x = inner.origin.x + text_metrics::measure_chrome(cx.backend, prefix, FONT);
                cx.backend.fill_rect(
                    Rect {
                        origin: Point2D::new(
                            x.min(inner.origin.x + inner.size.x),
                            field.origin.y + 6.0,
                        ),
                        size: Point2D::new(1.0, NAME_HEIGHT - 12.0),
                    },
                    theme.foreground,
                );
            }
        } else {
            let color = if item.accepted {
                theme.foreground
            } else {
                theme.muted_foreground
            };
            paint_clipped(cx, inner, &item.proposed, color);
        }
    }

    fn paint_button(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        label: &str,
        primary: bool,
        hovered: bool,
    ) {
        let theme = &self.theme;
        let enabled = !primary || self.dialog.accepted_count() > 0;
        let (fill, text) = match (primary, enabled) {
            (true, true) => (Some(theme.primary), theme.primary_foreground),
            (true, false) => (Some(theme.accent), theme.muted_foreground),
            (false, _) if hovered => (Some(theme.accent), theme.foreground),
            (false, _) => (None, theme.foreground),
        };
        if let Some(fill) = fill {
            cx.backend.fill_round_rect(rect, 8.0, fill);
        } else {
            cx.backend.stroke_round_rect(rect, 8.0, theme.border, 1.0);
        }
        let width = text_metrics::measure_chrome(cx.backend, label, 13.0);
        paint_text(
            cx,
            label,
            Point2D::new(
                rect.origin.x + (rect.size.x - width) / 2.0,
                centered_text_baseline_y(rect, 13.0),
            ),
            13.0,
            500,
            text,
        );
    }

    /// Paint with the editor clock so the inline field's caret blinks.
    pub fn paint_at(&self, cx: &mut PaintCx<'_>, panel: Rect, now_ms: u64) {
        measure_button_labels(
            cx.backend,
            13.0,
            &[self.apply_label(), translate(self.ui, "common.cancel")],
        );
        let theme = &self.theme;
        cx.backend.fill_round_rect(panel, 12.0, theme.card);
        cx.backend.stroke_round_rect(panel, 12.0, theme.border, 1.0);
        paint_text(
            cx,
            translate(self.ui, "renameLayers.title"),
            Point2D::new(panel.origin.x + PAD, panel.origin.y + 30.0),
            15.0,
            600,
            theme.foreground,
        );
        let summary = text_metrics::fit_chrome(
            cx.backend,
            &self.summary_text(),
            (panel.size.x - PAD * 2.0).max(0.0),
            FONT,
        );
        paint_text(
            cx,
            &summary,
            Point2D::new(panel.origin.x + PAD, panel.origin.y + 52.0),
            FONT,
            400,
            theme.muted_foreground,
        );
        let rows = self.rows_rect(panel);
        if self.dialog.rows.is_empty() {
            paint_text(
                cx,
                translate(self.ui, "renameLayers.empty"),
                Point2D::new(rows.origin.x + PAD, centered_text_baseline_y(rows, 13.0)),
                13.0,
                400,
                theme.muted_foreground,
            );
        } else {
            cx.backend.save();
            cx.backend.clip_rect(rows);
            let first = (self.scroll() / ROW_HEIGHT).floor().max(0.0) as usize;
            let visible = (rows.size.y / ROW_HEIGHT).ceil() as usize + 1;
            for index in first..(first + visible).min(self.dialog.rows.len()) {
                self.paint_row(cx, self.row_rect(panel, index), index, now_ms);
            }
            cx.backend.restore();
        }
        let hover = self.dialog.hover;
        self.paint_button(
            cx,
            self.cancel_rect(panel),
            translate(self.ui, "common.cancel"),
            false,
            hover == Some(RenameLayersHover::Cancel),
        );
        self.paint_button(
            cx,
            self.apply_rect(panel),
            self.apply_label(),
            true,
            hover == Some(RenameLayersHover::Apply),
        );
    }
}

impl Widget for RenameLayersPanel<'_> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(PANEL_WIDTH, self.height()),
            },
        }
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Dialog);
        node.set_label(format!(
            "{} — {}",
            translate(self.ui, "renameLayers.title"),
            self.summary_text()
        ));
        node
    }

    fn paint(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        self.paint_at(cx, panel, 0);
    }
}
