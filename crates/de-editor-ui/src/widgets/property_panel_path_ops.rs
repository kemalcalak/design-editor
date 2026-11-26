//! Path operation buttons in the Position header (#388), left of the
//! flip buttons. A boolean group shows its four ops (the active one lit;
//! a click switches it) and Flatten; a shape shows Flatten, Outline
//! stroke when it has a stroke, and Simplify / Reverse when it is a path.
//! Clicks run the same `PathOperation` as the menus and shortcuts,
//! through the host (the geometry needs skia).

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::PaintCx;
use crate::Rect;
use de_editor_core::path_ops::PathOperation;
use de_editor_core::BooleanOp;
use jian_ops_schema::node::PenNode;

const BUTTON: f32 = 20.0;
const GAP: f32 = 2.0;
const GLYPH: f32 = 14.0;
/// Room the two flip buttons take at the right of the header.
const FLIP_WIDTH: f32 = BUTTON * 2.0 + GAP;

/// Which path buttons the selected node gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PathOpsButtons {
    /// The node is a boolean group using this op.
    pub boolean: Option<BooleanOp>,
    /// Flatten applies (a shape, a group or a boolean group).
    pub flatten: bool,
    /// The node has a visible stroke to outline.
    pub outline: bool,
    /// The node is a path (Simplify / Reverse).
    pub path: bool,
}

impl PathOpsButtons {
    /// The buttons for `node`, the panel's selected node.
    pub fn for_node(node: &PenNode) -> Self {
        use jian_ops_schema::style::StrokeThickness as T;
        let stroked = |s: Option<&jian_ops_schema::style::PenStroke>| {
            s.is_some_and(|s| match &s.thickness {
                T::Uniform(w) => *w > 0.0,
                T::PerSide(sides) => sides.iter().any(|w| *w > 0.0),
                T::Sided(_) => true,
            })
        };
        match node {
            PenNode::Boolean(b) => Self {
                boolean: Some(b.op),
                flatten: true,
                ..Self::default()
            },
            PenNode::Group(_) => Self {
                flatten: true,
                ..Self::default()
            },
            PenNode::Rectangle(n) => Self {
                flatten: true,
                outline: stroked(n.container.stroke.as_ref()),
                ..Self::default()
            },
            PenNode::Frame(n) => Self {
                flatten: n.children.as_ref().is_none_or(Vec::is_empty),
                outline: stroked(n.container.stroke.as_ref()),
                ..Self::default()
            },
            PenNode::Ellipse(n) => Self {
                flatten: true,
                outline: stroked(n.stroke.as_ref()),
                ..Self::default()
            },
            PenNode::Polygon(n) => Self {
                flatten: true,
                outline: stroked(n.stroke.as_ref()),
                ..Self::default()
            },
            PenNode::Line(n) => Self {
                outline: stroked(n.stroke.as_ref()),
                ..Self::default()
            },
            PenNode::Path(n) => Self {
                flatten: false,
                outline: stroked(n.stroke.as_ref()),
                path: true,
                ..Self::default()
            },
            _ => Self::default(),
        }
    }

    /// The buttons in paint order, left to right, each with its icon and
    /// whether it shows as active.
    fn items(&self) -> Vec<(PathOperation, Icon, bool)> {
        let mut out = Vec::new();
        if let Some(current) = self.boolean {
            for (op, icon) in [
                (BooleanOp::Union, Icon::SquaresUnite),
                (BooleanOp::Subtract, Icon::SquaresSubtract),
                (BooleanOp::Intersect, Icon::SquaresIntersect),
                (BooleanOp::Exclude, Icon::SquaresExclude),
            ] {
                out.push((PathOperation::Boolean(op), icon, op == current));
            }
        }
        if self.flatten {
            out.push((PathOperation::Flatten, Icon::Layers, false));
        }
        if self.outline {
            out.push((PathOperation::OutlineStroke, Icon::Square, false));
        }
        if self.path {
            out.push((PathOperation::Simplify, Icon::PenTool, false));
            out.push((PathOperation::Reverse, Icon::RefreshCw, false));
        }
        out
    }
}

/// Button rects, right-aligned against the flip buttons in the header row
/// at `section_y`.
fn rects(
    buttons: &PathOpsButtons,
    x: f32,
    section_y: f32,
    width: f32,
) -> Vec<(PathOperation, Icon, bool, Rect)> {
    let items = buttons.items();
    let right = x + width - crate::widgets::property_panel_inputs::PAD_X - FLIP_WIDTH - GAP * 4.0;
    let n = items.len() as f32;
    let mut left = right - n * BUTTON - (n - 1.0).max(0.0) * GAP;
    items
        .into_iter()
        .map(|(op, icon, active)| {
            let rect = Rect::xywh(left, section_y + 1.0, BUTTON, BUTTON);
            left += BUTTON + GAP;
            (op, icon, active, rect)
        })
        .collect()
}

pub(crate) fn push_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    buttons: &PathOpsButtons,
    x: f32,
    section_y: f32,
    width: f32,
) {
    for (op, _, _, rect) in rects(buttons, x, section_y, width) {
        out.push((PropertyPanelAction::PathOperation(op), rect));
    }
}

pub(crate) fn paint(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    buttons: &PathOpsButtons,
    x: f32,
    section_y: f32,
    width: f32,
) {
    for (_, icon, active, rect) in rects(buttons, x, section_y, width) {
        if active {
            cx.backend.fill_round_rect(rect, 5.0, theme.muted);
        }
        let inset = (BUTTON - GLYPH) / 2.0;
        draw_icon(
            cx.backend,
            icon,
            crate::Point2D::new(rect.origin.x + inset, rect.origin.y + inset),
            GLYPH,
            if active {
                theme.foreground
            } else {
                theme.muted_foreground
            },
            1.4,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boolean_group_offers_its_ops_and_flatten() {
        let b = PathOpsButtons {
            boolean: Some(BooleanOp::Intersect),
            flatten: true,
            ..PathOpsButtons::default()
        };
        let r = rects(&b, 0.0, 0.0, 272.0);
        assert_eq!(r.len(), 5);
        assert!(r[2].2, "intersect lit");
        assert_eq!(r[4].0, PathOperation::Flatten);
        // Clear of the flip buttons and of the header label.
        assert!(r[4].3.origin.x + BUTTON <= 272.0 - FLIP_WIDTH);
        assert!(r[0].3.origin.x > 90.0, "{:?}", r[0].3);
    }

    #[test]
    fn a_path_offers_simplify_and_reverse() {
        let b = PathOpsButtons {
            path: true,
            outline: true,
            ..PathOpsButtons::default()
        };
        let ops: Vec<PathOperation> = rects(&b, 0.0, 0.0, 272.0).iter().map(|r| r.0).collect();
        assert_eq!(
            ops,
            [
                PathOperation::OutlineStroke,
                PathOperation::Simplify,
                PathOperation::Reverse
            ]
        );
        assert!(rects(&PathOpsButtons::default(), 0.0, 0.0, 272.0).is_empty());
    }
}
