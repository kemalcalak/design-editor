//! `FilePill` — the floating file identity at the canvas top-left (#30).
//!
//! `[⌂ │ name  Saved]`: the house returns to Home, the name opens the file
//! menu (which also carries the app-level actions the retired title bar
//! used to hold), and the save-state label saves.
//!
//! Layout runs on a width estimate, like Home's (hit-testing has no text
//! backend on the web host): paint fits the real glyphs into the rects
//! the estimate produced, so the painted pill and its click targets can
//! never disagree.

use crate::theme::Theme;
use crate::widgets::chrome_hit::ChromeHit;
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::home_surface::estimate_width;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics::{measure_chrome_weighted, CHROME_FONT_FAMILY};
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::ChromeButton;

pub const FILE_PILL_HEIGHT: f32 = 34.0;
const RADIUS: f32 = 10.0;
const HOME_BUTTON: f32 = 28.0;
const HOME_ICON: f32 = 16.0;
/// Inset of the home button from the pill's left / top edge.
const INSET: f32 = 3.0;
const DIVIDER_H: f32 = 14.0;
const NAME_FONT: f32 = 13.0;
const STATE_FONT: f32 = 12.0;
/// Longest the file name may grow before it is ellipsized.
const NAME_MAX_W: f32 = 220.0;
const NAME_GAP: f32 = 10.0;
const PAD_RIGHT: f32 = 12.0;
/// Space before the name — divider plus breathing room.
const NAME_LEAD: f32 = 12.0;
/// The width estimate runs a little narrow of Manrope at these weights; the
/// slack keeps the real glyphs inside their slots instead of ellipsizing.
const ESTIMATE_SLACK: f32 = 1.15;
/// Accent dot leading the "Edited" label.
const DOT: f32 = 6.0;

/// Whether the document on disk matches the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveState {
    /// Bound to a file and unchanged since the last save.
    Saved,
    /// Bound to a file with edits not yet written.
    Edited,
    /// Never saved — no file to be in sync with.
    NotSaved,
}

impl SaveState {
    pub fn for_editor_ui(ui: &EditorUiState) -> Self {
        match (&ui.file_name_display, ui.document_dirty) {
            (None, _) => Self::NotSaved,
            (Some(_), true) => Self::Edited,
            (Some(_), false) => Self::Saved,
        }
    }
}

/// The pill's rects, from one layout pass shared by paint and hit-test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilePillLayout {
    pub rect: Rect,
    /// `None` inside a VS Code embed, where the workbench owns Home.
    pub home: Option<Rect>,
    pub divider_x: Option<f32>,
    pub name: Rect,
    /// `None` inside a VS Code embed, where the workbench owns saving.
    pub save: Option<Rect>,
}

pub struct FilePill {
    pub file_name: String,
    pub save_state: SaveState,
    pub label_saved: &'static str,
    pub label_edited: &'static str,
    pub label_not_saved: &'static str,
    pub theme: Theme,
    pub hover: Option<ChromeButton>,
    pub pressed: Option<ChromeButton>,
    /// The VS Code embed drops Home and the save state — the workbench
    /// already owns both.
    pub embedded: bool,
}

impl FilePill {
    pub fn for_editor_ui(ui: &EditorUiState) -> Self {
        Self {
            file_name: ui
                .file_name_display
                .clone()
                .unwrap_or_else(|| translate(ui, "common.untitled").to_string()),
            save_state: SaveState::for_editor_ui(ui),
            label_saved: translate(ui, "fileMenu.saved"),
            label_edited: translate(ui, "chrome.edited"),
            label_not_saved: translate(ui, "chrome.notSaved"),
            theme: theme_for(ui),
            hover: ui.chrome_button_hover,
            pressed: match ui.pressed_button {
                Some(de_editor_core::ButtonPressTarget::Chrome(button)) => Some(button),
                _ => None,
            },
            embedded: ui.embed == de_editor_core::EmbedHost::VsCode,
        }
    }

    pub fn save_label(&self) -> &'static str {
        match self.save_state {
            SaveState::Saved => self.label_saved,
            SaveState::Edited => self.label_edited,
            SaveState::NotSaved => self.label_not_saved,
        }
    }

    fn name_width(&self) -> f32 {
        (estimate_width(&self.file_name, NAME_FONT) * ESTIMATE_SLACK).min(NAME_MAX_W)
    }

    fn save_width(&self) -> f32 {
        let dot = if self.save_state == SaveState::Edited {
            DOT + 5.0
        } else {
            0.0
        };
        dot + estimate_width(self.save_label(), STATE_FONT) * ESTIMATE_SLACK
    }

    /// Lay the pill out with its top-left at `origin`.
    pub fn layout_at(&self, origin: Point2D) -> FilePillLayout {
        let y = origin.y;
        let (home, divider_x, name_x) = if self.embedded {
            (None, None, origin.x + PAD_RIGHT)
        } else {
            let home = Rect::xywh(origin.x + INSET, y + INSET, HOME_BUTTON, HOME_BUTTON);
            let divider_x = home.origin.x + HOME_BUTTON + 4.0;
            (Some(home), Some(divider_x), divider_x + NAME_LEAD)
        };
        // The name's click target spans the pill's height so a press just
        // above or below the glyphs still opens the menu.
        let name = Rect::xywh(name_x - 4.0, y, self.name_width() + 8.0, FILE_PILL_HEIGHT);
        let mut right = name.origin.x + name.size.x;
        let save = (!self.embedded).then(|| {
            let save = Rect::xywh(
                right + NAME_GAP - 8.0,
                y,
                self.save_width() + 8.0,
                FILE_PILL_HEIGHT,
            );
            right = save.origin.x + save.size.x;
            save
        });
        let rect = Rect::xywh(
            origin.x,
            y,
            right + PAD_RIGHT - 4.0 - origin.x,
            FILE_PILL_HEIGHT,
        );
        FilePillLayout {
            rect,
            home,
            divider_x,
            name,
            save,
        }
    }

    pub fn hit_test(&self, layout: &FilePillLayout, point: Point2D) -> Option<ChromeHit> {
        if !layout.rect.contains(point) {
            return None;
        }
        if layout.home.is_some_and(|r| r.contains(point)) {
            return Some(ChromeHit::Home);
        }
        if layout.save.is_some_and(|r| r.contains(point)) {
            return Some(ChromeHit::Save);
        }
        if layout.name.contains(point) {
            return Some(ChromeHit::ToggleFileMenu);
        }
        None
    }

    /// The rect a chrome button's tooltip / dropdown anchors to.
    pub fn button_rect(&self, layout: &FilePillLayout, button: ChromeButton) -> Option<Rect> {
        match button {
            ChromeButton::Home => layout.home,
            ChromeButton::ToggleFileMenu => Some(layout.name),
            ChromeButton::Save => layout.save,
            _ => None,
        }
    }

    fn wash(&self, cx: &mut PaintCx<'_>, rect: Rect, button: ChromeButton) -> bool {
        let pressed = self.pressed == Some(button);
        let hovered = self.hover == Some(button);
        if hovered || pressed {
            let inset = Rect::xywh(
                rect.origin.x,
                rect.origin.y + INSET,
                rect.size.x,
                rect.size.y - INSET * 2.0,
            );
            cx.backend
                .fill_round_rect(inset, 7.0, self.theme.button_hover);
        }
        hovered || pressed
    }

    pub fn paint_layout(&self, cx: &mut PaintCx<'_>, layout: &FilePillLayout) {
        let theme = self.theme;
        cx.backend.fill_round_rect(layout.rect, RADIUS, theme.card);
        cx.backend
            .stroke_round_rect(layout.rect, RADIUS, theme.border, 1.0);
        let center_y = layout.rect.origin.y + FILE_PILL_HEIGHT / 2.0;

        if let Some(home) = layout.home {
            let active = self.wash(cx, home, ChromeButton::Home);
            let color = if active {
                theme.foreground
            } else {
                theme.muted_foreground
            };
            draw_icon(
                cx.backend,
                Icon::Home,
                Point2D::new(
                    home.origin.x + (HOME_BUTTON - HOME_ICON) / 2.0,
                    center_y - HOME_ICON / 2.0,
                ),
                HOME_ICON,
                color,
                1.5,
            );
        }
        if let Some(x) = layout.divider_x {
            cx.backend.fill_rect(
                Rect::xywh(x, center_y - DIVIDER_H / 2.0, 1.0, DIVIDER_H),
                theme.border,
            );
        }

        self.wash(cx, layout.name, ChromeButton::ToggleFileMenu);
        let name = fit_weighted(cx, &self.file_name, self.name_width(), NAME_FONT, 600);
        draw_run(
            cx,
            &name,
            NAME_FONT,
            600,
            theme.foreground,
            Point2D::new(
                layout.name.origin.x + 4.0,
                jian_widgets::centered_text_baseline_y(layout.rect, NAME_FONT),
            ),
        );

        if let Some(save) = layout.save {
            let active = self.wash(cx, save, ChromeButton::Save);
            let mut x = save.origin.x + 4.0;
            if self.save_state == SaveState::Edited {
                cx.backend
                    .fill_oval(Rect::xywh(x, center_y - DOT / 2.0, DOT, DOT), theme.primary);
                x += DOT + 5.0;
            }
            let color = match (active, self.save_state) {
                (true, _) => theme.foreground,
                (false, SaveState::Saved) => theme.muted_subtle,
                (false, _) => theme.muted_foreground,
            };
            let label_w = save.origin.x + save.size.x - 4.0 - x;
            let label = fit_weighted(cx, self.save_label(), label_w, STATE_FONT, 500);
            draw_run(
                cx,
                &label,
                STATE_FONT,
                500,
                color,
                Point2D::new(
                    x,
                    jian_widgets::centered_text_baseline_y(layout.rect, STATE_FONT),
                ),
            );
        }
    }
}

impl Widget for FilePill {
    fn id(&self) -> WidgetId {
        WidgetId::new(5000)
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect {
                origin: Point2D::ZERO,
                size: self.layout_at(Point2D::ZERO).rect.size,
            },
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        self.paint_layout(cx, &self.layout_at(rect.origin));
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Toolbar);
        node.set_label("File");
        node
    }
}

/// Ellipsize `text` into `max_w`, measured at the weight it is drawn in —
/// the slot comes from a width estimate, so the real glyphs must be fitted
/// into it rather than trusted to agree.
fn fit_weighted(cx: &mut PaintCx<'_>, text: &str, max_w: f32, size: f32, weight: u16) -> String {
    crate::util::ellipsize_to_width(text, max_w, |s| {
        measure_chrome_weighted(cx.backend, s, size, weight)
    })
}

fn draw_run(
    cx: &mut PaintCx<'_>,
    text: &str,
    size: f32,
    weight: u16,
    color: Color,
    origin: Point2D,
) {
    if text.is_empty() {
        return;
    }
    let layout = TextLayout::single_run(
        text,
        CHROME_FONT_FAMILY,
        size,
        color.to_jian(),
        Point2D::ZERO,
    )
    .with_font_weight(weight);
    cx.backend.draw_text(&layout, origin);
}
