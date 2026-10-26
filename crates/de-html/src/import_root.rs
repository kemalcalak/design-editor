//! The page root an ordinary (non-deck) import produces: `<body>` mapped
//! to one frame at the import viewport's width.

use jian_ops_schema::node::base::PenNodeBase;
use jian_ops_schema::node::container::{ContainerProps, Padding};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

use crate::css::cascade::ComputedStyle;
use crate::dom::DomElement;
use crate::mapper::{self, MapCtx};

/// Map `<body>` (the last element of `body_path`) to the import's root frame.
pub(crate) fn map_body_root(
    context: &mut MapCtx<'_>,
    body_path: &[&DomElement],
    body_style: &ComputedStyle,
    document_style: &ComputedStyle,
    title: Option<String>,
    body_display_none: bool,
) -> PenNode {
    let root_id = context.generate_id();
    let can_transfer_text_clip = mapper::text_scope::subtree_allows_text_clip_transfer(
        context,
        body_path,
        body_style,
        &body_path[1].children,
    );
    let (mut container, text_fill_override) = mapper::text_scope::container_props_with_text_scope(
        body_style,
        context,
        can_transfer_text_clip,
    );
    // A body that declares its own px width (a fixed-size page or poster)
    // keeps it; anything else is as wide as the import viewport.
    let root_width = match container.width {
        Some(SizingBehavior::Number(width)) if width.is_finite() && width > 0.0 => width,
        _ => context.opts.viewport_width,
    };
    container.width = Some(SizingBehavior::Number(root_width));
    container.height = container
        .height
        .or(Some(SizingBehavior::Keyword(SizingKeyword::FitContent)));
    let mut body_children_centered = false;
    if container.align_items.is_none() {
        container.align_items =
            mapper::infer_child_alignment(context, body_path, body_style, &body_path[1].children);
        body_children_centered = container.align_items.is_some();
    }
    if container.fill.is_none() {
        container.fill = Some(vec![crate::solid_fill("#ffffff")]);
    }
    let name = context
        .opts
        .document_name
        .clone()
        .or(title)
        .unwrap_or_else(|| "HTML Import".to_string());
    let mut base = PenNodeBase {
        id: root_id,
        name: Some(name),
        ..Default::default()
    };
    // The document root has no parent to reserve an un-shifted box in, so an
    // in-flow offset on `<body>` is reported rather than silently applied.
    let body_outcome = mapper::apply_base_style(&mut base, body_style, context);
    mapper::warn_dropped_flow_offset(context, body_outcome);
    if base
        .explain
        .as_deref()
        .is_some_and(|value| value.starts_with("__op_html_z_index:"))
    {
        base.explain = None;
    }
    if body_display_none {
        base.visible = Some(false);
    }
    let body_insets = mapper::box_insets(body_style, context);
    context.containing_width = root_width;
    context.containing_height = match container.height.as_ref() {
        Some(SizingBehavior::Number(value)) => *value,
        _ => context.opts.viewport_height(),
    };
    mapper::enter_box(context, body_style, body_insets);
    context.auto_margin_handled_by_parent = body_children_centered;
    let children = if body_display_none {
        Vec::new()
    } else {
        let mapped = mapper::map_container_children(
            context,
            body_path,
            body_style,
            Some(document_style),
            &body_path[1].children,
            text_fill_override.as_deref(),
        );
        let inline_padding = horizontal_padding(&container);
        mapper::apply_root_margins(
            &mut container,
            document_style,
            body_style,
            mapped.collapsed,
            context,
        );
        // The root has no parent to keep `<html>` / `<body>` margins in, so
        // they fold into its padding; wrapped rows must fit inside them too.
        let margins = horizontal_padding(&container) - inline_padding;
        context.containing_width = (context.containing_width - margins).max(0.0);
        mapper::apply_flex_wrap(context, body_style, &mut container, mapped.nodes)
    };
    PenNode::Frame(FrameNode {
        base,
        container,
        children: Some(children),
        image_search_query: None,
        reusable: None,
        slot: None,
        state: None,
        bindings: None,
        events: None,
        lifecycle: None,
        semantics: None,
        gestures: None,
        route: None,
        screen: None,
        breakpoint: None,
        placeholder: None,
    })
}

/// Left + right padding of a resolved container. `XY` is
/// `[vertical, horizontal]` and `LtrB` is `[top, right, bottom, left]`
/// (jian-core `layout/resolve.rs`).
fn horizontal_padding(container: &ContainerProps) -> f64 {
    match &container.padding {
        Some(Padding::Uniform(value)) => value * 2.0,
        Some(Padding::XY([_, horizontal])) => horizontal * 2.0,
        Some(Padding::LtrB([_, right, _, left])) => left + right,
        Some(Padding::Expression(_)) | None => 0.0,
    }
}
