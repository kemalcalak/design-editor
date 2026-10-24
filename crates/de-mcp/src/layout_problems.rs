//! Clipping problems for `snapshot_layout` entries (#169), pen.dev's
//! `ctx.problems`: a node whose box leaves the area its clipping ancestors
//! (`clipContent` frames) show is `"partially clipped"`, one with nothing
//! left inside that area is `"fully clipped"`.
//!
//! Boxes are the resolved scene rects in document space. A rotated node is
//! checked by the axis-aligned box of its rotated rect; a rotated ancestor's
//! clip is taken unrotated.

use jian_scene::layout_scene::SceneNode;

/// Below this many document px an edge counts as touching, so float noise
/// from the layout solver is not reported.
const TOLERANCE: f32 = 0.5;

pub(crate) const PARTIALLY_CLIPPED: &str = "partially clipped";
pub(crate) const FULLY_CLIPPED: &str = "fully clipped";

/// An axis-aligned document-space box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Area {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Area {
    /// The node's own box. A line's bounds can have a negative size (it runs
    /// left or up from its origin), so the corners are sorted.
    fn of_bounds(node: &SceneNode) -> Self {
        let b = &node.bounds;
        let (x0, x1) = (b.origin.x, b.origin.x + b.size.x);
        let (y0, y1) = (b.origin.y, b.origin.y + b.size.y);
        Self {
            left: x0.min(x1),
            top: y0.min(y1),
            right: x0.max(x1),
            bottom: y0.max(y1),
        }
    }

    /// The node's painted box: its bounds (the children's union for an
    /// unbounded container, whose own bounds are zero), or the axis-aligned
    /// box around them when rotated about their centre.
    fn painted(node: &SceneNode) -> Self {
        let unbounded =
            node.bounds.size.x == 0.0 && node.bounds.size.y == 0.0 && !node.children.is_empty();
        let area = if unbounded {
            let b = node.aggregate_bounds();
            Self {
                left: b.origin.x,
                top: b.origin.y,
                right: b.origin.x + b.size.x.max(0.0),
                bottom: b.origin.y + b.size.y.max(0.0),
            }
        } else {
            Self::of_bounds(node)
        };
        if node.rotation == 0.0 || !node.rotation.is_finite() {
            return area;
        }
        let (w, h) = (area.right - area.left, area.bottom - area.top);
        let (sin, cos) = node.rotation.sin_cos();
        let half_w = (w * cos.abs() + h * sin.abs()) / 2.0;
        let half_h = (w * sin.abs() + h * cos.abs()) / 2.0;
        let (cx, cy) = (area.left + w / 2.0, area.top + h / 2.0);
        Self {
            left: cx - half_w,
            top: cy - half_h,
            right: cx + half_w,
            bottom: cy + half_h,
        }
    }

    /// Overlap of two areas; an empty overlap collapses to a zero-size box,
    /// so everything checked against it reads as fully clipped.
    fn intersect(self, other: Self) -> Self {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        Self {
            left,
            top,
            right: self.right.min(other.right).max(left),
            bottom: self.bottom.min(other.bottom).max(top),
        }
    }
}

/// The visible area `node`'s children get: `clip` narrowed to the node's own
/// box when it clips its content.
pub(crate) fn child_clip(node: &SceneNode, clip: Option<Area>) -> Option<Area> {
    if !node.clip_content {
        return clip;
    }
    let own = Area::of_bounds(node);
    Some(clip.map_or(own, |outer| outer.intersect(own)))
}

/// `node`'s problem inside the visible area `clip`, if any. Hidden nodes
/// paint nothing and report nothing.
pub(crate) fn clip_problem(node: &SceneNode, clip: Option<Area>) -> Option<&'static str> {
    let clip = clip?;
    if node.hidden {
        return None;
    }
    let b = Area::painted(node);
    let inside = b.left >= clip.left - TOLERANCE
        && b.top >= clip.top - TOLERANCE
        && b.right <= clip.right + TOLERANCE
        && b.bottom <= clip.bottom + TOLERANCE;
    if inside {
        return None;
    }
    let disjoint = b.right <= clip.left + TOLERANCE
        || b.left >= clip.right - TOLERANCE
        || b.bottom <= clip.top + TOLERANCE
        || b.top >= clip.bottom - TOLERANCE;
    Some(if disjoint {
        FULLY_CLIPPED
    } else {
        PARTIALLY_CLIPPED
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jian_scene::layout_scene::NodeKind;

    fn node(x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        let mut n = SceneNode::leaf("n", NodeKind::Rect);
        n.bounds.origin.x = x;
        n.bounds.origin.y = y;
        n.bounds.size.x = w;
        n.bounds.size.y = h;
        n
    }

    fn clipping(x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        let mut n = node(x, y, w, h);
        n.clip_content = true;
        n
    }

    #[test]
    fn inside_partial_and_outside_are_told_apart() {
        let clip = child_clip(&clipping(0.0, 0.0, 100.0, 100.0), None);
        assert_eq!(clip_problem(&node(10.0, 10.0, 50.0, 50.0), clip), None);
        assert_eq!(
            clip_problem(&node(80.0, 10.0, 50.0, 50.0), clip),
            Some(PARTIALLY_CLIPPED)
        );
        assert_eq!(
            clip_problem(&node(120.0, 10.0, 50.0, 50.0), clip),
            Some(FULLY_CLIPPED)
        );
        assert_eq!(
            clip_problem(&node(100.2, 0.0, 10.0, 10.0), clip),
            Some(FULLY_CLIPPED),
            "touching the edge leaves nothing visible"
        );
        assert_eq!(
            clip_problem(&node(0.0, 0.0, 100.3, 100.0), clip),
            None,
            "sub-pixel overshoot is layout noise"
        );
        assert_eq!(clip_problem(&node(500.0, 0.0, 10.0, 10.0), None), None);
    }

    #[test]
    fn a_frame_that_does_not_clip_passes_the_outer_area_on() {
        let frame = node(0.0, 0.0, 10.0, 10.0);
        assert_eq!(child_clip(&frame, None), None);
        let outer = child_clip(&clipping(0.0, 0.0, 100.0, 100.0), None);
        assert_eq!(child_clip(&frame, outer), outer);
    }

    #[test]
    fn a_line_drawn_leftwards_is_measured_by_its_real_extent() {
        let clip = child_clip(&clipping(0.0, 0.0, 100.0, 100.0), None);
        // From (50, 50) back to (-30, 50): sticks out on the left.
        assert_eq!(
            clip_problem(&node(50.0, 50.0, -80.0, 0.0), clip),
            Some(PARTIALLY_CLIPPED)
        );
        assert_eq!(clip_problem(&node(50.0, 50.0, -40.0, 0.0), clip), None);
    }

    #[test]
    fn hidden_nodes_report_nothing() {
        let clip = child_clip(&clipping(0.0, 0.0, 100.0, 100.0), None);
        let mut n = node(500.0, 0.0, 10.0, 10.0);
        n.hidden = true;
        assert_eq!(clip_problem(&n, clip), None);
    }

    #[test]
    fn rotation_uses_the_rotated_box() {
        let clip = child_clip(&clipping(0.0, 0.0, 100.0, 100.0), None);
        // A 90x10 bar centred in the clip still fits at 45 degrees (~71x71).
        let mut bar = node(5.0, 45.0, 90.0, 10.0);
        bar.rotation = std::f32::consts::FRAC_PI_4;
        assert_eq!(clip_problem(&bar, clip), None);
        // A 10x120 bar overshoots top and bottom; turned 90 degrees it
        // overshoots left and right instead.
        let mut tall = node(45.0, -10.0, 10.0, 120.0);
        tall.rotation = std::f32::consts::FRAC_PI_2;
        assert_eq!(clip_problem(&tall, clip), Some(PARTIALLY_CLIPPED));
    }

    #[test]
    fn nested_clips_intersect() {
        let outer = child_clip(&clipping(0.0, 0.0, 100.0, 100.0), None);
        let inner = child_clip(&clipping(50.0, 50.0, 100.0, 100.0), outer);
        assert_eq!(
            clip_problem(&node(110.0, 60.0, 10.0, 10.0), inner),
            Some(FULLY_CLIPPED),
            "inside the inner frame's box but outside the outer frame"
        );
        assert_eq!(clip_problem(&node(60.0, 60.0, 10.0, 10.0), inner), None);
    }
}
