//! `flex-wrap: wrap` as native wrap auto layout (#379).
//!
//! A wrapping flex row becomes a `horizontal` container with `wrap: true`;
//! the layout engine breaks the lines like the browser does, so nothing is
//! measured or chunked at import time. `column-gap` stays the container's
//! `gap` and `row-gap` becomes `rowGap` when it differs. Column wrapping
//! (which needs a definite height) and `wrap-reverse` keep their warnings.

use crate::import_warning::ImportWarning;
use jian_ops_schema::node::base::NumberOrExpression;
use jian_ops_schema::node::container::{ContainerProps, LayoutMode};
use jian_ops_schema::node::PenNode;

use crate::css::cascade::ComputedStyle;
use crate::length::{parse_length, LengthCtx};

use super::MapCtx;

/// The grid then the wrap pass (#379): a container's children after both
/// native auto-layout post-passes, with `container` updated to match.
pub(crate) fn apply_flow_passes(
    context: &mut MapCtx<'_>,
    style: &ComputedStyle,
    container: &mut ContainerProps,
    children: Vec<PenNode>,
) -> Vec<PenNode> {
    let children = super::grid::apply_grid(context, style, container, children);
    apply_flex_wrap(context, style, container, children)
}

/// Turn a wrapping flex row into native wrap auto layout.
///
/// Children come back in the order they came in: this pass runs after
/// `mapper_stack::layer_absolute_children`, whose paint order must hold.
fn apply_flex_wrap(
    context: &mut MapCtx<'_>,
    style: &ComputedStyle,
    container: &mut ContainerProps,
    children: Vec<PenNode>,
) -> Vec<PenNode> {
    let Some(mode) = style.get("flex-wrap").map(str::trim) else {
        return children;
    };
    if !matches!(mode, "wrap" | "wrap-reverse")
        || !matches!(style.get("display"), Some("flex" | "inline-flex"))
        || children.is_empty()
    {
        return children;
    }
    if matches!(
        style.get("flex-direction"),
        Some("column" | "column-reverse")
    ) {
        context.warn_once(ImportWarning::FlexWrapColumnNotEmulated);
        return children;
    }
    if mode == "wrap-reverse" {
        context.warn_once(ImportWarning::FlexWrapReversePlain);
    }
    if style
        .get("align-content")
        .is_some_and(|value| !matches!(value.trim(), "" | "normal" | "stretch" | "flex-start"))
    {
        context.warn_once(ImportWarning::FlexAlignContentIgnored);
    }
    let column_gap = axis_gap(style, context, "column-gap");
    let row_gap = axis_gap(style, context, "row-gap");
    container.layout = Some(LayoutMode::Horizontal);
    container.wrap = Some(true);
    container.gap = (column_gap > 0.0).then_some(NumberOrExpression::Number(column_gap));
    container.row_gap = (row_gap != column_gap).then_some(NumberOrExpression::Number(row_gap));
    children
}

fn axis_gap(style: &ComputedStyle, context: &MapCtx<'_>, axis: &str) -> f64 {
    let Some(value) = style.get(axis).or_else(|| style.get("gap")) else {
        return 0.0;
    };
    if value.eq_ignore_ascii_case("normal") {
        return 0.0;
    }
    parse_length(
        value,
        &LengthCtx {
            font_size: style.font_size,
            root_font_size: context.opts.base_font_size,
            viewport_w: context.opts.viewport_width,
            viewport_h: context.opts.viewport_height(),
        },
    )
    .map(|length| length.resolve(context.containing_width))
    .filter(|value| value.is_finite() && *value >= 0.0)
    .unwrap_or(0.0)
}

#[cfg(test)]
#[path = "mapper_wrap_tests.rs"]
mod integration_tests;
