//! The native grid pass of the HTML importer (#379): a CSS grid container
//! becomes `layout: grid` with its tracks, gaps and item placement. The
//! column count and track resolution come from `mapper_grid`.

use crate::css::cascade::ComputedStyle;
use crate::import_warning::ImportWarning;
use jian_ops_schema::node::base::NumberOrExpression;
use jian_ops_schema::node::container::{AlignItems, ContainerProps, JustifyContent, LayoutMode};
use jian_ops_schema::node::grid::{GridPlacement, GridTemplate, GridTrack, TrackSize};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

use super::super::grid_place as place;
use super::super::node_access::{node_base_mut, node_width, node_width_mut};
use super::super::MapCtx;
use super::{
    effective_column_count, expand_fixed_tracks, gap_value, is_grid, is_grid_overlay,
    resolved_track_widths,
};

/// Turn a CSS grid container into native grid auto layout (#379): the
/// container takes `layout: grid`, its column tracks and gaps, and each
/// in-flow child keeps its place through `gridColumn` (a span, plus the
/// start line when the author pinned one). Rows flow automatically, as in
/// the browser.
///
/// Placement carriers are stripped on every path. A grid whose columns
/// cannot be counted, or that has one column, keeps the vertical stack the
/// mapper gave it. Children stay in their incoming order: the stacking pass
/// that ran before set their paint order.
pub(crate) fn apply_grid(
    context: &mut MapCtx<'_>,
    style: &ComputedStyle,
    container: &mut ContainerProps,
    mut children: Vec<PenNode>,
) -> Vec<PenNode> {
    let placements = place::take_placements(&mut children);
    if children.is_empty() || !is_grid(style) {
        return children;
    }
    let flow: Vec<usize> = (0..children.len())
        .filter(|index| !is_grid_overlay(&children[*index]))
        .collect();
    if flow.is_empty() {
        return children;
    }
    let Some(columns) = effective_column_count(style, context, flow.len()) else {
        return children;
    };
    if columns <= 1 {
        return children;
    }
    let flow_placements: Vec<_> = flow.iter().map(|index| placements[*index]).collect();
    let (packed, notes) = place::pack_rows(&flow_placements, columns);
    if notes.reflowed_start {
        context.warn_once(ImportWarning::GridSpanReflowed);
    }
    let column_gap = gap_value(style, context, "column-gap").unwrap_or(0.0);
    container.layout = Some(LayoutMode::Grid);
    container.columns = Some(grid_template(style, context, columns, column_gap));
    // `gap` carried the row gap; split it so each axis keeps its own.
    container.row_gap = container.gap.take();
    container.column_gap = (column_gap > 0.0).then_some(NumberOrExpression::Number(column_gap));
    // Items align in their cells: `justify-items` across, `align-items`
    // down; `justify-content` is track distribution, which has no home.
    let justify_items = style.get("justify-items").map(str::trim);
    container.justify_content = match justify_items {
        Some("center") => Some(JustifyContent::Center),
        Some("end" | "flex-end" | "right") => Some(JustifyContent::End),
        _ => None,
    };
    container.align_items = style
        .get("align-items")
        .and_then(AlignItems::from_css)
        .or(Some(AlignItems::Stretch));
    let stretch = !matches!(
        justify_items,
        Some("start" | "flex-start" | "left" | "center" | "end" | "flex-end" | "right")
    );
    let cells = packed.iter().flatten();
    for ((index, placement), cell) in flow.iter().zip(&flow_placements).zip(cells) {
        let child = &mut children[*index];
        if cell.span > 1 || placement.column_start.is_some() {
            node_base_mut(child).grid_column = Some(GridPlacement {
                start: placement
                    .column_start
                    .map(|_| (cell.start + 1).min(u16::MAX as usize) as u16),
                span: cell.span.min(GridPlacement::MAX_SPAN as usize) as u16,
            });
        }
        // CSS stretches an item without a width of its own across its cell.
        if stretch && !matches!(node_width(child), Some(SizingBehavior::Number(_))) {
            if let Some(width) = node_width_mut(child) {
                *width = Some(SizingBehavior::Keyword(SizingKeyword::FillContainer));
            }
        }
    }
    children
}

/// The column tracks as the schema stores them: each CSS track when it
/// reads as one (px, `fr`, `auto`, `minmax(px, fr)`), else its width
/// resolved against the container, else equal `1fr` columns.
fn grid_template(
    style: &ComputedStyle,
    context: &MapCtx<'_>,
    columns: usize,
    gap: f64,
) -> GridTemplate {
    let direct = style
        .get("grid-template-columns")
        .and_then(|value| expand_fixed_tracks(value, 0))
        .filter(|tracks| tracks.len() == columns)
        .and_then(|tracks| {
            tracks
                .iter()
                .map(|track| GridTrack::parse(track))
                .collect::<Option<Vec<_>>>()
        });
    if let Some(tracks) = direct {
        return GridTemplate::Tracks(tracks);
    }
    if let Some(widths) = resolved_track_widths(style, context, columns, gap) {
        return GridTemplate::Tracks(
            widths
                .into_iter()
                .map(|w| GridTrack::Size(TrackSize::Px((w * 100.0).round() / 100.0)))
                .collect(),
        );
    }
    GridTemplate::Count(columns.min(GridTemplate::MAX_TRACKS as usize) as u32)
}
