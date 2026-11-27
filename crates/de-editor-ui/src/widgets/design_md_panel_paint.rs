//! Design-MD panel paint helpers — section header/body rows, colour
//! swatches, inline markdown runs, and shared button chrome. Split out
//! of `design_md_panel.rs` to keep that module under the 800-line
//! ceiling. A `#[path]` submodule (declared as a child of
//! `design_md_panel`, not re-exported) so this second `impl
//! DesignMdPanel` block keeps access to the parent's private items
//! (`SectionLayout` / `RenderLine` / theme constants / `t` / `text`).

use super::helpers::hex_to_color;
use super::{DesignMdPanel, RenderLine, SectionLayout, BODY_INDENT, BTN, PAD, SECTION_HEADER_H};
use crate::widgets::button::paint_button_feedback_wash;
use crate::widgets::design_md_markdown::MdRun;
use crate::widgets::design_md_wrap::{PlacedRun, COLOR_SWATCH_W};
use crate::widgets::text_metrics;
use crate::widgets::{Icon, PaintCx};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::DesignMdButton;

impl<'a> DesignMdPanel<'a> {
    /// Paint one section's collapsible header row.
    pub(super) fn paint_section_header(&self, cx: &mut PaintCx<'_>, sec: &SectionLayout) {
        cx.backend
            .fill_round_rect(sec.header, 7.0, self.theme.muted);
        let target = DesignMdButton::ToggleSection(sec.index);
        paint_button_feedback_wash(
            cx.backend,
            &self.theme,
            sec.header,
            7.0,
            self.hover == Some(target),
            self.is_pressed(target),
        );
        let chevron = if sec.expanded { "▾" } else { "▸" };
        let baseline = sec.header.origin.y + SECTION_HEADER_H / 2.0 + 4.0;
        self.text(
            cx,
            chevron,
            sec.header.origin.x + 8.0,
            baseline,
            10.0,
            self.theme.muted_foreground,
        );
        self.text(
            cx,
            self.t(sec.title_key),
            sec.header.origin.x + 22.0,
            baseline,
            11.0,
            self.theme.foreground,
        );
    }

    /// Paint one expanded section's body lines.
    pub(super) fn paint_section_body(
        &self,
        cx: &mut PaintCx<'_>,
        panel: Rect,
        sec: &SectionLayout,
    ) {
        let left = panel.origin.x + PAD + BODY_INDENT;
        let mut y = sec.body_top;
        for line in sec.lines.iter() {
            match line {
                RenderLine::Heading(size, runs) => {
                    self.paint_runs(cx, runs, left, y + 12.0, *size, true);
                }
                RenderLine::Body(runs) => {
                    self.paint_runs(cx, runs, left, y + 11.0, 11.0, false);
                }
                RenderLine::Bullet(runs) => {
                    self.text(
                        cx,
                        "•",
                        left - 10.0,
                        y + 11.0,
                        11.0,
                        self.theme.muted_foreground,
                    );
                    self.paint_runs(cx, runs, left, y + 11.0, 11.0, false);
                }
                RenderLine::Color(i) => {
                    self.paint_color_row(cx, panel, *i, y);
                }
                RenderLine::Gap => {}
            }
            y += line.height();
        }
    }

    /// Paint one colour-palette row — swatch + name + hex/role.
    fn paint_color_row(&self, cx: &mut PaintCx<'_>, panel: Rect, index: usize, y: f32) {
        let Some(color) = self
            .spec
            .and_then(|s| s.color_palette.as_ref())
            .and_then(|c| c.get(index))
        else {
            return;
        };
        let left = panel.origin.x + PAD + BODY_INDENT;
        let swatch = Rect {
            origin: Point2D::new(left, y + 3.0),
            size: Point2D::new(16.0, 16.0),
        };
        cx.backend
            .fill_round_rect(swatch, 4.0, hex_to_color(&color.hex));
        cx.backend
            .stroke_round_rect(swatch, 4.0, self.theme.border, 1.0);
        let text_x = left + 24.0;
        let text_w = (panel.origin.x + panel.size.x - PAD - text_x).max(0.0);
        let name = text_metrics::fit_chrome(cx.backend, &color.name, text_w, 11.0);
        self.text(cx, &name, text_x, y + 11.0, 11.0, self.theme.foreground);
        let detail = if color.role.is_empty() {
            color.hex.clone()
        } else {
            format!("{} — {}", color.hex, color.role)
        };
        let detail = text_metrics::fit_chrome(cx.backend, &detail, text_w, 10.0);
        self.text(
            cx,
            &detail,
            text_x,
            y + 22.0,
            10.0,
            self.theme.muted_foreground,
        );
    }

    /// Paint a sequence of inline-styled runs left-to-right. `heading`
    /// brightens plain text; non-headings render plain text muted.
    fn paint_runs(
        &self,
        cx: &mut PaintCx<'_>,
        runs: &[PlacedRun],
        x: f32,
        baseline: f32,
        size: f32,
        heading: bool,
    ) {
        // Every run advances by the width the body wrapped it at, carried
        // on the run: a repaint measures nothing.
        let mut cur = x;
        for PlacedRun { run, advance } in runs {
            let advance = *advance;
            match run {
                MdRun::Plain(s) => {
                    let color = if heading {
                        self.theme.foreground
                    } else {
                        self.theme.muted_foreground
                    };
                    self.text(cx, s, cur, baseline, size, color);
                }
                MdRun::Bold(s) => {
                    self.text(cx, s, cur, baseline, size, self.theme.foreground);
                }
                MdRun::Code(s) => {
                    cx.backend.fill_round_rect(
                        Rect {
                            origin: Point2D::new(cur - 2.0, baseline - 10.0),
                            size: Point2D::new(advance, 14.0),
                        },
                        3.0,
                        self.theme.muted,
                    );
                    self.text(cx, s, cur, baseline, size, self.theme.foreground);
                }
                MdRun::Color(hex) => {
                    let swatch = Rect {
                        origin: Point2D::new(cur, baseline - 9.0),
                        size: Point2D::new(10.0, 10.0),
                    };
                    cx.backend.fill_round_rect(swatch, 2.0, hex_to_color(hex));
                    cx.backend
                        .stroke_round_rect(swatch, 2.0, self.theme.border, 1.0);
                    self.text(
                        cx,
                        hex,
                        cur + COLOR_SWATCH_W,
                        baseline,
                        size,
                        self.theme.foreground,
                    );
                }
            }
            cur += advance;
        }
    }

    /// Paint one square header icon button.
    pub(super) fn icon_button(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        icon: Icon,
        hovered: bool,
        pressed: bool,
    ) {
        cx.backend.fill_round_rect(rect, 6.0, self.theme.muted);
        jian_widgets::components::icon_button::IconButton {
            icon_paths: icon.paths(),
            hovered,
            pressed,
            active: false,
            enabled: true,
            icon_size: BTN - 10.0,
            stroke_width: 1.5,
        }
        .paint(
            cx.backend,
            rect,
            &crate::widgets::button::tokens_from_theme(&self.theme),
        );
    }

    pub(super) fn action_button(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        icon: Icon,
        label: &str,
        hovered: bool,
        pressed: bool,
    ) {
        jian_widgets::components::button::Button {
            label,
            icon_paths: Some(icon.paths()),
            // Secondary (muted fill) preserves the empty-state CTA's button
            // affordance — Ghost would render it transparent until hover.
            variant: jian_widgets::components::button::ButtonVariant::Secondary,
            enabled: true,
            hovered,
            pressed,
            font_size: 11.0,
        }
        .paint(
            cx.backend,
            rect,
            &crate::widgets::button::tokens_from_theme(&self.theme),
        );
    }

    /// Draw one line of text.
    pub(super) fn text(
        &self,
        cx: &mut PaintCx<'_>,
        s: &str,
        x: f32,
        baseline_y: f32,
        size: f32,
        color: Color,
    ) {
        let layout = TextLayout::single_run(
            s,
            text_metrics::CHROME_FONT_FAMILY,
            size,
            (color).to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(&layout, Point2D::new(x, baseline_y));
    }
}
