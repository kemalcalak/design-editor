//! The web host's left card (#86) — split out of `paint.rs`.
//!
//! The card floats over the full-bleed canvas, so it paints after it: its
//! ground, the body (slides navigator, a tab's body or the Pages + Layers
//! tree, plus the tab row) clipped to the rounded corners, then the
//! hairline — unless the pinned chat fills the body, in which case the
//! chat pass draws the hairline after itself (mirrors native
//! `paint_rail.rs`).

use super::slides_panel::SlidesFrame;
use super::WidgetHost;
use de_editor_ui::widgets::floating_panels::CARD_RADIUS;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::widgets::{LayerPanel, PaintCx, Widget};
use de_editor_ui::{Point2D, RenderBackend};

impl WidgetHost {
    pub(in crate::widget_host) fn paint_left_card(
        &mut self,
        backend: &mut dyn RenderBackend,
        viewport_height: f32,
        slides_panel: Option<&SlidesFrame>,
    ) {
        if !self.left_card_shown() {
            return;
        }
        let card = canvas_geometry::layer_panel_rect(&self.editor_state, viewport_height);
        backend.fill_round_rect(card, CARD_RADIUS, self.theme.card);
        backend.save();
        backend.clip_round_rect(card, CARD_RADIUS);
        self.paint_left_card_body(backend, viewport_height, slides_panel);
        backend.restore();
        if !self.editor_state.editor_ui.chat_pinned() {
            self.paint_left_card_border(backend, viewport_height);
        }
    }

    /// The left card's hairline.
    pub(in crate::widget_host) fn paint_left_card_border(
        &self,
        backend: &mut dyn RenderBackend,
        viewport_height: f32,
    ) {
        if !self.left_card_shown() {
            return;
        }
        let card = canvas_geometry::layer_panel_rect(&self.editor_state, viewport_height);
        backend.stroke_round_rect(card, CARD_RADIUS, self.theme.border, 1.0);
    }

    fn paint_left_card_body(
        &mut self,
        backend: &mut dyn RenderBackend,
        viewport_height: f32,
        slides_panel: Option<&SlidesFrame>,
    ) {
        // Slides tab — the same card showing the deck's page navigator
        // instead of the layer tree.
        if let Some(slides) = slides_panel {
            self.paint_slides_panel(&mut *backend, slides);
        }
        // Chat tab — the pinned chat owns the card's BODY (it paints in the
        // later chat pass of `paint.rs`); the card keeps its ground behind
        // the panel and its tab row on top, and a minimized chat carried in
        // from a floating session is expanded exactly like native does.
        let chat_owns_rail = slides_panel.is_none()
            && de_editor_ui::widgets::slides_panel_flow::chat_tab_active(&self.editor_state);
        if chat_owns_rail {
            backend.fill_rect(self.layer_panel_rect(viewport_height), self.theme.card);
            if self.editor_state.chat.is_minimized() {
                self.editor_state.chat.expand();
            }
            if let Some(tabs) = self.slides_tab_row(viewport_height) {
                self.paint_slides_tab_row(backend, &tabs);
            }
        }
        // Components / Images own the body too: the card paints its tab
        // row; Images paints its body here, the component browser later
        // (mirrors native).
        let other_tab_owns_rail = slides_panel.is_none()
            && !chat_owns_rail
            && !de_editor_ui::widgets::slides_panel_flow::layers_tab_active(&self.editor_state);
        if other_tab_owns_rail {
            backend.fill_rect(self.layer_panel_rect(viewport_height), self.theme.card);
            if let Some(tabs) = self.slides_tab_row(viewport_height) {
                self.paint_slides_tab_row(backend, &tabs);
            }
            let body = de_editor_ui::widgets::host_canvas_geometry::rail_body_rect(
                &self.editor_state,
                viewport_height,
            );
            de_editor_ui::widgets::images_tab::paint(
                &mut de_editor_ui::widgets::PaintCx { backend },
                &self.editor_state,
                body,
            );
        }
        if slides_panel.is_none() && !chat_owns_rail && !other_tab_owns_rail {
            let layer_panel_rect = self.layer_panel_rect(viewport_height);
            // While a drag is active, paint against a panel with the
            // source's subtree excluded — see native paint.rs. The
            // panel walks the canonical `PenNode` tree off
            // `EditorState`; the drag source id is shell-core's
            // `NodeId` from the input path, losslessly accepted.
            let active_drag = self.layer_drag.clone().filter(|d| {
                d.active
                    && self
                        .layout_scene
                        .active_page()
                        .map(|p| p.find(d.source.as_str()).is_some())
                        .unwrap_or(false)
            });
            let mut layer_panel = if let Some(d) = &active_drag {
                LayerPanel::from_editor_with_drag_source(&self.editor_state, &d.source)
            } else {
                // Per-frame paint: resolve the row model through the
                // owner-scoped cache so idle / streaming / hover repaints
                // that don't touch the layer tree skip the walk + measure.
                self.layer_panel()
            };

            // Auto-reveal selected node: if the selection changed and differs
            // from the last-revealed anchor, expand ancestors and reveal.
            // This covers MCP set_selection, undo/redo, and programmatic
            // selection changes (not just canvas clicks).
            if active_drag.is_none() {
                // Only auto-reveal when not dragging; explicit drag interactions
                // take precedence and manual collapse should be respected.
                let should_reveal = match (
                    &self.editor_state.selection.anchor,
                    &self.editor_state.editor_ui.last_revealed_layer_anchor,
                ) {
                    (anchor, last) if anchor.is_real() => Some(anchor) != last.as_ref(),
                    _ => false,
                };
                if should_reveal {
                    de_editor_ui::widgets::scroll_flow::reveal_layer_panel_selection(
                        &mut self.editor_state,
                        &layer_panel,
                        layer_panel_rect,
                    );
                    // Rebuild the panel after reveal to reflect any expanded ancestors.
                    layer_panel = self.layer_panel();
                }
            }

            if let Some(d) = &active_drag {
                layer_panel.drop_target = layer_panel
                    .drop_target_at(layer_panel_rect, Point2D::new(d.current_x, d.current_y));
                if let Some(item) = LayerPanel::ghost_item_for(&self.editor_state, &d.source) {
                    layer_panel.drag_ghost = Some((item, d.current_y));
                }
            }
            layer_panel.now_ms = self.now_ms;
            {
                let mut cx = PaintCx { backend };
                layer_panel.paint(&mut cx, layer_panel_rect);
            }
            // The tab row heads the rail in BOTH tabs — it is how the
            // user gets back to the slides.
            if let Some(tabs) = self.slides_tab_row(viewport_height) {
                self.paint_slides_tab_row(backend, &tabs);
            }
        }
    }
}
