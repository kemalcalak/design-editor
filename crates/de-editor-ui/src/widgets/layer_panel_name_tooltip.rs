//! Full-name tooltip for a hovered layer row whose name does not fit (#306).
//!
//! A deep tree (an imported web page nests 15+ `div`s) or a long name
//! ellipsizes the row's label, and the hovered row loses a little more
//! width to its eye/lock actions. Resting on such a row names it in full.
//! Rows whose name fits get no tooltip, so sweeping a readable tree shows
//! nothing; like the rail tabs it appears at once rather than after a dwell.

use crate::widgets::editor_state_ext::theme_for;
use crate::widgets::host_canvas_geometry::{rail_body_rect, rail_tab_on_show};
use crate::widgets::layer_panel::LayerPanel;
use crate::widgets::layer_panel_metrics::{layer_indent, layer_label_x};
use crate::widgets::layer_panel_paint::{
    layer_label_available_width_with_metrics, truncate_to_fit_measured,
};
use crate::widgets::layer_panel_walkers::visible_row_range;
use crate::widgets::tooltip::{self, TooltipPlacement};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect, RenderBackend};
use de_editor_core::{EditorState, LeftPanelTab};

impl LayerPanel {
    /// Where the hovered row's label sits on screen and its full text,
    /// when the hovered row cannot show the whole name. `None` for a row
    /// that fits, one being renamed, or one scrolled out of the viewport.
    pub(crate) fn truncated_name_anchor(
        &self,
        backend: &mut dyn RenderBackend,
        rect: Rect,
    ) -> Option<(Rect, &str)> {
        let hovered = self.hovered_layer.as_ref()?;
        let r = self.regions(rect);
        // Only a row in the painted window can carry a tooltip.
        let index = visible_row_range(
            self.items.len(),
            r.layers.offset,
            r.layers_view_h,
            self.metrics.layer_row_height,
        )
        .find(|&index| &self.items[index].node_id == hovered)?;
        let item = &self.items[index];
        if item.renaming {
            return None;
        }
        let row = self.layer_row_rect(rect, &r, index);
        let viewport_bottom = r.layers_rows_top + r.layers_view_h;
        if row.origin.y < r.layers_rows_top || row.origin.y + row.size.y > viewport_bottom {
            return None;
        }
        let indent = layer_indent(item.depth, row.size.x, self.metrics);
        let label_x = layer_label_x(row, indent, self.metrics);
        let horizontal_offset = r.layers.horizontal_offset;
        let available_w =
            layer_label_available_width_with_metrics(row, label_x, horizontal_offset, true, false);
        // The same memoized fit the row paints with, so the tooltip shows
        // exactly when the hovered row's name ends in an ellipsis.
        let display =
            truncate_to_fit_measured(backend, &item.label, self.metrics.row_font, available_w);
        if display == item.label {
            return None;
        }
        let screen_label_x = label_x - horizontal_offset;
        let anchor = Rect {
            origin: Point2D::new(screen_label_x, row.origin.y),
            size: Point2D::new(available_w, row.size.y),
        };
        Some((anchor, item.label.as_str()))
    }
}

/// Paint the hovered layer row's full-name tooltip, if its name is cut.
/// `panel` builds the Layers panel the rail painted this frame; it runs only
/// once a row is hovered with the tree on show. No tooltip while a layer
/// drag is active (`drag_active`): the ghost row says what moves. Hosts
/// call this in the same pass as
/// [`super::chrome_tooltip::paint_chrome_tooltip`], so the bubble can hang
/// over the canvas.
///
/// Returns the painted rect, for tests.
pub fn paint_layer_name_tooltip(
    cx: &mut PaintCx<'_>,
    state: &EditorState,
    panel: impl FnOnce() -> LayerPanel,
    drag_active: bool,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<Rect> {
    let ui = &state.editor_ui;
    if ui.hovered_layer_id.is_none()
        || drag_active
        || !rail_tab_on_show(state, LeftPanelTab::Layers)
    {
        return None;
    }
    // The tree paints into the rail's body, under its tab row.
    let panel_rect = rail_body_rect(state, viewport_h);
    let panel = panel();
    let (anchor, label) = panel.truncated_name_anchor(cx.backend, panel_rect)?;
    Some(tooltip::paint_tooltip(
        cx,
        &theme_for(ui),
        anchor,
        label,
        None,
        TooltipPlacement::Below,
        Some(tooltip::viewport_clamp(viewport_w)),
    ))
}
