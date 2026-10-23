//! In-app update modal (#111): offer → download (progress bar, percent,
//! size, speed) → verify → install → restart, painted over every other
//! surface. State lives in `EditorUiState::update_modal`; buttons map to
//! `UpdateModalButton`s that `EditorUiState::press_update_button` applies.

use crate::theme::Theme;
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::panel_controls::{paint_accent_button, paint_neutral_button, ButtonSpec};
use crate::widgets::text_metrics::{fit_chrome, measure_chrome, CHROME_FONT_FAMILY};
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::editor_ui_state::{
    UpdateFailure, UpdateModalButton, UpdateModalState, UpdatePhase,
};
use de_editor_core::{EditorState, EditorUiState};

const CARD_W: f32 = 440.0;
const CARD_H: f32 = 196.0;
const PAD: f32 = 24.0;
const RADIUS: f32 = 14.0;
const TITLE_SIZE: f32 = 16.0;
const BODY_SIZE: f32 = 13.0;
const STATS_SIZE: f32 = 12.0;
const LINE_H: f32 = 19.0;
const BAR_H: f32 = 6.0;
const BUTTON_H: f32 = 32.0;
const BUTTON_W: f32 = 112.0;
const BUTTON_GAP: f32 = 8.0;
const VIEWPORT_MARGIN: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateModalHit {
    Button(UpdateModalButton),
    /// Inside the card but on no button — swallowed.
    Inside,
    /// On the scrim — swallowed too: the modal owns input while shown.
    Outside,
}

pub struct UpdateModal<'a> {
    id: WidgetId,
    theme: Theme,
    modal: &'a UpdateModalState,
    ui: &'a EditorUiState,
}

impl<'a> UpdateModal<'a> {
    /// `None` unless an update modal is on screen.
    pub fn for_editor(state: &'a EditorState) -> Option<Self> {
        let modal = state
            .editor_ui
            .update_modal
            .as_ref()
            .filter(|m| m.visible)?;
        Some(Self {
            id: WidgetId::new(5470),
            theme: theme_for(&state.editor_ui),
            modal,
            ui: &state.editor_ui,
        })
    }

    pub fn rect(&self, viewport_w: f32, viewport_h: f32) -> Rect {
        let w = CARD_W.min((viewport_w - VIEWPORT_MARGIN * 2.0).max(0.0));
        Rect {
            origin: Point2D::new(
                ((viewport_w - w) / 2.0).max(VIEWPORT_MARGIN),
                ((viewport_h - CARD_H) / 2.0).max(VIEWPORT_MARGIN),
            ),
            size: Point2D::new(w, CARD_H),
        }
    }

    /// Button rects, right-aligned along the card's bottom edge, in the
    /// phase's paint order.
    pub fn button_rects(&self, card: Rect) -> Vec<(UpdateModalButton, Rect)> {
        let buttons = self.modal.buttons();
        let y = card.origin.y + card.size.y - PAD - BUTTON_H;
        let mut x = card.origin.x + card.size.x - PAD;
        let mut out: Vec<_> = buttons
            .iter()
            .rev()
            .map(|button| {
                x -= BUTTON_W;
                let rect = Rect::xywh(x, y, BUTTON_W, BUTTON_H);
                x -= BUTTON_GAP;
                (*button, rect)
            })
            .collect();
        out.reverse();
        out
    }

    pub fn hit_test(&self, card: Rect, point: Point2D) -> UpdateModalHit {
        if !card.contains(point) {
            return UpdateModalHit::Outside;
        }
        self.button_rects(card)
            .into_iter()
            .find(|(_, rect)| rect.contains(point))
            .map_or(UpdateModalHit::Inside, |(button, _)| {
                UpdateModalHit::Button(button)
            })
    }

    fn t(&self, key: &'static str) -> &'static str {
        translate(self.ui, key)
    }

    fn body_text(&self) -> String {
        let key = match self.modal.phase {
            UpdatePhase::Offer => {
                return self
                    .t("updateModal.offerBody")
                    .replace("{{current}}", env!("CARGO_PKG_VERSION"))
            }
            UpdatePhase::Downloading { .. } => "updateModal.downloading",
            UpdatePhase::Verifying => "updateModal.verifying",
            UpdatePhase::Installing => "updateModal.installing",
            UpdatePhase::ReadyToRestart => "updateModal.ready",
            UpdatePhase::Failed(failure) => failure_key(failure),
        };
        self.t(key).to_string()
    }

    fn button_label(&self, button: UpdateModalButton) -> &'static str {
        self.t(match button {
            UpdateModalButton::Later => "updateModal.later",
            UpdateModalButton::Update => "updateModal.update",
            UpdateModalButton::Cancel => "updateModal.cancel",
            UpdateModalButton::Restart => "updateModal.restart",
            UpdateModalButton::Close => "updateModal.close",
            UpdateModalButton::Retry => "updateModal.retry",
            UpdateModalButton::Releases => "updateModal.releases",
        })
    }
}

fn failure_key(failure: UpdateFailure) -> &'static str {
    match failure {
        UpdateFailure::Network => "updateModal.failedNetwork",
        UpdateFailure::Checksum => "updateModal.failedChecksum",
        UpdateFailure::Install => "updateModal.failedInstall",
        UpdateFailure::Unsupported => "updateModal.failedUnsupported",
    }
}

/// `18.3 MB` — decimal megabytes, one decimal, like the OS download UIs.
pub fn format_megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

/// The stats line under the bar: `42% · 18.3 / 43.7 MB · 4.2 MB/s`. Without
/// a content length only the received size and the speed are known.
pub fn download_stats(received: u64, total: Option<u64>, bytes_per_sec: f64) -> String {
    let speed = format!("{:.1} MB/s", bytes_per_sec.max(0.0) / 1_000_000.0);
    match total.filter(|t| *t > 0) {
        Some(total) => {
            let percent = (received.min(total) as f64 * 100.0 / total as f64).floor() as u32;
            let received_mb = format!("{:.1}", received as f64 / 1_000_000.0);
            format!(
                "{percent}% · {received_mb} / {} · {speed}",
                format_megabytes(total)
            )
        }
        None => format!("{} · {speed}", format_megabytes(received)),
    }
}

/// Greedy word wrap against the chrome font.
fn wrap_lines(cx: &mut PaintCx<'_>, text: &str, max_w: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && measure_chrome(cx.backend, &candidate, size) > max_w {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
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

impl Widget for UpdateModal<'_> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect::xywh(0.0, 0.0, CARD_W, CARD_H),
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let theme = &self.theme;
        cx.backend.fill_round_rect(rect, RADIUS, theme.card);
        cx.backend
            .stroke_round_rect(rect, RADIUS, theme.border, 1.0);

        let x = rect.origin.x + PAD;
        let inner_w = rect.size.x - PAD * 2.0;
        let title = self
            .t("updateModal.title")
            .replace("{{version}}", &format!("v{}", self.modal.version));
        let title = fit_chrome(cx.backend, &title, inner_w, TITLE_SIZE);
        draw(
            cx,
            &title,
            TITLE_SIZE,
            theme.foreground,
            x,
            rect.origin.y + PAD + TITLE_SIZE,
        );

        let mut y = rect.origin.y + PAD + TITLE_SIZE + 26.0;
        let body_color = match self.modal.phase {
            UpdatePhase::Failed(_) => theme.destructive,
            _ => theme.muted_foreground,
        };
        for line in wrap_lines(cx, &self.body_text(), inner_w, BODY_SIZE)
            .iter()
            .take(3)
        {
            draw(cx, line, BODY_SIZE, body_color, x, y);
            y += LINE_H;
        }

        if let Some(progress) = self.modal.progress() {
            let bar_y = y - 2.0;
            let track = Rect::xywh(x, bar_y, inner_w, BAR_H);
            cx.backend.fill_round_rect(track, BAR_H / 2.0, theme.muted);
            if progress > 0.0 {
                let fill = Rect::xywh(x, bar_y, (inner_w * progress).max(BAR_H), BAR_H);
                cx.backend.fill_round_rect(fill, BAR_H / 2.0, theme.primary);
            }
        }
        if let UpdatePhase::Downloading {
            received,
            total,
            bytes_per_sec,
        } = self.modal.phase
        {
            draw(
                cx,
                &download_stats(received, total, bytes_per_sec),
                STATS_SIZE,
                theme.muted_foreground,
                x,
                y + BAR_H + 16.0,
            );
        }

        let rects = self.button_rects(rect);
        let primary = rects.last().map(|(button, _)| *button);
        for (button, button_rect) in rects {
            let spec = ButtonSpec {
                rect: button_rect,
                radius: 8.0,
                label: self.button_label(button),
                label_size: BODY_SIZE,
                hovered: self.modal.hover == Some(button),
                pressed: false,
            };
            if Some(button) == primary && button != UpdateModalButton::Cancel {
                paint_accent_button(cx, theme, spec);
            } else {
                paint_neutral_button(cx, theme, spec);
            }
        }
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Dialog);
        node.set_label(self.t("updateModal.title"));
        if matches!(
            self.modal.phase,
            UpdatePhase::Downloading { .. } | UpdatePhase::Verifying | UpdatePhase::Installing
        ) {
            node.set_busy();
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(phase: UpdatePhase) -> EditorState {
        let mut state = EditorState::default();
        state.editor_ui.update_modal = Some(UpdateModalState::new("0.2.1", phase));
        state
    }

    #[test]
    fn stats_line_reads_percent_size_and_speed() {
        assert_eq!(
            download_stats(18_300_000, Some(43_700_000), 4_200_000.0),
            "41% · 18.3 / 43.7 MB · 4.2 MB/s"
        );
        assert_eq!(
            download_stats(5_000_000, None, 1_000_000.0),
            "5.0 MB · 1.0 MB/s"
        );
        // A server that over-delivers never shows more than 100%.
        assert!(download_stats(200, Some(100), 0.0).starts_with("100%"));
    }

    #[test]
    fn hidden_modal_is_not_painted() {
        let mut state = state_with(UpdatePhase::ReadyToRestart);
        assert!(UpdateModal::for_editor(&state).is_some());
        state.editor_ui.update_modal.as_mut().unwrap().visible = false;
        assert!(UpdateModal::for_editor(&state).is_none());
    }

    #[test]
    fn buttons_are_right_aligned_and_hit_testable() {
        let state = state_with(UpdatePhase::Offer);
        let modal = UpdateModal::for_editor(&state).unwrap();
        let card = modal.rect(1200.0, 800.0);
        let rects = modal.button_rects(card);
        assert_eq!(rects[0].0, UpdateModalButton::Later);
        assert_eq!(rects[1].0, UpdateModalButton::Update);
        let update = rects[1].1;
        assert!(
            (update.origin.x + update.size.x - (card.origin.x + card.size.x - PAD)).abs() < 0.01
        );
        let centre = Point2D::new(
            update.origin.x + update.size.x / 2.0,
            update.origin.y + update.size.y / 2.0,
        );
        assert_eq!(
            modal.hit_test(card, centre),
            UpdateModalHit::Button(UpdateModalButton::Update)
        );
        assert_eq!(
            modal.hit_test(card, Point2D::new(card.origin.x + 4.0, card.origin.y + 4.0)),
            UpdateModalHit::Inside
        );
        assert_eq!(
            modal.hit_test(card, Point2D::new(1.0, 1.0)),
            UpdateModalHit::Outside
        );
    }

    #[test]
    fn installing_has_no_buttons() {
        let state = state_with(UpdatePhase::Installing);
        let modal = UpdateModal::for_editor(&state).unwrap();
        assert!(modal.button_rects(modal.rect(1200.0, 800.0)).is_empty());
    }
}
