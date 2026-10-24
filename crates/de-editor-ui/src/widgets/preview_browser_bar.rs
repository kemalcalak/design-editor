//! APP MODE browser bar (#153): back, forward and an address field that
//! shows the mounted page's title and route, so a routed preview reads
//! like a website in a browser.
//!
//! Shared by paint and hit-testing like its siblings
//! (`preview_device_switcher.rs`, `screen_switcher_pills.rs`). It sits
//! directly below the device switcher; the screen pills sit below it.
//! The bar paints only in APP MODE, the same gate as the pills.

use de_editor_core::editor_ui_state::groups::PreviewBrowserControl;

use crate::widgets::file_menu::truncate_to_width;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::preview_device_switcher::PreviewDeviceSwitcher;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, TextLayout, Theme};

const BAR_MAX_W: f32 = 360.0;
const BAR_H: f32 = 28.0;
const BUTTON_W: f32 = 28.0;
const GAP: f32 = 4.0;
const RADIUS: f32 = 12.0;
/// Gap below the device switcher's pill, matching the pills' row gap.
pub(crate) const ROW_GAP: f32 = 8.0;
/// Side margin kept free on a narrow canvas.
const SIDE_MARGIN: f32 = 16.0;
const ICON: f32 = 14.0;
const FONT: f32 = 11.0;
const ADDRESS_PAD_X: f32 = 10.0;

pub struct PreviewBrowserBar<'a> {
    /// The mounted page's name.
    pub title: &'a str,
    /// The mounted page's route, e.g. `/pricing`.
    pub route: &'a str,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub hover: Option<PreviewBrowserControl>,
    pub pressed: Option<PreviewBrowserControl>,
}

impl PreviewBrowserBar<'_> {
    /// Bar bounds: centred like the device switcher, `ROW_GAP` below it.
    pub fn bar_rect(canvas: Rect) -> Rect {
        let width = BAR_MAX_W
            .min(canvas.size.x - SIDE_MARGIN * 2.0)
            .max(BUTTON_W * 3.0);
        let device_pill = PreviewDeviceSwitcher::pill_rect(canvas);
        Rect {
            origin: Point2D::new(
                canvas.origin.x + (canvas.size.x - width) / 2.0,
                device_pill.origin.y + device_pill.size.y + ROW_GAP,
            ),
            size: Point2D::new(width, BAR_H),
        }
    }

    pub fn button_rect(canvas: Rect, control: PreviewBrowserControl) -> Rect {
        let bar = Self::bar_rect(canvas);
        let index = match control {
            PreviewBrowserControl::Back => 0.0,
            PreviewBrowserControl::Forward => 1.0,
        };
        Rect {
            origin: Point2D::new(bar.origin.x + index * (BUTTON_W + GAP), bar.origin.y),
            size: Point2D::new(BUTTON_W, BAR_H),
        }
    }

    pub fn address_rect(canvas: Rect) -> Rect {
        let bar = Self::bar_rect(canvas);
        let left = 2.0 * (BUTTON_W + GAP);
        Rect {
            origin: Point2D::new(bar.origin.x + left, bar.origin.y),
            size: Point2D::new((bar.size.x - left).max(0.0), BAR_H),
        }
    }

    /// The button under `point`, if any.
    pub fn hit_test(canvas: Rect, point: Point2D) -> Option<PreviewBrowserControl> {
        [PreviewBrowserControl::Back, PreviewBrowserControl::Forward]
            .into_iter()
            .find(|control| Self::button_rect(canvas, *control).contains(point))
    }

    /// Whether `point` is anywhere on the bar (presses there never reach
    /// the page underneath).
    pub fn contains(canvas: Rect, point: Point2D) -> bool {
        Self::bar_rect(canvas).contains(point)
    }

    fn enabled(&self, control: PreviewBrowserControl) -> bool {
        match control {
            PreviewBrowserControl::Back => self.can_go_back,
            PreviewBrowserControl::Forward => self.can_go_forward,
        }
    }

    pub fn paint(&self, cx: &mut PaintCx<'_>, canvas: Rect, theme: &Theme) {
        for (control, icon) in [
            (PreviewBrowserControl::Back, Icon::ChevronLeft),
            (PreviewBrowserControl::Forward, Icon::ChevronRight),
        ] {
            let rect = Self::button_rect(canvas, control);
            let enabled = self.enabled(control);
            let fill = if enabled && self.pressed == Some(control) {
                theme.accent
            } else if enabled && self.hover == Some(control) {
                theme.muted
            } else {
                theme.card
            };
            cx.backend.fill_round_rect(rect, RADIUS, fill);
            cx.backend
                .stroke_round_rect(rect, RADIUS, theme.border, 1.0);
            let color = if enabled {
                theme.foreground
            } else {
                theme.muted_foreground
            };
            draw_icon(
                cx.backend,
                icon,
                Point2D::new(
                    rect.origin.x + (rect.size.x - ICON) / 2.0,
                    rect.origin.y + (rect.size.y - ICON) / 2.0,
                ),
                ICON,
                color,
                1.75,
            );
        }

        let address = Self::address_rect(canvas);
        cx.backend.fill_round_rect(address, RADIUS, theme.card);
        cx.backend
            .stroke_round_rect(address, RADIUS, theme.border, 1.0);
        let max_w = (address.size.x - ADDRESS_PAD_X * 2.0).max(0.0);
        let baseline_y = address.origin.y + address.size.y / 2.0 + 4.0;
        let title = truncate_to_width(cx, self.title, FONT, max_w);
        let title_w = cx.backend.measure_text_family(&title, FONT, "system-ui");
        let mut x = address.origin.x + ADDRESS_PAD_X;
        self.text(cx, &title, x, baseline_y, theme.foreground);
        x += title_w + 8.0;
        let route_w = (address.origin.x + address.size.x - ADDRESS_PAD_X - x).max(0.0);
        if route_w > FONT * 2.0 {
            let route = truncate_to_width(cx, self.route, FONT, route_w);
            self.text(cx, &route, x, baseline_y, theme.muted_foreground);
        }
    }

    fn text(&self, cx: &mut PaintCx<'_>, text: &str, x: f32, baseline_y: f32, color: crate::Color) {
        let layout = TextLayout::single_run(
            text,
            "system-ui",
            FONT,
            color.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(&layout, Point2D::new(x, baseline_y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> Rect {
        Rect {
            origin: Point2D::new(100.0, 40.0),
            size: Point2D::new(1000.0, 700.0),
        }
    }

    #[test]
    fn bar_sits_centred_below_the_device_switcher() {
        let bar = PreviewBrowserBar::bar_rect(canvas());
        let pill = PreviewDeviceSwitcher::pill_rect(canvas());
        assert!((bar.origin.y - (pill.origin.y + pill.size.y + ROW_GAP)).abs() < 0.5);
        assert!((bar.size.x - BAR_MAX_W).abs() < 0.5);
        let centre = bar.origin.x + bar.size.x / 2.0;
        assert!((centre - (canvas().origin.x + canvas().size.x / 2.0)).abs() < 0.5);
    }

    #[test]
    fn buttons_hit_test_and_the_address_does_not() {
        let back = PreviewBrowserBar::button_rect(canvas(), PreviewBrowserControl::Back);
        let forward = PreviewBrowserBar::button_rect(canvas(), PreviewBrowserControl::Forward);
        let mid = |r: Rect| Point2D::new(r.origin.x + r.size.x / 2.0, r.origin.y + r.size.y / 2.0);
        assert_eq!(
            PreviewBrowserBar::hit_test(canvas(), mid(back)),
            Some(PreviewBrowserControl::Back)
        );
        assert_eq!(
            PreviewBrowserBar::hit_test(canvas(), mid(forward)),
            Some(PreviewBrowserControl::Forward)
        );
        let address = PreviewBrowserBar::address_rect(canvas());
        assert_eq!(PreviewBrowserBar::hit_test(canvas(), mid(address)), None);
        assert!(PreviewBrowserBar::contains(canvas(), mid(address)));
    }

    #[test]
    fn narrow_canvas_shrinks_the_bar_inside_its_margins() {
        let narrow = Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(300.0, 600.0),
        };
        let bar = PreviewBrowserBar::bar_rect(narrow);
        assert!(bar.origin.x >= SIDE_MARGIN - 0.5);
        assert!(bar.origin.x + bar.size.x <= 300.0 - SIDE_MARGIN + 0.5);
    }
}
