//! Paint-assertion tests for [`super::AIChatPlaceholder`] — drive the
//! widget against a recording [`PanelPaintBackend`] and assert on the
//! emitted draw ops. Sibling of `tests.rs`, split to keep both files
//! under the 800-line cap.

use super::tests::{has_fill_rect, seed_available_model};
use super::*;

pub(in super::super) fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "expected {actual} to be close to {expected}"
    );
}

pub(in super::super) fn rect_close(actual: Rect, expected: Rect) -> bool {
    (actual.origin.x - expected.origin.x).abs() < 0.01
        && (actual.origin.y - expected.origin.y).abs() < 0.01
        && (actual.size.x - expected.size.x).abs() < 0.01
        && (actual.size.y - expected.size.y).abs() < 0.01
}

pub(in super::super) fn color_close(actual: crate::Color, expected: crate::Color) -> bool {
    (actual.r - expected.r).abs() < 0.001
        && (actual.g - expected.g).abs() < 0.001
        && (actual.b - expected.b).abs() < 0.001
        && (actual.a - expected.a).abs() < 0.001
}

#[test]
fn paint_header_uses_auto_generated_chat_title() {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives:
    // the rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    s.chat.title = "Pricing page".into();
    let panel = AIChatPlaceholder::from_editor(&s);
    let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
    let mut backend = PanelPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    let select = panel.header(rect).select;
    assert!(
        backend
            .texts
            .iter()
            .any(|(text, _, _, at)| text == "Pricing page"
                && at.x > select.origin.x
                && at.x < select.origin.x + select.size.x),
        "the session select shows the chat's title"
    );
}

#[test]
fn paint_quick_action_card_hover_adds_visible_feedback() {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives:
    // the rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    seed_available_model(&mut s);
    s.editor_ui.chat_example_hover = Some(0);
    let panel = AIChatPlaceholder::from_editor(&s);
    let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
    let cards = crate::widgets::ai_chat_panel_paint::example_card_rects(rect);
    let mut backend = PanelPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .round_rects
            .iter()
            .any(|(r, _, color)| rect_close(*r, cards[0])
                && color_close(*color, panel.palette.subtle)),
        "the hover prompt row should paint the subtle wash"
    );
}

#[test]
fn paint_quick_action_card_pressed_uses_shared_feedback() {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives:
    // the rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    seed_available_model(&mut s);
    s.editor_ui.pressed_button = Some(de_editor_core::ButtonPressTarget::ChatExample(0));
    let panel = AIChatPlaceholder::from_editor(&s);
    let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
    let cards = crate::widgets::ai_chat_panel_paint::example_card_rects(rect);
    let mut backend = PanelPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend
            .round_rects
            .iter()
            .any(|(r, _, color)| rect_close(*r, cards[0])
                && color_close(*color, panel.palette.subtle)),
        "the pressed prompt row should paint the subtle wash"
    );
}

#[test]
fn paint_send_button_hover_adds_visible_feedback() {
    // The active send circle dims on hover (rest 1.0 → hover 0.9 alpha), so the
    // hovered fill must visibly differ from the resting fill — not just exist.
    let send_fill = |hovered: bool| -> crate::Color {
        let mut s = EditorState::new();
        // The expanded panel only exists where the conversation lives:
        // the rail's Agent tab. Elsewhere the chat is composer-only.
        s.editor_ui.enter_chat_tab();
        seed_available_model(&mut s);
        s.chat.set_input_text("design a login page");
        if hovered {
            s.editor_ui.chat_footer_hover = Some(de_editor_core::ChatFooterButton::Send);
        }
        let panel = AIChatPlaceholder::from_editor(&s);
        let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
        let mut backend = PanelPaintBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(&mut cx, rect);
        // Use footer_layout to get the exact rect rather than hardcoding.
        let footer = panel.footer_layout(rect);
        backend
            .round_rects
            .iter()
            .filter(|(r, _, _)| rect_close(*r, footer.send))
            .map(|(_, _, c)| *c)
            .next_back()
            .expect("send circle must paint a fill")
    };

    let resting = send_fill(false);
    let hovered = send_fill(true);
    assert!(
        !color_close(resting, hovered),
        "hovered send fill must visibly differ from the resting fill"
    );
}

#[test]
fn from_editor_picks_up_chat_button_press_targets() {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives:
    // the rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    s.editor_ui.pressed_button = Some(de_editor_core::ButtonPressTarget::ChatHeader(
        de_editor_core::ChatHeaderButton::NewChat,
    ));
    let header_panel = AIChatPlaceholder::from_editor(&s);
    assert_eq!(
        header_panel.header_pressed,
        Some(de_editor_core::ChatHeaderButton::NewChat)
    );
    assert_eq!(header_panel.footer_pressed, None);

    s.editor_ui.pressed_button = Some(de_editor_core::ButtonPressTarget::ChatFooter(
        de_editor_core::ChatFooterButton::Send,
    ));
    let footer_panel = AIChatPlaceholder::from_editor(&s);
    assert_eq!(footer_panel.header_pressed, None);
    assert_eq!(
        footer_panel.footer_pressed,
        Some(de_editor_core::ChatFooterButton::Send)
    );
}

#[test]
fn paint_footer_hovers_use_the_subtle_wash() {
    for hover in [
        de_editor_core::ChatFooterButton::PlusMenu,
        de_editor_core::ChatFooterButton::ModelPicker,
    ] {
        let mut s = EditorState::new();
        // The expanded panel only exists where the conversation lives:
        // the rail's Agent tab. Elsewhere the chat is composer-only.
        s.editor_ui.enter_chat_tab();
        seed_available_model(&mut s);
        s.editor_ui.chat_footer_hover = Some(hover);
        let mut panel = AIChatPlaceholder::from_editor(&s);
        panel.usable_agent = true;
        let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
        let footer = panel.footer_layout(rect);
        let target = match hover {
            de_editor_core::ChatFooterButton::PlusMenu => footer.plus,
            _ => footer.model,
        };
        let mut backend = PanelPaintBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(&mut cx, rect);
        assert!(
            backend
                .round_rects
                .iter()
                .any(|(r, _, color)| rect_close(*r, target)
                    && color_close(*color, panel.palette.subtle)),
            "{hover:?} hover should paint the subtle wash on its own control"
        );
    }
}

#[test]
fn paint_session_row_shows_the_chat_title_and_new() {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives:
    // the rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    let panel = AIChatPlaceholder::from_editor(&s);
    let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
    let mut backend = PanelPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    panel.paint(&mut cx, rect);
    let in_header = |y: f32| y > 0.0 && y < HEADER_HEIGHT;
    assert!(backend
        .texts
        .iter()
        .any(|(t, _, _, at)| t == &s.chat.title && in_header(at.y)));
    assert!(backend
        .texts
        .iter()
        .any(|(t, _, _, at)| t == &panel.label_new && in_header(at.y)));
}

#[test]
fn model_chip_shows_model_and_provider_or_the_provider_link() {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives:
    // the rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    seed_available_model(&mut s);
    let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
    let paint = |panel: &AIChatPlaceholder<'_>| {
        let mut backend = PanelPaintBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(&mut cx, rect);
        backend.texts
    };
    // No agent can answer: the chip is Home's "Add a model provider" link.
    let panel = AIChatPlaceholder::from_editor(&s);
    assert!(paint(&panel)
        .iter()
        .any(|(t, ..)| t == &panel.label_no_models));
    // With one, it reads "model · provider", inside the chip.
    let mut panel = AIChatPlaceholder::from_editor(&s);
    panel.usable_agent = true;
    panel.model_chip_label = "GPT-5 · Codex CLI".to_string();
    let footer = panel.footer_layout(rect);
    let texts = paint(&panel);
    let (_, _, _, at) = texts
        .iter()
        .find(|(t, ..)| t.starts_with("GPT-5"))
        .expect("chip label painted");
    assert!(at.x >= footer.model.origin.x && at.x < footer.model.origin.x + footer.model.size.x);
}

#[derive(Default)]
struct PanelPaintBackend {
    fills: Vec<(Rect, crate::Color)>,
    round_rects: Vec<(Rect, f32, crate::Color)>,
    texts: Vec<(String, f32, jian_core::scene::Color, Point2D)>,
    svg_strokes: Vec<(Point2D, f32, crate::Color, f32)>,
    svg_paths: Vec<String>,
    stroke_lines: usize,
}

impl crate::RenderBackend for PanelPaintBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, rect: Rect, color: crate::Color) {
        self.fills.push((rect, color));
    }
    fn stroke_rect(&mut self, _: Rect, _: crate::Color, _: f32) {}
    fn draw_text(&mut self, layout: &crate::TextLayout, origin: Point2D) {
        if let Some(run) = layout.runs().first() {
            self.texts
                .push((run.content.clone(), run.font_size, run.color, origin));
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: crate::Color, _: f32) {
        self.stroke_lines += 1;
    }
    fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: crate::Color) {
        self.round_rects.push((rect, radius, color));
    }
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: crate::Color, _: f32) {}
    fn stroke_svg_path(
        &mut self,
        d: &str,
        top_left: Point2D,
        size: f32,
        color: crate::Color,
        width: f32,
    ) {
        self.svg_paths.push(d.to_string());
        self.svg_strokes.push((top_left, size, color, width));
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

#[test]
fn paint_draws_the_session_row_rule_and_the_composer_card() {
    let mut s = EditorState::new();
    // The expanded panel only exists where the conversation lives:
    // the rail's Agent tab. Elsewhere the chat is composer-only.
    s.editor_ui.enter_chat_tab();
    let panel = AIChatPlaceholder::from_editor(&s);
    let rect = Rect::xywh(10.0, 20.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
    let mut backend = PanelPaintBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    // Pinned into the rail, the panel is the page background with one rule
    // under the session row (#26), and the composer is a card, not a rule.
    assert!(has_fill_rect(&backend.fills, rect), "rail background");
    assert!(
        has_fill_rect(
            &backend.fills,
            Rect::xywh(
                rect.origin.x,
                rect.origin.y + HEADER_HEIGHT - 1.0,
                rect.size.x,
                1.0
            )
        ),
        "the session row's hairline"
    );
    let card = panel.composer_card(rect);
    assert!(backend
        .round_rects
        .iter()
        .any(|(r, _, c)| rect_close(*r, card) && color_close(*c, panel.palette.surface)));
}
