//! Vector edit mode (#387): point-level editing of a `PenNode::Path`.
//!
//! A path is edited as a list of [`Subpath`]s (one anchor list per
//! `M … [Z]` run), so compound SVG paths and boolean results edit point
//! by point without a schema change. The session itself only keeps the
//! node id, the point selection and transient pen state; the geometry
//! is always re-derived from the document ([`source::node_subpaths`]),
//! so undo / redo inside the mode can never leave a stale copy behind.
//!
//! Frames: every coordinate this module hands out or accepts is in the
//! path's *parent-local* frame — the frame `base.x / base.y` live in and
//! the one editor-authored `anchors` already use (scale 1, see
//! `pen::refit_path_bounds`). A `d`-only path is mapped into that frame
//! on read (its native box fitted into `x / y / width / height`, the same
//! fit the canvas painter applies), and written back in it after the
//! first edit. Until that first edit the document — including the
//! original `d` — is never touched.
//!
//! Layout: `parse` (`d` → subpaths, arcs via `arc`), `geometry` (split,
//! heal, join, reverse), `snap` (45° constraint, pixel grid, point and
//! axis snapping), `source` / `write` (document ↔ subpaths), `session`
//! (the `EditorState` operations) and `pen` (pen-tool continue / close
//! inside the mode).

mod arc;
mod geometry;
mod parse;
mod pen;
mod session;
mod snap;
mod source;
mod write;

#[cfg(test)]
mod tests_geometry;
#[cfg(test)]
mod tests_parse;
#[cfg(test)]
mod tests_session;

use crate::node_id::NodeId;
use jian_ops_schema::node::PenPathAnchor;
use std::collections::BTreeSet;

pub use arc::arc_to_cubics;
pub use geometry::{
    cubic_point, delete_points, join_endpoints, nearest_on_segment, reverse_subpath,
    segment_controls, segment_count, split_cubic, split_segment,
};
pub use parse::parse_path_data;
pub use snap::{constrain_45, snap_point, snap_to_pixel_grid, SnapGuide, SnapResult};
pub use source::{node_subpaths, subpaths_bounds};
pub use write::subpaths_to_path_data;

/// One 2-D point / vector in the path's parent-local frame.
pub type Pt = (f64, f64);

/// One `M … [Z]` run of a path: its anchors (handles are anchor-relative
/// deltas, as in the schema) and whether it closes back to the start.
#[derive(Debug, Clone, PartialEq)]
pub struct Subpath {
    pub anchors: Vec<PenPathAnchor>,
    pub closed: bool,
}

impl Subpath {
    /// An open subpath through `points`, all corners.
    pub fn polyline(points: &[Pt]) -> Self {
        Self {
            anchors: points.iter().map(|p| corner(*p)).collect(),
            closed: false,
        }
    }

    /// True when `idx` is the first or last anchor of an OPEN subpath —
    /// the points the pen can continue from and Cmd+J can join.
    pub fn is_endpoint(&self, idx: usize) -> bool {
        !self.closed && !self.anchors.is_empty() && (idx == 0 || idx + 1 == self.anchors.len())
    }
}

/// A corner anchor (no handles) at `p`.
pub fn corner(p: Pt) -> PenPathAnchor {
    PenPathAnchor {
        x: p.0,
        y: p.1,
        handle_in: None,
        handle_out: None,
        point_type: None,
    }
}

/// One anchor of one subpath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointRef {
    pub sub: usize,
    pub idx: usize,
}

impl PointRef {
    pub fn new(sub: usize, idx: usize) -> Self {
        Self { sub, idx }
    }
}

/// Where the pen keeps drawing inside the mode: the subpath and which
/// end of it (`at_start` prepends, so continuing from the first anchor
/// never has to reverse the stored path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PenContinue {
    pub sub: usize,
    pub at_start: bool,
}

/// Live vector-edit session — UI state only, never serialized.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorEditSession {
    /// The path being edited (also the single selected node).
    pub node_id: NodeId,
    /// Selected points.
    pub selected: BTreeSet<PointRef>,
    /// Q toggles lasso: the next empty-canvas drag draws a free-form
    /// lasso instead of a rectangle.
    pub lasso: bool,
    /// Pen drawing target inside the mode.
    pub pen_continue: Option<PenContinue>,
    /// Snap guides of the last snapped move, in scene coordinates, for
    /// the overlay. Cleared when a gesture ends.
    pub guides: Vec<SnapGuide>,
    /// The marquee / lasso being dragged, scene coordinates (a closed
    /// polygon; a marquee is its four corners). Empty when idle.
    pub marquee: Vec<Pt>,
    /// Cursor (scene coordinates) for the pen's rubber band while the
    /// pen continues a subpath.
    pub pen_cursor: Option<Pt>,
}

impl VectorEditSession {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            selected: BTreeSet::new(),
            lasso: false,
            pen_continue: None,
            guides: Vec::new(),
            marquee: Vec::new(),
            pen_cursor: None,
        }
    }
}
