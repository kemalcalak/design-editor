//! "What's new" window (#251): the embedded release notes of each release
//! the user has not seen, one page per release (newest first), painted
//! over every other surface on the first launch after an update. State
//! lives in `EditorUiState::whats_new`; buttons map to `WhatsNewButton`s
//! that `EditorUiState::press_whats_new_button` applies.

use crate::theme::Theme;
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::home_surface::estimate_width;
use crate::widgets::line_break::wrap_lines;
use crate::widgets::panel_controls::{paint_accent_button, paint_neutral_button, ButtonSpec};
use crate::widgets::text_metrics::CHROME_FONT_FAMILY;
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::editor_ui_state::{WhatsNewButton, WhatsNewState};
use de_editor_core::release_notes::{note_for, ReleaseNote};
use de_editor_core::text_wrap::wrap_text_measured;
use de_editor_core::{EditorState, EditorUiState};

const CARD_W: f32 = 480.0;
const PAD: f32 = 28.0;
const RADIUS: f32 = 14.0;
const EYEBROW_SIZE: f32 = 12.0;
const TITLE_SIZE: f32 = 20.0;
const TITLE_LINE_H: f32 = 26.0;
const BODY_SIZE: f32 = 13.0;
const LINE_H: f32 = 19.0;
/// Space between bullets.
const ITEM_GAP: f32 = 6.0;
/// The bullet column the wrapped item text hangs from.
const BULLET_INDENT: f32 = 16.0;
const SECTION_GAP: f32 = 14.0;
const BUTTON_H: f32 = 32.0;
const BUTTON_RADIUS: f32 = 8.0;
/// Baselines sit this far above the bottom of their line box.
const TITLE_BASELINE_INSET: f32 = 6.0;
const BODY_BASELINE_INSET: f32 = 5.0;
const BUTTON_MIN_W: f32 = 88.0;
const BUTTON_PAD_X: f32 = 14.0;
const BUTTON_GAP: f32 = 8.0;
const VIEWPORT_MARGIN: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatsNewHit {
    Button(WhatsNewButton),
    /// Inside the card but on no button: swallowed.
    Inside,
    /// On the scrim: swallowed too, the window owns input while shown.
    Outside,
}

pub struct WhatsNewModal<'a> {
    id: WidgetId,
    theme: Theme,
    state: &'a WhatsNewState,
    note: &'static ReleaseNote,
    ui: &'a EditorUiState,
}

/// The text blocks of a page, wrapped to the card's inner width. Paint and
/// hit-test both walk this one stack, so they agree on the card height.
struct PageLines {
    title: Vec<String>,
    intro: Vec<String>,
    items: Vec<Vec<String>>,
}

impl<'a> WhatsNewModal<'a> {
    /// `None` unless the window is on screen and its release has notes.
    pub fn for_editor(state: &'a EditorState) -> Option<Self> {
        let whats_new = state.editor_ui.whats_new.as_ref()?;
        let note = note_for(whats_new.version(), state.editor_ui.effective_locale())?;
        Some(Self {
            id: WidgetId::new(5480),
            theme: theme_for(&state.editor_ui),
            state: whats_new,
            note,
            ui: &state.editor_ui,
        })
    }

    /// Centred card, as tall as the longest page so the buttons stay put
    /// while paging, and never taller than the viewport (the text is
    /// clipped above the buttons instead, which always stay reachable).
    pub fn rect(&self, viewport_w: f32, viewport_h: f32) -> Rect {
        let w = CARD_W.min((viewport_w - VIEWPORT_MARGIN * 2.0).max(0.0));
        let inner_w = w - PAD * 2.0;
        let locale = self.ui.effective_locale();
        let tallest = self
            .state
            .versions
            .iter()
            .filter_map(|version| note_for(version, locale))
            .map(|note| card_height(&lines_for(note, inner_w)))
            .fold(card_height(&self.lines(inner_w)), f32::max);
        let h = tallest.min((viewport_h - VIEWPORT_MARGIN * 2.0).max(0.0));
        Rect {
            origin: Point2D::new(
                ((viewport_w - w) / 2.0).max(VIEWPORT_MARGIN),
                ((viewport_h - h) / 2.0).max(VIEWPORT_MARGIN),
            ),
            size: Point2D::new(w, h),
        }
    }

    fn lines(&self, inner_w: f32) -> PageLines {
        lines_for(self.note, inner_w)
    }

    /// Button rects along the card's bottom edge: the paging pair on the
    /// left, "Got it" on the right.
    pub fn button_rects(&self, card: Rect) -> Vec<(WhatsNewButton, Rect)> {
        let y = card.origin.y + card.size.y - PAD - BUTTON_H;
        let width = |button| {
            (estimate_width(self.button_label(button), BODY_SIZE) + BUTTON_PAD_X * 2.0)
                .max(BUTTON_MIN_W)
        };
        let mut x = card.origin.x + PAD;
        self.state
            .buttons()
            .iter()
            .map(|&button| {
                let w = width(button);
                let rect = if button == WhatsNewButton::GotIt {
                    Rect::xywh(card.origin.x + card.size.x - PAD - w, y, w, BUTTON_H)
                } else {
                    let rect = Rect::xywh(x, y, w, BUTTON_H);
                    x += w + BUTTON_GAP;
                    rect
                };
                (button, rect)
            })
            .collect()
    }

    pub fn hit_test(&self, card: Rect, point: Point2D) -> WhatsNewHit {
        if !card.contains(point) {
            return WhatsNewHit::Outside;
        }
        self.button_rects(card)
            .into_iter()
            .find(|(_, rect)| rect.contains(point))
            .map_or(WhatsNewHit::Inside, |(button, _)| {
                WhatsNewHit::Button(button)
            })
    }

    fn t(&self, key: &'static str) -> &'static str {
        translate(self.ui, key)
    }

    fn version_label(&self) -> String {
        self.t("whatsNew.version")
            .replace("{{version}}", self.state.version())
    }

    fn button_label(&self, button: WhatsNewButton) -> &'static str {
        self.t(match button {
            WhatsNewButton::Previous => "whatsNew.previous",
            WhatsNewButton::Next => "whatsNew.next",
            WhatsNewButton::GotIt => "whatsNew.gotIt",
        })
    }

    fn paint_button(&self, cx: &mut PaintCx<'_>, button: WhatsNewButton, rect: Rect) {
        let theme = &self.theme;
        let enabled = self.state.enabled(button);
        let spec = ButtonSpec {
            rect,
            radius: BUTTON_RADIUS,
            label: self.button_label(button),
            label_size: BODY_SIZE,
            hovered: enabled && self.state.hover == Some(button),
            pressed: false,
        };
        if button == WhatsNewButton::GotIt {
            paint_accent_button(cx, theme, spec);
        } else if enabled {
            paint_neutral_button(cx, theme, spec);
        } else {
            // A paging button at the end of the list: the outline stays so
            // the pair keeps its place, the label fades.
            cx.backend
                .stroke_round_rect(rect, BUTTON_RADIUS, theme.border, 1.0);
            let label = spec.label;
            let label_x = rect.origin.x + (rect.size.x - estimate_width(label, BODY_SIZE)) / 2.0;
            let baseline = jian_widgets::centered_text_baseline_y(rect, BODY_SIZE);
            draw(
                cx,
                label,
                BODY_SIZE,
                theme.muted_foreground,
                label_x,
                baseline,
            );
        }
    }
}

fn lines_for(note: &ReleaseNote, inner_w: f32) -> PageLines {
    let item_w = inner_w - BULLET_INDENT;
    PageLines {
        title: wrap(&note.title, inner_w, TITLE_SIZE),
        intro: note
            .intro
            .as_deref()
            .map(|intro| wrap(intro, inner_w, BODY_SIZE))
            .unwrap_or_default(),
        items: note
            .items
            .iter()
            .map(|item| wrap(item, item_w, BODY_SIZE))
            .collect(),
    }
}

/// The full height a page's text asks for.
fn card_height(lines: &PageLines) -> f32 {
    let mut h = PAD + EYEBROW_SIZE + SECTION_GAP;
    h += lines.title.len() as f32 * TITLE_LINE_H;
    if !lines.intro.is_empty() {
        h += SECTION_GAP / 2.0 + lines.intro.len() as f32 * LINE_H;
    }
    h += SECTION_GAP;
    let item_lines: usize = lines.items.iter().map(Vec::len).sum();
    h += item_lines as f32 * LINE_H;
    h += ITEM_GAP * lines.items.len().saturating_sub(1) as f32;
    h + SECTION_GAP * 2.0 + BUTTON_H + PAD
}

/// Wrap against the layout-time width estimate, so hit-test (no backend)
/// and paint agree on the card height. The canvas wrap breaks Latin at
/// spaces and CJK between characters; a line it leaves too wide (Thai
/// writes phrases without spaces) is broken again between clusters.
fn wrap(text: &str, max_w: f32, size: f32) -> Vec<String> {
    let mut measure = |s: &str| estimate_width(s, size);
    wrap_text_measured(text, max_w, &mut measure)
        .into_iter()
        .flat_map(|line| wrap_lines(&line, max_w, |s| estimate_width(s, size)))
        .collect()
}

fn draw(cx: &mut PaintCx<'_>, text: &str, size: f32, color: Color, x: f32, baseline: f32) {
    let layout = TextLayout::single_run(
        text,
        CHROME_FONT_FAMILY,
        size,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(&layout, Point2D::new(x, baseline));
}

impl Widget for WhatsNewModal<'_> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect::xywh(
                0.0,
                0.0,
                CARD_W,
                card_height(&self.lines(CARD_W - PAD * 2.0)),
            ),
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let theme = &self.theme;
        cx.backend.fill_round_rect(rect, RADIUS, theme.card);
        cx.backend
            .stroke_round_rect(rect, RADIUS, theme.border, 1.0);

        let x = rect.origin.x + PAD;
        let inner_w = rect.size.x - PAD * 2.0;
        let lines = self.lines(inner_w);
        let buttons = self.button_rects(rect);
        // A card capped by a short viewport clips its text above the buttons.
        let text_bottom = rect.origin.y + rect.size.y - PAD - BUTTON_H - SECTION_GAP;
        cx.backend.save();
        cx.backend.clip_rect(Rect::xywh(
            rect.origin.x,
            rect.origin.y,
            rect.size.x,
            (text_bottom - rect.origin.y).max(0.0),
        ));

        // Eyebrow: "What's new" on the left, the release on the right.
        let mut y = rect.origin.y + PAD + EYEBROW_SIZE;
        draw(
            cx,
            self.t("whatsNew.heading"),
            EYEBROW_SIZE,
            theme.primary,
            x,
            y,
        );
        let version = self.version_label();
        let version_x = x + inner_w - estimate_width(&version, EYEBROW_SIZE);
        draw(
            cx,
            &version,
            EYEBROW_SIZE,
            theme.muted_foreground,
            version_x,
            y,
        );
        y += SECTION_GAP;

        for line in &lines.title {
            draw(
                cx,
                line,
                TITLE_SIZE,
                theme.foreground,
                x,
                y + TITLE_LINE_H - TITLE_BASELINE_INSET,
            );
            y += TITLE_LINE_H;
        }
        if !lines.intro.is_empty() {
            y += SECTION_GAP / 2.0;
            for line in &lines.intro {
                draw(
                    cx,
                    line,
                    BODY_SIZE,
                    theme.muted_foreground,
                    x,
                    y + LINE_H - BODY_BASELINE_INSET,
                );
                y += LINE_H;
            }
        }
        y += SECTION_GAP;
        for (index, item) in lines.items.iter().enumerate() {
            if index > 0 {
                y += ITEM_GAP;
            }
            let dot_y = y + LINE_H / 2.0 - 2.0;
            cx.backend
                .fill_round_rect(Rect::xywh(x + 2.0, dot_y, 4.0, 4.0), 2.0, theme.primary);
            for line in item {
                let baseline = y + LINE_H - BODY_BASELINE_INSET;
                draw(
                    cx,
                    line,
                    BODY_SIZE,
                    theme.foreground,
                    x + BULLET_INDENT,
                    baseline,
                );
                y += LINE_H;
            }
        }

        cx.backend.restore();

        // "2 / 3" after the paging pair (only shown with several releases).
        if let Some((_, next)) = buttons.iter().find(|(b, _)| *b == WhatsNewButton::Next) {
            let page = format!("{} / {}", self.state.page + 1, self.state.versions.len());
            let baseline = jian_widgets::centered_text_baseline_y(*next, BODY_SIZE);
            let page_x = next.origin.x + next.size.x + BUTTON_GAP + 4.0;
            draw(
                cx,
                &page,
                BODY_SIZE,
                theme.muted_foreground,
                page_x,
                baseline,
            );
        }
        for (button, button_rect) in buttons {
            self.paint_button(cx, button, button_rect);
        }
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Dialog);
        node.set_label(format!(
            "{} · {} · {}",
            self.t("whatsNew.heading"),
            self.version_label(),
            self.note.title
        ));
        node
    }
}

#[cfg(test)]
#[path = "whats_new_modal_tests.rs"]
mod tests;
