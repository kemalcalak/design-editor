//! Track and cell geometry of a laid-out `layout: grid` container (#379).
//!
//! Taffy 0.5 does not report a grid's resolved tracks, so the canvas
//! overlay and MCP `snapshot_layout` rebuild them from what layout already
//! decided: the container's content box and each child's laid-out size.
//! Placement follows CSS grid auto-placement (`grid-auto-flow: row`,
//! sparse); track sizing follows the CSS rules for the track kinds the
//! schema has (px, `fr`, `auto`, `minmax(px | auto, px | fr | auto)`),
//! with single-track items as the content contributions. That is exact for
//! the cases the editor builds (fixed and `fr` columns, auto rows) and a
//! close reading of the rest.

use jian_ops_schema::node::container::{ContainerProps, Padding};
use jian_ops_schema::node::grid::{GridPlacement, GridTrack, TrackSize};
use jian_ops_schema::node::NumberOrExpression;

/// One grid child as layout left it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridItem {
    pub column: Option<GridPlacement>,
    pub row: Option<GridPlacement>,
    /// Laid-out `(width, height)`.
    pub size: (f64, f64),
}

/// A resolved track: offset from the content box's edge, and its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    pub start: f64,
    pub size: f64,
}

/// Where an item sits, 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCell {
    pub column: usize,
    pub row: usize,
    pub column_span: usize,
    pub row_span: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridGeometry {
    pub columns: Vec<Track>,
    pub rows: Vec<Track>,
    /// One per item, in item order.
    pub cells: Vec<GridCell>,
}

/// The content box `(x, y, w, h)` inside `bounds`, CSS padding order.
pub fn content_box(c: &ContainerProps, bounds: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let [t, r, b, l] = match c.padding {
        Some(Padding::Uniform(v)) => [v; 4],
        Some(Padding::XY([v, h])) => [v, h, v, h],
        Some(Padding::LtrB(p)) => p,
        _ => [0.0; 4],
    };
    let (x, y, w, h) = bounds;
    (x + l, y + t, (w - l - r).max(0.0), (h - t - b).max(0.0))
}

fn number(v: Option<&NumberOrExpression>) -> f64 {
    match v {
        Some(NumberOrExpression::Number(n)) if n.is_finite() => n.max(0.0),
        _ => 0.0,
    }
}

/// Resolve a grid's tracks and cells. `content` is the content box size;
/// `definite_height` says whether rows may share a fixed height.
pub fn grid_geometry(
    c: &ContainerProps,
    content: (f64, f64),
    definite_height: bool,
    items: &[GridItem],
) -> GridGeometry {
    let explicit_columns = c.columns.as_ref().map(|t| t.tracks()).unwrap_or_default();
    let explicit_rows = c.rows.as_ref().map(|t| t.tracks()).unwrap_or_default();
    let column_count = explicit_columns
        .len()
        .max(
            items
                .iter()
                .filter_map(|i| {
                    i.column
                        .and_then(|p| p.start)
                        .map(|s| s as usize - 1 + p_span(i.column))
                })
                .max()
                .unwrap_or(0),
        )
        .max(1);
    let cells = place(items, column_count);
    let row_count = cells
        .iter()
        .map(|c| c.row + c.row_span)
        .max()
        .unwrap_or(0)
        .max(explicit_rows.len());
    let contributions = |columns: bool, count: usize| {
        let mut out = vec![Vec::new(); count];
        for (cell, item) in cells.iter().zip(items) {
            let (index, span, size) = if columns {
                (cell.column, cell.column_span, item.size.0)
            } else {
                (cell.row, cell.row_span, item.size.1)
            };
            if span == 1 && index < count {
                out[index].push(size);
            }
        }
        out
    };
    let column_gap = number(c.column_gap_value());
    let row_gap = number(c.row_gap_value());
    let column_sizes = size_tracks(
        &explicit_columns,
        column_count,
        Some(content.0),
        column_gap,
        &contributions(true, column_count),
    );
    let row_sizes = size_tracks(
        &explicit_rows,
        row_count,
        definite_height.then_some(content.1),
        row_gap,
        &contributions(false, row_count),
    );
    GridGeometry {
        columns: positions(&column_sizes, column_gap),
        rows: positions(&row_sizes, row_gap),
        cells,
    }
}

fn p_span(p: Option<GridPlacement>) -> usize {
    p.map_or(1, |p| p.span.max(1) as usize)
}

fn positions(sizes: &[f64], gap: f64) -> Vec<Track> {
    let mut at = 0.0;
    sizes
        .iter()
        .map(|size| {
            let track = Track {
                start: at,
                size: *size,
            };
            at += size + gap;
            track
        })
        .collect()
}

/// CSS auto-placement, `grid-auto-flow: row` (sparse).
fn place(items: &[GridItem], columns: usize) -> Vec<GridCell> {
    let mut taken: Vec<Vec<bool>> = Vec::new();
    let free = |taken: &Vec<Vec<bool>>, row: usize, col: usize, rs: usize, cs: usize| {
        (row..row + rs).all(|r| {
            (col..col + cs).all(|c| taken.get(r).and_then(|line| line.get(c)) != Some(&true))
        })
    };
    let mark = |taken: &mut Vec<Vec<bool>>, cell: GridCell| {
        for r in cell.row..cell.row + cell.row_span {
            if taken.len() <= r {
                taken.resize(r + 1, vec![false; columns]);
            }
            let end = (cell.column + cell.column_span).min(columns);
            for slot in taken[r].iter_mut().take(end).skip(cell.column) {
                *slot = true;
            }
        }
    };
    let spans = |item: &GridItem| {
        let cs = p_span(item.column).min(columns);
        (cs, p_span(item.row))
    };
    let start = |p: Option<GridPlacement>| p.and_then(|p| p.start).map(|s| s as usize - 1);
    let mut cells: Vec<Option<GridCell>> = vec![None; items.len()];
    // 1. Items with a definite row: their column too, or the first that fits.
    for (index, item) in items.iter().enumerate() {
        let Some(row) = start(item.row) else {
            continue;
        };
        let (cs, rs) = spans(item);
        let column = start(item.column)
            .map(|c| c.min(columns - cs))
            .unwrap_or_else(|| {
                (0..=columns - cs)
                    .find(|c| free(&taken, row, *c, rs, cs))
                    .unwrap_or(0)
            });
        let cell = GridCell {
            column,
            row,
            column_span: cs,
            row_span: rs,
        };
        mark(&mut taken, cell);
        cells[index] = Some(cell);
    }
    // 2. The rest, in order, from a cursor that only moves forward.
    let (mut cursor_row, mut cursor_col) = (0usize, 0usize);
    for (index, item) in items.iter().enumerate() {
        if cells[index].is_some() {
            continue;
        }
        let (cs, rs) = spans(item);
        let cell = match start(item.column) {
            Some(column) => {
                let column = column.min(columns - cs);
                if column < cursor_col {
                    cursor_row += 1;
                }
                while !free(&taken, cursor_row, column, rs, cs) {
                    cursor_row += 1;
                }
                GridCell {
                    column,
                    row: cursor_row,
                    column_span: cs,
                    row_span: rs,
                }
            }
            None => loop {
                if cursor_col + cs > columns {
                    cursor_row += 1;
                    cursor_col = 0;
                    continue;
                }
                if free(&taken, cursor_row, cursor_col, rs, cs) {
                    break GridCell {
                        column: cursor_col,
                        row: cursor_row,
                        column_span: cs,
                        row_span: rs,
                    };
                }
                cursor_col += 1;
            },
        };
        cursor_col = cell.column + cs;
        mark(&mut taken, cell);
        cells[index] = Some(cell);
    }
    cells
        .into_iter()
        .map(|c| c.expect("every item placed"))
        .collect()
}

fn content_max(contributions: &[f64]) -> f64 {
    contributions.iter().copied().fold(0.0, f64::max)
}

/// CSS track sizing for one axis. Tracks past the explicit list are `auto`.
fn size_tracks(
    explicit: &[GridTrack],
    count: usize,
    available: Option<f64>,
    gap: f64,
    contributions: &[Vec<f64>],
) -> Vec<f64> {
    let track = |i: usize| {
        explicit
            .get(i)
            .copied()
            .unwrap_or(GridTrack::Size(TrackSize::Auto))
    };
    let min_of = |s: TrackSize, i: usize| match s {
        TrackSize::Px(v) => v,
        _ => content_max(&contributions[i]),
    };
    let mut sizes = vec![0.0; count];
    let mut flex: Vec<(usize, f64, f64)> = Vec::new(); // (index, weight, floor)
    let mut stretchable = Vec::new();
    for (i, size) in sizes.iter_mut().enumerate() {
        match track(i) {
            GridTrack::Size(TrackSize::Px(v)) => *size = v,
            GridTrack::Size(TrackSize::Auto) => {
                *size = content_max(&contributions[i]);
                stretchable.push(i);
            }
            GridTrack::Size(TrackSize::Fr(w)) => flex.push((i, w, content_max(&contributions[i]))),
            GridTrack::MinMax {
                min,
                max: TrackSize::Fr(w),
            } => flex.push((i, w, min_of(min, i))),
            GridTrack::MinMax {
                min,
                max: TrackSize::Px(max),
            } => {
                let floor = min_of(min, i);
                *size = if available.is_some() {
                    max.max(floor)
                } else {
                    content_max(&contributions[i]).clamp(floor, max.max(floor))
                };
            }
            GridTrack::MinMax {
                min,
                max: TrackSize::Auto,
            } => {
                *size = min_of(min, i).max(content_max(&contributions[i]));
                stretchable.push(i);
            }
        }
    }
    let gaps = gap * count.saturating_sub(1) as f64;
    let fixed: f64 = sizes.iter().sum();
    let Some(available) = available else {
        for (i, _, floor) in flex {
            sizes[i] = floor;
        }
        return sizes;
    };
    let mut free = (available - gaps - fixed).max(0.0);
    if flex.is_empty() {
        // `normal` content alignment stretches auto tracks into the rest.
        if !stretchable.is_empty() && free > 0.0 {
            let share = free / stretchable.len() as f64;
            for i in stretchable {
                sizes[i] += share;
            }
        }
        return sizes;
    }
    // Freeze flexible tracks whose share would fall under their floor.
    let mut open = flex;
    loop {
        let weight: f64 = open.iter().map(|(_, w, _)| w).sum();
        let per = if weight > 0.0 { free / weight } else { 0.0 };
        let (frozen, rest): (Vec<_>, Vec<_>) =
            // 1 px of slack: laid-out sizes are rounded to whole pixels, so
            // a stretched item can read a hair over its fair share.
            open.iter().partition(|(_, w, floor)| per * w + 1.0 < *floor);
        if frozen.is_empty() {
            for (i, w, _) in rest {
                sizes[i] = per * w;
            }
            return sizes;
        }
        for (i, _, floor) in &frozen {
            sizes[*i] = *floor;
            free = (free - floor).max(0.0);
        }
        open = rest;
        if open.is_empty() {
            return sizes;
        }
    }
}

#[cfg(test)]
#[path = "grid_geometry_tests.rs"]
mod tests;
