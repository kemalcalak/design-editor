//! Pencil (freehand) tool mutators (#88).
//!
//! A press starts a stroke, cursor moves append samples to
//! `ui.pencil_stroke` (UI state only — the document is untouched while
//! the drag runs, so nothing half-drawn reaches undo, MCP snapshots or a
//! save), and the release fits the samples into cubic beziers
//! ([`crate::pencil_fit`]) and inserts ONE ordinary `PenNode::Path`.
//! The pre-stroke snapshot is pushed with it, so undo removes exactly one
//! stroke. The path carries `anchors`, so the pen tool's point editing
//! works on it like on any authored path.

use crate::id_allocator::{IdAllocError, IdAllocator, SequentialIdAllocator};
use crate::node_id::NodeId;
use crate::pen::refit_path_bounds;
use crate::pen_node_ext::make_path;
use crate::render_backend::Point2D;
use crate::state::EditorState;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{StrokeCap, StrokeJoin, StrokeThickness};

/// Stroke colour of a new pencil path — the pen tool's default.
pub const PENCIL_STROKE_HEX: &str = "#374151";
/// Stroke width (doc px) of a new pencil path.
pub const PENCIL_STROKE_WIDTH: f32 = 2.0;
/// Minimum on-screen distance (px) between two recorded samples; closer
/// cursor moves are dropped so a slow drag does not pile up points.
pub const PENCIL_SAMPLE_SPACING_PX: f32 = 1.0;

impl EditorState {
    /// Start a freehand stroke at `doc` (document coords). Replaces any
    /// stroke left over from a lost release.
    pub fn begin_pencil_stroke(&mut self, doc: Point2D) {
        self.ui.pencil_stroke = Some(vec![doc]);
    }

    /// True between a Pencil press and its release.
    pub fn pencil_stroke_active(&self) -> bool {
        self.ui.pencil_stroke.is_some()
    }

    /// Append a sample while the stroke runs. Samples closer than
    /// [`PENCIL_SAMPLE_SPACING_PX`] on screen to the previous one are
    /// skipped. True when the sample was recorded (repaint the preview).
    pub fn extend_pencil_stroke(&mut self, doc: Point2D) -> bool {
        let min_step = PENCIL_SAMPLE_SPACING_PX / self.viewport.zoom.max(0.0001);
        let Some(points) = self.ui.pencil_stroke.as_mut() else {
            return false;
        };
        if let Some(last) = points.last() {
            if (doc.x - last.x).hypot(doc.y - last.y) < min_step {
                return false;
            }
        }
        points.push(doc);
        true
    }

    /// Drop the in-flight stroke without touching the document (pointer
    /// cancel, Escape, tool switch). True when one was active.
    pub fn cancel_pencil_stroke(&mut self) -> bool {
        self.ui.pencil_stroke.take().is_some()
    }

    /// Finish the stroke: fit the samples at the current
    /// `editor_ui.pencil_smoothing` tolerance, insert the path at the front
    /// of the active page, select it, and push one undo entry. A click
    /// without a drag (fewer than two distinct samples, or a zero-span
    /// result) inserts nothing and pushes no history. The tool stays on
    /// Pencil so the next stroke can start right away.
    pub fn finish_pencil_stroke(&mut self, next_id: &mut u64) -> Option<NodeId> {
        let mut allocator = SequentialIdAllocator::for_document(&self.doc, *next_id).ok();
        let result = match allocator.as_mut() {
            Some(allocator) => self
                .finish_pencil_stroke_with_allocator(allocator)
                .ok()
                .flatten(),
            None => {
                self.ui.pencil_stroke = None;
                None
            }
        };
        if let (Some(_), Some(allocator)) = (&result, allocator) {
            *next_id = allocator.next_counter();
        }
        result
    }

    /// Allocator-aware form of [`Self::finish_pencil_stroke`].
    pub fn finish_pencil_stroke_with_allocator(
        &mut self,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let Some(points) = self.ui.pencil_stroke.take() else {
            return Ok(None);
        };
        let tolerance = self.editor_ui.pencil_smoothing.tolerance_screen_px()
            / f64::from(self.viewport.zoom.max(0.0001));
        let samples: Vec<(f64, f64)> = points
            .iter()
            .map(|p| (f64::from(p.x), f64::from(p.y)))
            .collect();
        let anchors = crate::pencil_fit::fit_freehand_path(&samples, tolerance);
        if anchors.len() < 2 {
            return Ok(None);
        }
        let (_, _, w, h) = crate::path_bounds::path_bounds_from_anchors(&anchors, false);
        if w < 0.001 && h < 0.001 {
            return Ok(None);
        }
        let mut taken = self.collect_node_ids();
        let id = allocator.allocate(&mut taken)?;
        let pre = self.snapshot_for_history();
        let first = (anchors[0].x, anchors[0].y);
        let mut node = make_path(id.clone().into(), "Pencil", first);
        if let PenNode::Path(path) = &mut node {
            path.anchors = Some(anchors);
            path.closed = Some(false);
        }
        crate::fills::set_primary_fill_hex(&mut node, "transparent");
        crate::fills::set_primary_stroke_hex(&mut node, PENCIL_STROKE_HEX);
        if let PenNode::Path(path) = &mut node {
            if let Some(stroke) = path.stroke.as_mut() {
                stroke.thickness = StrokeThickness::Uniform(PENCIL_STROKE_WIDTH);
                stroke.cap = Some(StrokeCap::Round);
                stroke.join = Some(StrokeJoin::Round);
            }
        }
        refit_path_bounds(&mut node);
        // Index 0 is the front of the page, like every other new shape.
        self.active_children_mut().insert(0, node);
        self.mark_document_changed();
        self.history_push_past(pre);
        self.set_single_selection(id.clone());
        Ok(Some(id))
    }
}

#[cfg(test)]
#[path = "pencil_tests.rs"]
mod tests;
