//! Components tab — the Libraries section (#15).
//!
//! Sits under the kit strip: a header row ("Libraries" + Reload + Link)
//! and one row per linked library file with its component count (or why
//! it could not be read) and an Unlink button. Sibling of
//! `component_browser_panel.rs` for the 800-line cap; the panel calls in
//! here for geometry, hit-test, hover and paint.

use crate::widgets::button::paint_button_feedback_wash;
use crate::widgets::component_browser_panel::{
    ComponentBrowserHit, ComponentBrowserPanel, IMPORTED_ROW_H, PAD,
};
use crate::widgets::text_metrics::{fit_chrome, measure_chrome};
use crate::widgets::{draw_icon, Icon, PaintCx};
use crate::{Point2D, Rect};
use de_editor_core::ComponentBrowserButton as Btn;
use de_editor_core::{LibraryPanelAction, LibraryStatus, LinkedLibrary};

/// The "Libraries" header row.
const LIB_HEADER_H: f32 = 28.0;
/// Square icon buttons on the header and the rows.
const LIB_BTN: f32 = 18.0;
const LIB_BTN_GAP: f32 = 4.0;

impl ComponentBrowserPanel<'_> {
    fn libraries(&self) -> &[LinkedLibrary] {
        &self.state.editor_ui.libraries.entries
    }

    /// Height of the Libraries section (header + one row per link).
    pub(in crate::widgets) fn library_strip_height(&self) -> f32 {
        LIB_HEADER_H + self.libraries().len() as f32 * IMPORTED_ROW_H
    }

    /// Top of the section: right under the kit strip's own rows.
    fn library_top(&self, panel: Rect) -> f32 {
        panel.origin.y + crate::widgets::component_browser_panel::HEADER_H + self.kit_strip_height()
    }

    fn header_button(&self, panel: Rect, slot: usize) -> Rect {
        let right = panel.origin.x + panel.size.x - PAD;
        let x = right - LIB_BTN - slot as f32 * (LIB_BTN + LIB_BTN_GAP);
        Rect {
            origin: Point2D::new(x, self.library_top(panel) + (LIB_HEADER_H - LIB_BTN) / 2.0),
            size: Point2D::new(LIB_BTN, LIB_BTN),
        }
    }

    fn link_rect(&self, panel: Rect) -> Rect {
        self.header_button(panel, 0)
    }

    fn reload_rect(&self, panel: Rect) -> Rect {
        self.header_button(panel, 1)
    }

    fn library_row_rects(&self, panel: Rect) -> Vec<Rect> {
        let top = self.library_top(panel) + LIB_HEADER_H;
        (0..self.libraries().len())
            .map(|i| Rect {
                origin: Point2D::new(panel.origin.x, top + i as f32 * IMPORTED_ROW_H),
                size: Point2D::new(panel.size.x, IMPORTED_ROW_H),
            })
            .collect()
    }

    fn unlink_rect(panel: Rect, row: Rect) -> Rect {
        Rect {
            origin: Point2D::new(
                panel.origin.x + panel.size.x - PAD - LIB_BTN,
                row.origin.y + (IMPORTED_ROW_H - LIB_BTN) / 2.0,
            ),
            size: Point2D::new(LIB_BTN, LIB_BTN),
        }
    }

    /// Click routing for the section; `None` falls through.
    pub(in crate::widgets) fn hit_test_library_strip(
        &self,
        panel: Rect,
        point: Point2D,
    ) -> Option<ComponentBrowserHit> {
        if self.link_rect(panel).contains(point) {
            return Some(ComponentBrowserHit::Library(LibraryPanelAction::Link));
        }
        if !self.libraries().is_empty() && self.reload_rect(panel).contains(point) {
            return Some(ComponentBrowserHit::Library(LibraryPanelAction::Reload));
        }
        for (entry, row) in self.libraries().iter().zip(self.library_row_rects(panel)) {
            if Self::unlink_rect(panel, row).contains(point) {
                return Some(ComponentBrowserHit::Library(LibraryPanelAction::Unlink(
                    entry.link.path.clone(),
                )));
            }
        }
        None
    }

    pub(in crate::widgets) fn hover_library_strip(
        &self,
        panel: Rect,
        point: Point2D,
    ) -> Option<Btn> {
        if self.link_rect(panel).contains(point) {
            return Some(Btn::LibraryLink);
        }
        if !self.libraries().is_empty() && self.reload_rect(panel).contains(point) {
            return Some(Btn::LibraryReload);
        }
        self.library_row_rects(panel)
            .into_iter()
            .position(|row| Self::unlink_rect(panel, row).contains(point))
            .map(Btn::LibraryUnlink)
    }

    /// Status text on a row: the component count, or why it failed.
    fn library_status_label(&self, status: &LibraryStatus) -> String {
        match status {
            LibraryStatus::Synced { components, .. } => {
                format!("{components} {}", self.t("componentBrowser.components"))
            }
            LibraryStatus::NotLoaded => self.t("componentBrowser.libraryNotLoaded").to_string(),
            LibraryStatus::Missing => self.t("componentBrowser.libraryMissing").to_string(),
            LibraryStatus::Invalid(_) => self.t("componentBrowser.libraryInvalid").to_string(),
        }
    }

    fn paint_icon_button(&self, cx: &mut PaintCx<'_>, rect: Rect, icon: Icon, button: Btn) {
        paint_button_feedback_wash(
            cx.backend,
            &self.theme,
            rect,
            4.0,
            self.hover == Some(button),
            self.is_pressed(button),
        );
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(rect.origin.x + 3.0, rect.origin.y + 3.0),
            LIB_BTN - 6.0,
            self.theme.muted_foreground,
            1.4,
        );
    }

    /// Paint the section between the kit rows and the category pills.
    pub(in crate::widgets) fn paint_library_strip(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        let top = self.library_top(panel);
        self.text(
            cx,
            self.t("componentBrowser.libraries"),
            panel.origin.x + PAD,
            top + LIB_HEADER_H / 2.0 + 4.0,
            11.0,
            self.theme.muted_foreground,
        );
        self.paint_icon_button(cx, self.link_rect(panel), Icon::Link, Btn::LibraryLink);
        if !self.libraries().is_empty() {
            self.paint_icon_button(
                cx,
                self.reload_rect(panel),
                Icon::RefreshCw,
                Btn::LibraryReload,
            );
        }
        for (i, (entry, row)) in self
            .libraries()
            .iter()
            .zip(self.library_row_rects(panel))
            .enumerate()
        {
            let problem = entry.status.is_problem();
            let unlink = Self::unlink_rect(panel, row);
            let status = self.library_status_label(&entry.status);
            // Measured, not estimated: CJK labels are twice as wide.
            let status_w = measure_chrome(cx.backend, &status, 10.0);
            let status_x = unlink.origin.x - 8.0 - status_w;
            let name_w = (status_x - panel.origin.x - PAD - 8.0).max(24.0);
            let name = fit_chrome(cx.backend, &entry.link.display_name(), name_w, 11.0);
            self.text(
                cx,
                &name,
                panel.origin.x + PAD,
                row.origin.y + IMPORTED_ROW_H / 2.0 + 4.0,
                11.0,
                self.theme.foreground,
            );
            self.text(
                cx,
                &status,
                status_x,
                row.origin.y + IMPORTED_ROW_H / 2.0 + 3.5,
                10.0,
                if problem {
                    self.theme.destructive
                } else {
                    self.theme.muted_foreground
                },
            );
            self.paint_icon_button(cx, unlink, Icon::Unlink, Btn::LibraryUnlink(i));
        }
        cx.backend.fill_rect(
            Rect {
                origin: Point2D::new(panel.origin.x, top + self.library_strip_height()),
                size: Point2D::new(panel.size.x, 1.0),
            },
            self.theme.border,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::{EditorState, LibraryLink};

    fn state_with_links() -> EditorState {
        let mut state = EditorState::new();
        state.editor_ui.open_components_tab();
        state.editor_ui.libraries.set_links(vec![
            LibraryLink::new("kits/ui.de", None),
            LibraryLink::new("brand.de", Some("Brand".into())),
        ]);
        state.editor_ui.libraries.entries[1].status = LibraryStatus::Missing;
        state
    }

    fn panel_rect() -> Rect {
        Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(300.0, 800.0),
        }
    }

    #[test]
    fn rows_route_unlink_and_header_routes_link_and_reload() {
        let state = state_with_links();
        let panel = ComponentBrowserPanel::for_editor(&state).expect("components tab");
        let rect = panel_rect();
        let link = panel.link_rect(rect);
        let center = |r: Rect| Point2D::new(r.origin.x + 4.0, r.origin.y + 4.0);
        assert_eq!(
            panel.hit_test(rect, center(link)),
            Some(ComponentBrowserHit::Library(LibraryPanelAction::Link))
        );
        assert_eq!(
            panel.hit_test(rect, center(panel.reload_rect(rect))),
            Some(ComponentBrowserHit::Library(LibraryPanelAction::Reload))
        );
        let rows = panel.library_row_rects(rect);
        let unlink = ComponentBrowserPanel::unlink_rect(rect, rows[1]);
        assert_eq!(
            panel.hit_test(rect, center(unlink)),
            Some(ComponentBrowserHit::Library(LibraryPanelAction::Unlink(
                "brand.de".into()
            )))
        );
        assert_eq!(
            panel.hover_at(rect, center(unlink)),
            Some(Btn::LibraryUnlink(1))
        );
    }

    #[test]
    fn the_section_grows_one_row_per_link() {
        let mut state = state_with_links();
        let with = ComponentBrowserPanel::for_editor(&state)
            .unwrap()
            .library_strip_height();
        state.editor_ui.libraries.set_links(Vec::new());
        let without = ComponentBrowserPanel::for_editor(&state)
            .unwrap()
            .library_strip_height();
        assert_eq!(with - without, 2.0 * IMPORTED_ROW_H);
    }
}
