//! Body painter for [`super::ai_chat_panel::AIChatPlaceholder`] — the
//! panel surface and the empty state. The active-transcript painter
//! lives in `ai_chat_transcript.rs`. Split out of `ai_chat_panel.rs`
//! to keep that file under the 800-line cap.

use super::ai_chat_panel::{ExampleCard, HEADER_HEIGHT, PAD};
use crate::theme::Theme;
use crate::widgets::ai_chat_chrome::{chat_label, fit_label, label_baseline, outline_control};
use crate::widgets::home_surface::HomePalette;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};

/// Gap between stacked prompt rows.
pub(crate) const EXAMPLE_CARD_GAP: f32 = 8.0;
/// Height of each prompt row.
pub(crate) const EXAMPLE_CARD_HEIGHT: f32 = 36.0;
const PROMPT_RADIUS: f32 = 8.0;
const PROMPT_FONT: f32 = 13.0;
const TITLE_FONT: f32 = 15.0;
/// Space between the session row and the title.
const EMPTY_TOP: f32 = 24.0;
/// Title row height plus the gap under it.
const TITLE_BLOCK: f32 = 38.0;
const TIPS_TOP_GAP: f32 = 28.0;
const TIP_FONT: f32 = 13.0;
const TIP_LINE_H: f32 = 19.0;
const TIP_GAP: f32 = 16.0;

/// Whether a prompt row fully fits above `content_bottom` — the shared
/// predicate paint and hit-test use so a row that would run into the
/// bottom-anchored composer is dropped from BOTH.
pub(crate) fn example_card_fits(card: &Rect, content_bottom: f32) -> bool {
    card.origin.y + card.size.y <= content_bottom
}

/// The four prompt rows, stacked full-width under the title.
pub(crate) fn example_card_rects(rect: Rect) -> [Rect; 4] {
    let first = rect.origin.y + HEADER_HEIGHT + EMPTY_TOP + TITLE_BLOCK;
    let w = rect.size.x - PAD * 2.0;
    std::array::from_fn(|i| {
        Rect::xywh(
            rect.origin.x + PAD,
            first + i as f32 * (EXAMPLE_CARD_HEIGHT + EXAMPLE_CARD_GAP),
            w,
            EXAMPLE_CARD_HEIGHT,
        )
    })
}

/// Paint the floating chat panel shell. TS uses
/// `rounded-xl border bg-card/95 shadow-lg backdrop-blur-sm`; Skia here
/// has no blur primitive, so we fake one with two translucent rounded
/// rects behind the card. The near layer carries most of the (light)
/// shadow right at the panel edge; the far layer is very faint so the
/// shadow fades out instead of ending in a hard full-width grey band.
pub(crate) fn paint_panel_surface(cx: &mut PaintCx<'_>, theme: &Theme, rect: Rect) {
    let shadow_far = Rect {
        origin: Point2D::new(rect.origin.x, rect.origin.y + 8.0),
        size: rect.size,
    };
    let shadow_near = Rect {
        origin: Point2D::new(rect.origin.x, rect.origin.y + 3.0),
        size: rect.size,
    };
    cx.backend
        .fill_round_rect(shadow_far, 14.0, (Color::BLACK).with_alpha(0.025));
    cx.backend
        .fill_round_rect(shadow_near, 14.0, (Color::BLACK).with_alpha(0.07));
    cx.backend
        .fill_round_rect(rect, 14.0, (theme.card).with_alpha(0.98));
    cx.backend
        .stroke_round_rect(rect, 14.0, (theme.border).with_alpha(0.82), 1.0);
}

/// The empty state (#26): a title, four prompt rows and three tips.
/// `content_bottom` is where the composer block begins; rows and tips
/// that would run into it are dropped rather than painted over it.
pub(crate) struct EmptyState<'a> {
    pub(crate) title: &'a str,
    pub(crate) examples: &'a [ExampleCard; 4],
    /// `(heading, body)` per tip.
    pub(crate) tips: [(&'a str, String); 3],
    pub(crate) disabled: bool,
    pub(crate) hover: Option<usize>,
    pub(crate) pressed: Option<usize>,
    pub(crate) content_bottom: f32,
}

pub(crate) fn paint_empty_state(
    cx: &mut PaintCx<'_>,
    palette: &HomePalette,
    rect: Rect,
    empty: &EmptyState<'_>,
) {
    let width = (rect.size.x - PAD * 2.0).max(0.0);
    let left = rect.origin.x + PAD;
    let title_top = rect.origin.y + HEADER_HEIGHT + EMPTY_TOP;
    let title = fit_label(cx, empty.title, TITLE_FONT, width);
    chat_label(
        cx,
        &title,
        TITLE_FONT,
        palette.text,
        600,
        Point2D::new(
            left,
            label_baseline(Rect::xywh(left, title_top, width, 20.0), TITLE_FONT),
        ),
    );

    let rows = example_card_rects(rect);
    for (idx, (row, ex)) in rows.iter().zip(empty.examples.iter()).enumerate() {
        if !example_card_fits(row, empty.content_bottom) {
            return;
        }
        let active = !empty.disabled && (empty.hover == Some(idx) || empty.pressed == Some(idx));
        outline_control(cx, palette, *row, PROMPT_RADIUS, active);
        let label = fit_label(cx, &ex.title, PROMPT_FONT, (row.size.x - 24.0).max(0.0));
        chat_label(
            cx,
            &label,
            PROMPT_FONT,
            if active { palette.text } else { palette.text_2 },
            400,
            Point2D::new(row.origin.x + 12.0, label_baseline(*row, PROMPT_FONT)),
        );
    }

    // Tips: a heading and a wrapped body each, dropped once they no
    // longer fit above the composer.
    let mut y = rows[3].origin.y + rows[3].size.y + TIPS_TOP_GAP;
    for (heading, body) in &empty.tips {
        let lines = wrap_words(cx, body, TIP_FONT, width);
        let tip_h = TIP_LINE_H + 4.0 + lines.len() as f32 * TIP_LINE_H;
        if y + tip_h > empty.content_bottom {
            return;
        }
        chat_label(
            cx,
            heading,
            TIP_FONT,
            palette.text,
            600,
            Point2D::new(
                left,
                label_baseline(Rect::xywh(left, y, width, TIP_LINE_H), TIP_FONT),
            ),
        );
        let mut line_y = y + TIP_LINE_H + 4.0;
        for line in &lines {
            chat_label(
                cx,
                line,
                TIP_FONT,
                palette.text_3,
                400,
                Point2D::new(
                    left,
                    label_baseline(Rect::xywh(left, line_y, width, TIP_LINE_H), TIP_FONT),
                ),
            );
            line_y += TIP_LINE_H;
        }
        y += tip_h + TIP_GAP;
    }
}

/// Greedy word wrap of `text` to `max_w` at `size`.
fn wrap_words(cx: &mut PaintCx<'_>, text: &str, size: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && text_metrics::measure_chrome(cx.backend, &candidate, size) > max_w {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RenderBackend, TextLayout};

    #[derive(Default)]
    struct CaptureBackend {
        text_runs: Vec<String>,
        round_rects: Vec<Rect>,
    }

    impl RenderBackend for CaptureBackend {
        fn begin_frame(&mut self) {}
        fn end_frame(&mut self) {}
        fn fill_rect(&mut self, _: Rect, _: Color) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn draw_text(&mut self, layout: &TextLayout, _: Point2D) {
            self.text_runs
                .extend(layout.runs().iter().map(|run| run.content.clone()));
        }
        fn clip_rect(&mut self, _: Rect) {}
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _: Point2D) {}
        fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
        fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
        fn stroke_round_rect(&mut self, r: Rect, _: f32, _: Color, _: f32) {
            self.round_rects.push(r);
        }
        fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
        fn resize(&mut self, _: u32, _: u32) {}
        fn dpi_scale(&self) -> f32 {
            1.0
        }
        fn measure_text(&mut self, text: &str, size: f32) -> f32 {
            text.chars().count() as f32 * size * 0.55
        }
    }

    fn examples() -> [ExampleCard; 4] {
        ["Pricing page", "Onboarding", "Dashboard", "Deck cover"].map(|t| ExampleCard {
            title: t.into(),
            subtitle: String::new(),
            prompt: t.into(),
            emoji: "",
        })
    }

    fn paint(content_bottom: f32) -> CaptureBackend {
        let mut backend = CaptureBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        let examples = examples();
        let tip = |h: &'static str| {
            (
                h,
                "A body long enough to wrap onto a second line in a rail.".to_string(),
            )
        };
        paint_empty_state(
            &mut cx,
            &HomePalette::dark(),
            Rect::xywh(0.0, 0.0, 320.0, 800.0),
            &EmptyState {
                title: "Ask me to design anything",
                examples: &examples,
                tips: [tip("Tip one"), tip("Tip two"), tip("Tip three")],
                disabled: false,
                hover: None,
                pressed: None,
                content_bottom,
            },
        );
        backend
    }

    #[test]
    fn prompt_rows_stack_full_width_under_the_title() {
        let rows = example_card_rects(Rect::xywh(0.0, 0.0, 320.0, 800.0));
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.origin.x, PAD);
            assert_eq!(row.size.x, 320.0 - PAD * 2.0);
            assert_eq!(row.size.y, EXAMPLE_CARD_HEIGHT);
            if i > 0 {
                let prev = rows[i - 1];
                assert_eq!(
                    row.origin.y,
                    prev.origin.y + EXAMPLE_CARD_HEIGHT + EXAMPLE_CARD_GAP
                );
            }
        }
        assert!(rows[0].origin.y > HEADER_HEIGHT + EMPTY_TOP);
    }

    #[test]
    fn a_roomy_panel_paints_title_prompts_and_every_tip() {
        let b = paint(800.0);
        assert!(b.text_runs.iter().any(|r| r == "Ask me to design anything"));
        assert_eq!(b.round_rects.len(), 4, "one outline per prompt row");
        for heading in ["Tip one", "Tip two", "Tip three"] {
            assert!(b.text_runs.iter().any(|r| r == heading));
        }
    }

    #[test]
    fn rows_and_tips_that_would_meet_the_composer_are_dropped() {
        let rows = example_card_rects(Rect::xywh(0.0, 0.0, 320.0, 800.0));
        let b = paint(rows[1].origin.y + rows[1].size.y + 1.0);
        assert_eq!(b.round_rects.len(), 2);
        assert!(!b.text_runs.iter().any(|r| r.starts_with("Tip")));
        let b = paint(rows[3].origin.y + rows[3].size.y + TIPS_TOP_GAP + 30.0);
        assert_eq!(b.round_rects.len(), 4);
        assert!(!b.text_runs.iter().any(|r| r.starts_with("Tip")));
    }
}
