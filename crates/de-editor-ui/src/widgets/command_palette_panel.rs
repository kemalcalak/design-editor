//! Command palette / find layers (#395): a modal card near the top of the
//! window. A search field, in find mode a replace field with Replace all,
//! the ranked rows (label, muted detail, right-aligned chord / kind /
//! page), and a footer with the match summary and the key hints.
//!
//! Every string painted here is either a `&'static str` translation or
//! text the core formatted when the results changed, so a frame never
//! allocates for labels.

use crate::theme::Theme;
use crate::widgets::chrome_buttons::{fit_button_width, measure_button_labels};
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics;
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::command_palette::{
    CommandPaletteState, PaletteField, PaletteHover, PaletteMode, ROW_HEIGHT, VISIBLE_ROWS,
};
use de_editor_core::{EditorState, EditorUiState};
use jian_core::text_input::TextInputState;
use jian_widgets::centered_text_baseline_y;

const PANEL_WIDTH: f32 = 640.0;
const INPUT_HEIGHT: f32 = 52.0;
const REPLACE_HEIGHT: f32 = 44.0;
const EMPTY_HEIGHT: f32 = 44.0;
const FOOTER_HEIGHT: f32 = 30.0;
const PAD: f32 = 16.0;
const ICON_SIZE: f32 = 16.0;
const INPUT_FONT: f32 = 15.0;
const ROW_FONT: f32 = 13.0;
const SMALL_FONT: f32 = 11.0;
const BUTTON_HEIGHT: f32 = 28.0;
/// Top of the card as a share of the viewport height.
const TOP_RATIO: f32 = 0.14;

/// What a press landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPaletteHit {
    Query,
    Replace,
    ReplaceAll,
    Row(usize),
    /// On the card but on no control.
    Inside,
    /// On the scrim: closes the palette.
    Outside,
}

pub struct CommandPalettePanel<'a> {
    id: WidgetId,
    theme: Theme,
    ui: &'a EditorUiState,
    palette: &'a CommandPaletteState,
}

impl<'a> CommandPalettePanel<'a> {
    /// `None` while the palette is closed.
    pub fn for_editor(state: &'a EditorState) -> Option<Self> {
        let ui = &state.editor_ui;
        Some(Self {
            id: WidgetId::new(5495),
            theme: theme_for(ui),
            palette: ui.command_palette.as_ref()?,
            ui,
        })
    }

    fn finding(&self) -> bool {
        self.palette.mode == PaletteMode::FindLayers
    }

    fn replace_height(&self) -> f32 {
        if self.finding() {
            REPLACE_HEIGHT
        } else {
            0.0
        }
    }

    fn rows_viewport_height(&self) -> f32 {
        if self.palette.rows.is_empty() {
            return EMPTY_HEIGHT;
        }
        self.palette.rows.len().min(VISIBLE_ROWS) as f32 * ROW_HEIGHT
    }

    fn height(&self) -> f32 {
        INPUT_HEIGHT + self.replace_height() + self.rows_viewport_height() + FOOTER_HEIGHT
    }

    /// Centred horizontally, near the top, clamped below the window strip.
    pub fn rect(&self, viewport_w: f32, viewport_h: f32) -> Rect {
        let width = PANEL_WIDTH.min((viewport_w - 32.0).max(320.0));
        let height = self.height();
        let top = (viewport_h * TOP_RATIO)
            .min((viewport_h - height - 16.0).max(0.0))
            .max(crate::widgets::WINDOW_STRIP_H + 16.0);
        Rect {
            origin: Point2D::new(((viewport_w - width) / 2.0).max(0.0), top),
            size: Point2D::new(width, height),
        }
    }

    pub fn query_rect(&self, panel: Rect) -> Rect {
        Rect {
            origin: panel.origin,
            size: Point2D::new(panel.size.x, INPUT_HEIGHT),
        }
    }

    pub fn replace_rect(&self, panel: Rect) -> Option<Rect> {
        self.finding().then(|| Rect {
            origin: Point2D::new(panel.origin.x, panel.origin.y + INPUT_HEIGHT),
            size: Point2D::new(panel.size.x, REPLACE_HEIGHT),
        })
    }

    fn replace_all_label(&self) -> &'static str {
        translate(self.ui, "palette.replaceAll")
    }

    pub fn replace_all_rect(&self, panel: Rect) -> Option<Rect> {
        let row = self.replace_rect(panel)?;
        let width = fit_button_width(self.replace_all_label(), 12.0);
        Some(Rect {
            origin: Point2D::new(
                row.origin.x + row.size.x - PAD - width,
                row.origin.y + (REPLACE_HEIGHT - BUTTON_HEIGHT) / 2.0,
            ),
            size: Point2D::new(width, BUTTON_HEIGHT),
        })
    }

    pub fn rows_rect(&self, panel: Rect) -> Rect {
        Rect {
            origin: Point2D::new(
                panel.origin.x,
                panel.origin.y + INPUT_HEIGHT + self.replace_height(),
            ),
            size: Point2D::new(panel.size.x, self.rows_viewport_height()),
        }
    }

    pub fn max_rows_scroll(&self) -> f32 {
        let content = self.palette.rows.len() as f32 * ROW_HEIGHT;
        (content - self.rows_viewport_height()).max(0.0)
    }

    fn scroll(&self) -> f32 {
        self.palette
            .scroll
            .offset
            .clamp(0.0, self.max_rows_scroll())
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

    pub fn hit_test(&self, panel: Rect, point: Point2D) -> CommandPaletteHit {
        if !panel.contains(point) {
            return CommandPaletteHit::Outside;
        }
        if self.query_rect(panel).contains(point) {
            return CommandPaletteHit::Query;
        }
        if self
            .replace_all_rect(panel)
            .is_some_and(|rect| rect.contains(point))
        {
            return CommandPaletteHit::ReplaceAll;
        }
        if self
            .replace_rect(panel)
            .is_some_and(|rect| rect.contains(point))
        {
            return CommandPaletteHit::Replace;
        }
        let rows = self.rows_rect(panel);
        if rows.contains(point) && !self.palette.rows.is_empty() {
            let index = ((point.y - rows.origin.y + self.scroll()) / ROW_HEIGHT).floor();
            if index >= 0.0 && (index as usize) < self.palette.rows.len() {
                return CommandPaletteHit::Row(index as usize);
            }
        }
        CommandPaletteHit::Inside
    }

    pub fn hover_at(&self, panel: Rect, point: Point2D) -> Option<PaletteHover> {
        match self.hit_test(panel, point) {
            CommandPaletteHit::Row(index) => Some(PaletteHover::Row(index)),
            CommandPaletteHit::ReplaceAll => Some(PaletteHover::ReplaceAll),
            _ => None,
        }
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

/// Text clipped to `clip`, vertically centred in it.
fn paint_clipped(cx: &mut PaintCx<'_>, clip: Rect, text: &str, size: f32, color: Color) {
    if clip.size.x <= 0.0 {
        return;
    }
    cx.backend.save();
    cx.backend.clip_rect(clip);
    paint_text(
        cx,
        text,
        Point2D::new(clip.origin.x, centered_text_baseline_y(clip, size)),
        size,
        400,
        color,
    );
    cx.backend.restore();
}

impl CommandPalettePanel<'_> {
    /// One text field: selection wash, text or placeholder, blinking caret.
    #[allow(clippy::too_many_arguments)]
    fn paint_field(
        &self,
        cx: &mut PaintCx<'_>,
        area: Rect,
        input: &TextInputState,
        placeholder: &'static str,
        focused: bool,
        font: f32,
        now_ms: u64,
    ) {
        let theme = &self.theme;
        let text = input.text();
        if text.is_empty() {
            paint_clipped(cx, area, placeholder, font, theme.muted_foreground);
        } else {
            if focused {
                let (start, end) = input.selection().ordered();
                if start != end {
                    let prefix = text.get(..start).unwrap_or("");
                    let selected = text.get(start..end).unwrap_or("");
                    let x = area.origin.x + text_metrics::measure_chrome(cx.backend, prefix, font);
                    let w = text_metrics::measure_chrome(cx.backend, selected, font);
                    cx.backend.fill_rect(
                        Rect {
                            origin: Point2D::new(
                                x,
                                area.origin.y + (area.size.y - font - 6.0) / 2.0,
                            ),
                            size: Point2D::new(w, font + 6.0),
                        },
                        theme.row_selected,
                    );
                }
            }
            paint_clipped(cx, area, text, font, theme.foreground);
        }
        if focused && input.caret_visible(now_ms) {
            let caret = input.caret().min(text.len());
            let prefix = text.get(..caret).unwrap_or(text);
            let x = area.origin.x + text_metrics::measure_chrome(cx.backend, prefix, font);
            cx.backend.fill_rect(
                Rect {
                    origin: Point2D::new(
                        x.min(area.origin.x + area.size.x),
                        area.origin.y + (area.size.y - font - 4.0) / 2.0,
                    ),
                    size: Point2D::new(1.0, font + 4.0),
                },
                theme.foreground,
            );
        }
    }

    fn paint_row(&self, cx: &mut PaintCx<'_>, rect: Rect, index: usize) {
        let Some(row) = self.palette.rows.get(index) else {
            return;
        };
        let theme = &self.theme;
        let inset = Rect {
            origin: Point2D::new(rect.origin.x + 6.0, rect.origin.y + 1.0),
            size: Point2D::new(rect.size.x - 12.0, rect.size.y - 2.0),
        };
        if index == self.palette.highlighted {
            cx.backend.fill_round_rect(inset, 6.0, theme.accent);
        } else if self.palette.hover == Some(PaletteHover::Row(index)) {
            cx.backend.fill_round_rect(inset, 6.0, theme.muted);
        }
        let trailing_width = if row.trailing.is_empty() {
            0.0
        } else {
            text_metrics::measure_chrome_weighted(cx.backend, &row.trailing, SMALL_FONT, 500)
        };
        let right = rect.origin.x + rect.size.x - PAD;
        if trailing_width > 0.0 {
            paint_text(
                cx,
                &row.trailing,
                Point2D::new(
                    right - trailing_width,
                    centered_text_baseline_y(rect, SMALL_FONT),
                ),
                SMALL_FONT,
                500,
                theme.muted_foreground,
            );
        }
        let text_right = right - trailing_width - if trailing_width > 0.0 { 16.0 } else { 0.0 };
        let label_x = rect.origin.x + PAD;
        let label_area = Rect {
            origin: Point2D::new(label_x, rect.origin.y),
            size: Point2D::new((text_right - label_x).max(0.0), ROW_HEIGHT),
        };
        paint_clipped(cx, label_area, &row.label, ROW_FONT, theme.foreground);
        if !row.secondary.is_empty() {
            let label_width = text_metrics::measure_chrome(cx.backend, &row.label, ROW_FONT);
            let x = label_x + label_width + 10.0;
            let area = Rect {
                origin: Point2D::new(x, rect.origin.y),
                size: Point2D::new((text_right - x).max(0.0), ROW_HEIGHT),
            };
            paint_clipped(cx, area, &row.secondary, ROW_FONT, theme.muted_foreground);
        }
    }

    /// Paint with the editor clock so the focused field's caret blinks.
    pub fn paint_at(&self, cx: &mut PaintCx<'_>, panel: Rect, now_ms: u64) {
        measure_button_labels(cx.backend, 12.0, &[self.replace_all_label()]);
        let theme = &self.theme;
        cx.backend.fill_round_rect(panel, 12.0, theme.popover);
        cx.backend.stroke_round_rect(panel, 12.0, theme.border, 1.0);

        let query = self.query_rect(panel);
        draw_icon(
            cx.backend,
            Icon::Search,
            Point2D::new(
                query.origin.x + PAD,
                query.origin.y + (INPUT_HEIGHT - ICON_SIZE) / 2.0,
            ),
            ICON_SIZE,
            theme.muted_foreground,
            2.0,
        );
        let text_x = query.origin.x + PAD + ICON_SIZE + 10.0;
        let placeholder = if self.finding() {
            "palette.findPlaceholder"
        } else {
            "palette.placeholder"
        };
        self.paint_field(
            cx,
            Rect {
                origin: Point2D::new(text_x, query.origin.y),
                size: Point2D::new(query.origin.x + query.size.x - PAD - text_x, INPUT_HEIGHT),
            },
            &self.palette.input,
            translate(self.ui, placeholder),
            self.palette.field == PaletteField::Query,
            INPUT_FONT,
            now_ms,
        );
        let divider = |cx: &mut PaintCx<'_>, y: f32| {
            cx.backend.fill_rect(
                Rect {
                    origin: Point2D::new(panel.origin.x, y),
                    size: Point2D::new(panel.size.x, 1.0),
                },
                theme.border,
            );
        };
        divider(cx, query.origin.y + INPUT_HEIGHT);

        if let (Some(row), Some(button)) = (self.replace_rect(panel), self.replace_all_rect(panel))
        {
            let field = Rect {
                origin: Point2D::new(text_x, row.origin.y),
                size: Point2D::new((button.origin.x - 12.0 - text_x).max(0.0), REPLACE_HEIGHT),
            };
            self.paint_field(
                cx,
                field,
                &self.palette.replace,
                translate(self.ui, "palette.replacePlaceholder"),
                self.palette.field == PaletteField::Replace,
                ROW_FONT,
                now_ms,
            );
            let hovered = self.palette.hover == Some(PaletteHover::ReplaceAll);
            if hovered {
                cx.backend.fill_round_rect(button, 7.0, theme.accent);
            }
            cx.backend.stroke_round_rect(button, 7.0, theme.border, 1.0);
            let label = self.replace_all_label();
            let width = text_metrics::measure_chrome(cx.backend, label, 12.0);
            paint_text(
                cx,
                label,
                Point2D::new(
                    button.origin.x + (button.size.x - width) / 2.0,
                    centered_text_baseline_y(button, 12.0),
                ),
                12.0,
                500,
                theme.foreground,
            );
            divider(cx, row.origin.y + REPLACE_HEIGHT);
        }

        let rows = self.rows_rect(panel);
        if self.palette.rows.is_empty() {
            let empty = if self.finding() && self.palette.input.text().trim().is_empty() {
                ""
            } else {
                translate(self.ui, "palette.noResults")
            };
            paint_text(
                cx,
                empty,
                Point2D::new(
                    rows.origin.x + PAD,
                    centered_text_baseline_y(rows, ROW_FONT),
                ),
                ROW_FONT,
                400,
                theme.muted_foreground,
            );
        } else {
            cx.backend.save();
            cx.backend.clip_rect(rows);
            let first = (self.scroll() / ROW_HEIGHT).floor().max(0.0) as usize;
            let visible = (rows.size.y / ROW_HEIGHT).ceil() as usize + 1;
            for index in first..(first + visible).min(self.palette.rows.len()) {
                self.paint_row(cx, self.row_rect(panel, index), index);
            }
            cx.backend.restore();
        }

        let footer = Rect {
            origin: Point2D::new(panel.origin.x, rows.origin.y + rows.size.y),
            size: Point2D::new(panel.size.x, FOOTER_HEIGHT),
        };
        divider(cx, footer.origin.y);
        let hint = translate(
            self.ui,
            if self.finding() {
                "palette.findHint"
            } else {
                "palette.hint"
            },
        );
        let hint_width = text_metrics::measure_chrome(cx.backend, hint, SMALL_FONT);
        let hint_x = footer.origin.x + footer.size.x - PAD - hint_width;
        paint_text(
            cx,
            hint,
            Point2D::new(hint_x, centered_text_baseline_y(footer, SMALL_FONT)),
            SMALL_FONT,
            400,
            theme.muted_foreground,
        );
        if !self.palette.summary.is_empty() {
            let area = Rect {
                origin: Point2D::new(footer.origin.x + PAD, footer.origin.y),
                size: Point2D::new(
                    (hint_x - 16.0 - footer.origin.x - PAD).max(0.0),
                    FOOTER_HEIGHT,
                ),
            };
            paint_clipped(
                cx,
                area,
                &self.palette.summary,
                SMALL_FONT,
                theme.muted_foreground,
            );
        }
    }
}

impl Widget for CommandPalettePanel<'_> {
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
        node.set_label(translate(
            self.ui,
            if self.finding() {
                "palette.cmd.findLayers"
            } else {
                "menu.commandPalette"
            },
        ));
        node
    }

    fn paint(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        self.paint_at(cx, panel, 0);
    }
}
