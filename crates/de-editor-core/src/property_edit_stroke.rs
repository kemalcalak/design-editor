//! Stroke-style writes on the anchor node: alignment (#90), cap / join /
//! dash / ends (#89, #389) and the miter limit (#389). Split out of
//! `property_edit_mutators.rs` for the 800-line cap.

use crate::state::EditorState;

impl EditorState {
    /// Set the anchor node's stroke alignment. A node without a stroke
    /// is left alone (see `fills::set_stroke_align`).
    pub fn set_selected_stroke_align(
        &mut self,
        align: jian_ops_schema::style::StrokeAlign,
    ) -> bool {
        let changed = self
            .with_selected_node(|node| crate::fills::set_stroke_align(node, align))
            .unwrap_or(false);
        if changed {
            self.mark_document_changed();
        }
        changed
    }

    /// Apply a cap / join / dash / end click to the anchor node's stroke.
    /// A node without a stroke is left alone.
    pub fn set_selected_stroke_style(&mut self, edit: &crate::fills::StrokeStyleEdit) -> bool {
        let changed = self
            .with_selected_node(|node| crate::fills::apply_stroke_style_edit(node, edit))
            .unwrap_or(false);
        if changed {
            self.mark_document_changed();
        }
        changed
    }

    /// The Stroke section's Miter field (#389): the limit clamped to
    /// `1..=MAX_MITER_LIMIT`. A non-finite value is refused.
    pub fn set_selected_stroke_miter_limit(&mut self, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let limit = value.clamp(1.0, crate::fills::MAX_MITER_LIMIT);
        self.set_selected_stroke_style(&crate::fills::StrokeStyleEdit::MiterLimit(limit))
    }
}
