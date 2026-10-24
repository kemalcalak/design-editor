//! Left-rail painting (slides navigator / Pages + Layers tree) — split
//! out of `paint.rs` for the 800-line ceiling.

use super::WidgetHostNative;
use crate::NativeFrameBackend;
use de_editor_ui::widgets::host_canvas_geometry as canvas_geometry;
use de_editor_ui::widgets::{LayerPanel, PaintCx, Widget};
use de_editor_ui::Point2D;

impl WidgetHostNative {
    /// The left card (slides navigator, Pages + Layers tree, or a tab's
    /// body). Painted AFTER the canvas: it floats over it (#86). Hidden
    /// in Preview, like its input (`left_card_on_screen`).
    pub(in crate::widget_host) fn paint_left_rail(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        // 3. The visible left rail — the persistent sidebar, never while
        //    presenting. It shows
        //    the deck's slides navigator (which OWNS the rail when it is
        //    on show), the Chat tab's column (the pinned chat paints over
        //    it in the ordinary chat pass, but the rail keeps its card
        //    behind the panel and the tab row on top), or the Pages +
        //    Layers tree.
        let presenting = self.preview_slideshow_active();
        // On screen while open AND while it slides shut (#86).
        if !self.left_rail_visible() || presenting {
            return;
        }
        // The card ground, then everything inside clipped to its corners.
        let card = canvas_geometry::layer_panel_rect(&self.editor_state, viewport_height);
        {
            use de_editor_ui::RenderBackend;
            let radius = de_editor_ui::widgets::floating_panels::CARD_RADIUS;
            frame.fill_round_rect(card, radius, self.theme.card);
            frame.save();
            frame.clip_round_rect(card, radius);
        }
        self.paint_left_card_body(frame, viewport_width, viewport_height);
        use de_editor_ui::RenderBackend;
        frame.restore();
        // A pinned chat fills the body later and draws the hairline after
        // itself; otherwise the card closes here, under the composer pill
        // that may travel across its edge.
        if !self.editor_state.editor_ui.chat_pinned() {
            self.paint_left_card_border(frame, viewport_height);
        }
    }

    /// Everything inside the left card: the slides navigator, a tab's body
    /// (Agent / Components / Images) or the Pages + Layers tree, and the
    /// tab row heading it.
    fn paint_left_card_body(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let slides_panel = self.slides_panel_frame(viewport_width, viewport_height);
        if let Some(slides) = &slides_panel {
            self.paint_slides_panel(frame, slides);
        }
        // The Chat tab owns the rail's BODY — there is no layer tree to
        // paint under the pinned chat, but the rail still paints its
        // card (the chat panel draws over it at the ordinary chat pass)
        // and its tab row, which is how the user gets back.
        let chat_owns_rail = slides_panel.is_none()
            && de_editor_ui::widgets::slides_panel_flow::chat_tab_active(&self.editor_state);
        if chat_owns_rail {
            let panel = self.layers_content_rect(viewport_height);
            use de_editor_ui::RenderBackend;
            frame.fill_rect(panel, self.theme.card);
            // The dock always shows the EXPANDED chat — a minimized bar
            // carried in from a floating session has no working expand
            // affordance while pinned, so reconcile here exactly like
            // the workspace chrome paint does.
            if self.editor_state.chat.is_minimized() {
                self.editor_state.chat.expand();
            }
            if let Some(tabs) = self.slides_tab_row(viewport_height) {
                self.paint_slides_tab_row(frame, &tabs);
            }
        }
        // Components / Images own the body too: the rail paints its card
        // and tab row; Images paints its body right here, the component
        // browser later in the floating-panel pass.
        let other_tab_owns_rail = slides_panel.is_none()
            && !chat_owns_rail
            && !de_editor_ui::widgets::slides_panel_flow::layers_tab_active(&self.editor_state);
        if other_tab_owns_rail {
            let panel = self.layers_content_rect(viewport_height);
            use de_editor_ui::RenderBackend;
            frame.fill_rect(panel, self.theme.card);
            if let Some(tabs) = self.slides_tab_row(viewport_height) {
                self.paint_slides_tab_row(frame, &tabs);
            }
            let body = de_editor_ui::widgets::host_canvas_geometry::rail_body_rect(
                &self.editor_state,
                viewport_height,
            );
            de_editor_ui::widgets::images_tab::paint(
                &mut de_editor_ui::widgets::PaintCx { backend: frame },
                &self.editor_state,
                body,
            );
        }
        if slides_panel.is_none() && !chat_owns_rail && !other_tab_owns_rail {
            // Compute the active drop target so the panel can paint
            // the drop-indicator line during a drag-to-reorder.
            // The rail is the tab row's leftovers when a tab row shows,
            // so paint and hit-test both start from the same rect.
            let layer_panel_rect = self.layers_content_rect(viewport_height);
            // Build the panel for paint. While a drag is active,
            // exclude the source's subtree so the rendered row stack
            // mirrors the post-commit layout — both the visible rows
            // and the drop-indicator y the user sees are then exactly
            // what `reorder_before/after` produces on release.
            // The panel walks the canonical `PenNode` tree directly
            // off `EditorState`; the drag source id is shell-core's
            // `NodeId` (from the input path), losslessly accepted.
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
                // Floating ghost — keeps the source visible mid-drag.
                if let Some(item) = LayerPanel::ghost_item_for(&self.editor_state, &d.source) {
                    layer_panel.drag_ghost = Some((item, d.current_y));
                }
            }
            layer_panel.now_ms = self.now_ms;
            {
                let mut cx = PaintCx {
                    backend: &mut *frame,
                };
                layer_panel.paint(&mut cx, layer_panel_rect);
            }
            // The tab row heads the rail in BOTH tabs — it is how the
            // user gets back to the slides — so it paints over the
            // layer tree's own card background here.
            if let Some(tabs) = self.slides_tab_row(viewport_height) {
                self.paint_slides_tab_row(frame, &tabs);
            }
        }
    }

    /// The left card's hairline.
    ///
    /// Painted AFTER the pinned chat, not at the end of the rail pass:
    /// the chat fills the card's body with its own ground, which would
    /// bury the border's lower half.
    pub(in crate::widget_host) fn paint_left_card_border(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_height: f32,
    ) {
        if !de_editor_ui::widgets::floating_panels::left_card_on_screen(&self.editor_state) {
            return;
        }
        use de_editor_ui::RenderBackend;
        let card = canvas_geometry::layer_panel_rect(&self.editor_state, viewport_height);
        frame.stroke_round_rect(
            card,
            de_editor_ui::widgets::floating_panels::CARD_RADIUS,
            self.theme.border,
            1.0,
        );
    }
}
