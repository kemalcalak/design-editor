//! Auto layout, sizing and positioning as flexbox / grid declarations.
//!
//! This restates the rules jian's taffy pass applies (`jian-core`
//! `layout/resolve.rs` + `flow_child.rs`), so the browser lays the export
//! out the way the canvas does:
//! - `layout: vertical | horizontal` is a flex column / row; an absent
//!   `layout` is a row too, but there a child with `x` / `y` is absolutely
//!   positioned;
//! - `layout: none` stacks its children in one grid cell, and `x` / `y`
//!   children are absolute;
//! - inside auto layout `x` / `y` are ignored unless the child is an overlay
//!   (`role: "overlay"`) or has constraints;
//! - `fill_container` on the main axis shares the free space
//!   (`flex: 1 1 0`), on the cross axis it stretches; `fit_content` hugs;
//! - `alignItems: stretch` reads as start (the canvas has no stretch pass),
//!   and a main-axis number never shrinks.

use super::ir::Decls;
use super::value::{fixed_size, number_or_expr_px, px};
use jian_ops_schema::node::{AlignItems, ContainerProps, JustifyContent, LayoutMode, Padding};
use jian_ops_schema::node::{PenNode, PenNodeBase, ScriptNode};
use jian_ops_schema::sizing::{SizeLimits, SizingBehavior, SizingKeyword};

/// How a node's parent lays it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Parent {
    /// A top-level node: sits in the page flow.
    Root,
    /// A flex parent. `auto_layout` is false for a frame with no `layout`,
    /// whose positioned children leave the flow.
    Flow { horizontal: bool, auto_layout: bool },
    /// A `layout: none` stack.
    Stack,
}

/// The parent context a container gives its children.
pub(crate) fn child_parent(container: &ContainerProps) -> Parent {
    match container.layout {
        Some(LayoutMode::None) => Parent::Stack,
        Some(LayoutMode::Vertical) => Parent::Flow {
            horizontal: false,
            auto_layout: true,
        },
        Some(LayoutMode::Horizontal) => Parent::Flow {
            horizontal: true,
            auto_layout: true,
        },
        None => Parent::Flow {
            horizontal: true,
            auto_layout: false,
        },
    }
}

/// Display, direction, alignment, gap, padding and clipping of a container
/// that has children.
pub(crate) fn apply_container(decls: &mut Decls, c: &ContainerProps) {
    if matches!(c.layout, Some(LayoutMode::None)) {
        decls.set("display", "grid");
        let justify = match c.justify_content {
            Some(JustifyContent::Center) => "center",
            Some(JustifyContent::End) => "end",
            _ => "start",
        };
        let align = match c.align_items {
            Some(AlignItems::Center) => "center",
            Some(AlignItems::End) => "end",
            _ => "start",
        };
        decls.set("justify-items", justify);
        decls.set("align-items", align);
    } else {
        decls.set("display", "flex");
        if matches!(c.layout, Some(LayoutMode::Vertical)) {
            decls.set("flex-direction", "column");
        }
        let justify = match c.justify_content {
            Some(JustifyContent::Center) => Some("center"),
            Some(JustifyContent::End) => Some("flex-end"),
            Some(JustifyContent::SpaceBetween) => Some("space-between"),
            Some(JustifyContent::SpaceAround) => Some("space-around"),
            Some(JustifyContent::Start) | None => None,
        };
        if let Some(j) = justify {
            decls.set("justify-content", j);
        }
        // CSS defaults to stretch; the canvas to start.
        let align = match c.align_items {
            Some(AlignItems::Center) => "center",
            Some(AlignItems::End) => "flex-end",
            _ => "flex-start",
        };
        decls.set("align-items", align);
        if let Some(gap) = c.gap.as_ref().and_then(number_or_expr_px) {
            if gap != "0" {
                decls.set("gap", gap);
            }
        }
    }
    if let Some(padding) = c.padding.as_ref().and_then(padding_css) {
        decls.set("padding", padding);
    }
}

/// CSS `padding` shorthand. `.de` padding is already CSS order: one value,
/// `[vertical, horizontal]`, or `[top, right, bottom, left]`.
fn padding_css(p: &Padding) -> Option<String> {
    let out = match p {
        Padding::Uniform(v) => px(*v),
        Padding::XY([v, h]) => format!("{} {}", px(*v), px(*h)),
        Padding::LtrB([t, r, b, l]) => format!("{} {} {} {}", px(*t), px(*r), px(*b), px(*l)),
        Padding::Expression(e) => super::value::length_expr(e)?,
    };
    (out != "0" && out != "0 0" && out != "0 0 0 0").then_some(out)
}

/// The node's authored size props, `(width, height, limits)`.
pub(crate) fn node_sizing(
    node: &PenNode,
) -> (
    Option<&SizingBehavior>,
    Option<&SizingBehavior>,
    Option<&SizeLimits>,
) {
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => (
            n.container.width.as_ref(),
            n.container.height.as_ref(),
            Some(&n.container.limits),
        ),
        PenNode::Group(n) => (
            n.container.width.as_ref(),
            n.container.height.as_ref(),
            Some(&n.container.limits),
        ),
        PenNode::Rectangle(n) => (
            n.container.width.as_ref(),
            n.container.height.as_ref(),
            Some(&n.container.limits),
        ),
        PenNode::Text(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::TextInput(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::IconFont(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Image(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Ellipse(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Polygon(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Path(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::TextArea(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Select(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Switch(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Checkbox(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Slider(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::RadioGroup(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::NumberInput(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Progress(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Tabs(n) => (n.width.as_ref(), n.height.as_ref(), Some(&n.limits)),
        PenNode::Note(n) | PenNode::Context(n) | PenNode::Prompt(n) => {
            (n.width.as_ref(), n.height.as_ref(), None)
        }
        PenNode::Browser(n) => (n.width.as_ref(), n.height.as_ref(), None),
        PenNode::Line(_) | PenNode::Ref(_) => (None, None, None),
    }
}

/// Width / height declarations for one axis pair. `absolute` marks a child
/// taken out of the flow (it fills its parent's padding box instead).
pub(crate) fn apply_size(
    decls: &mut Decls,
    width: Option<&SizingBehavior>,
    height: Option<&SizingBehavior>,
    limits: Option<&SizeLimits>,
    parent: Parent,
    absolute: bool,
) {
    let fill = |s: Option<&SizingBehavior>| {
        matches!(
            s,
            Some(SizingBehavior::Keyword(SizingKeyword::FillContainer))
        )
    };
    if let Some(w) = width.and_then(fixed_size) {
        decls.set("width", w);
    }
    if let Some(h) = height.and_then(fixed_size) {
        decls.set("height", h);
    }
    match (parent, absolute) {
        (Parent::Flow { horizontal, .. }, false) => {
            let (main_fill, cross_fill) = if horizontal {
                (fill(width), fill(height))
            } else {
                (fill(height), fill(width))
            };
            if main_fill {
                if horizontal {
                    decls.set("flex", "1 1 0px");
                    decls.set("min-width", "0");
                } else {
                    decls.set("flex-grow", "1");
                    // The canvas lets a fill-height box shrink below its content.
                    decls.set("min-height", "0");
                }
            }
            if cross_fill {
                decls.set("align-self", "stretch");
            }
            let main = if horizontal { width } else { height };
            if matches!(
                main,
                Some(SizingBehavior::Number(_) | SizingBehavior::Expression(_))
            ) {
                decls.set("flex-shrink", "0");
            }
        }
        _ => {
            if fill(width) {
                decls.set("width", "100%");
            }
            if fill(height) {
                decls.set("height", "100%");
            }
        }
    }
    if let Some(l) = limits {
        for (prop, v) in [
            ("min-width", l.min_width),
            ("max-width", l.max_width),
            ("min-height", l.min_height),
            ("max-height", l.max_height),
        ] {
            if let Some(v) = v {
                decls.set(prop, px(v));
            }
        }
    }
}

/// True when the child leaves its parent's flow at `left` / `top`.
pub(crate) fn is_absolute(base: &PenNodeBase, parent: Parent) -> bool {
    let positioned = base.x.is_some() || base.y.is_some();
    match parent {
        Parent::Root => false,
        Parent::Stack => positioned,
        Parent::Flow { auto_layout, .. } => {
            positioned
                && (!auto_layout
                    || base.role.as_deref() == Some("overlay")
                    || base.constraints.is_some())
        }
    }
}

/// `position: absolute` at the authored offset.
pub(crate) fn apply_absolute(decls: &mut Decls, base: &PenNodeBase) {
    decls.set("position", "absolute");
    decls.set("left", px(base.x.unwrap_or(0.0)));
    decls.set("top", px(base.y.unwrap_or(0.0)));
}
