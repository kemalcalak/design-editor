//! Figma wrap and grid auto layout → `wrap` / `layout: grid` (#379).
//!
//! Wrap: `stackWrap: WRAP` on a horizontal stack, with
//! `stackCounterSpacing` as the space between lines (`rowGap`).
//!
//! Grid (`stackMode: GRID`, Figma's 2026 grid auto layout): the track
//! counts `gridColumnCount` / `gridRowCount`, the gaps `gridColumnGap` /
//! `gridRowGap`, and per-track sizes in `gridColumnsSizing` /
//! `gridRowsSizing` (`{ type: FIXED | FLEX | HUG, value }`) when the file
//! has them, else equal `1fr` tracks. A child's cell is its 0-based
//! `gridColumnAnchorIndex` / `gridRowAnchorIndex` plus `gridColumnSpan` /
//! `gridRowSpan`. These names follow the plugin API's grid fields; a file
//! whose schema names them differently keeps the plain vertical stack.

use jian_ops_schema::node::grid::{GridPlacement, GridTemplate, GridTrack, TrackSize};

use crate::kiwi::FigValue;

/// Wrap / grid fields read off one Figma frame.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct WrapGrid {
    pub wrap: bool,
    pub row_gap: Option<f64>,
    pub column_gap: Option<f64>,
    pub columns: Option<GridTemplate>,
    pub rows: Option<GridTemplate>,
}

pub fn is_grid(node: &FigValue) -> bool {
    node.get_str("stackMode") == Some("GRID")
}

pub fn map_wrap_grid(node: &FigValue) -> WrapGrid {
    let mut out = WrapGrid::default();
    if is_grid(node) {
        out.columns = template(node, "gridColumnsSizing", "gridColumnCount");
        out.rows = template(node, "gridRowsSizing", "gridRowCount");
        out.column_gap = node.get_f64("gridColumnGap").filter(|g| *g > 0.0);
        out.row_gap = node.get_f64("gridRowGap").filter(|g| *g > 0.0);
        return out;
    }
    if node.get_str("stackMode") == Some("HORIZONTAL") && node.get_str("stackWrap") == Some("WRAP")
    {
        out.wrap = true;
        out.row_gap = node.get_f64("stackCounterSpacing");
    }
    out
}

fn template(node: &FigValue, sizing: &str, count: &str) -> Option<GridTemplate> {
    let tracks: Option<Vec<GridTrack>> = node
        .get(sizing)
        .and_then(FigValue::as_array)
        .filter(|a| !a.is_empty())
        .map(|a| a.iter().map(track).collect());
    if let Some(tracks) = tracks {
        return Some(GridTemplate::Tracks(tracks));
    }
    let n = node.get_f64(count)?;
    (n >= 1.0).then(|| GridTemplate::Count((n as u32).min(GridTemplate::MAX_TRACKS)))
}

fn track(v: &FigValue) -> GridTrack {
    let value = v.get_f64("value").filter(|n| n.is_finite() && *n > 0.0);
    match v.get_str("type") {
        Some("FIXED") => GridTrack::Size(TrackSize::Px(value.unwrap_or(0.0))),
        Some("HUG") => GridTrack::Size(TrackSize::Auto),
        _ => GridTrack::Size(TrackSize::Fr(value.unwrap_or(1.0))),
    }
}

/// A grid child's `(gridColumn, gridRow)`.
pub fn child_placement(node: &FigValue) -> (Option<GridPlacement>, Option<GridPlacement>) {
    let axis = |anchor: &str, span: &str| {
        let start = node
            .get_f64(anchor)
            .filter(|i| *i >= 0.0)
            .map(|i| (i as u16).saturating_add(1));
        let span = node
            .get_f64(span)
            .filter(|s| *s >= 1.0)
            .map_or(1, |s| (s as u16).min(GridPlacement::MAX_SPAN));
        (start.is_some() || span > 1).then_some(GridPlacement { start, span })
    };
    (
        axis("gridColumnAnchorIndex", "gridColumnSpan"),
        axis("gridRowAnchorIndex", "gridRowSpan"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn obj(pairs: Vec<(&str, FigValue)>) -> FigValue {
        FigValue::Object(pairs.into_iter().map(|(k, v)| (Arc::from(k), v)).collect())
    }

    #[test]
    fn wrapping_stack() {
        let w = map_wrap_grid(&obj(vec![
            ("stackMode", FigValue::Str("HORIZONTAL".into())),
            ("stackWrap", FigValue::Str("WRAP".into())),
            ("stackCounterSpacing", FigValue::Float(12.0)),
        ]));
        assert!(w.wrap);
        assert_eq!(w.row_gap, Some(12.0));
        let plain = map_wrap_grid(&obj(vec![(
            "stackMode",
            FigValue::Str("HORIZONTAL".into()),
        )]));
        assert!(!plain.wrap);
    }

    #[test]
    fn grid_counts_sizes_and_cells() {
        let g = map_wrap_grid(&obj(vec![
            ("stackMode", FigValue::Str("GRID".into())),
            ("gridColumnCount", FigValue::Uint(3)),
            ("gridRowCount", FigValue::Uint(2)),
            ("gridColumnGap", FigValue::Float(16.0)),
            (
                "gridRowsSizing",
                FigValue::Array(vec![
                    obj(vec![
                        ("type", FigValue::Str("FIXED".into())),
                        ("value", FigValue::Float(80.0)),
                    ]),
                    obj(vec![("type", FigValue::Str("HUG".into()))]),
                ]),
            ),
        ]));
        assert_eq!(g.columns, Some(GridTemplate::Count(3)));
        assert_eq!(
            g.rows.as_ref().map(|t| t.short_text()).as_deref(),
            Some("80 auto")
        );
        assert_eq!(g.column_gap, Some(16.0));
        let (column, row) = child_placement(&obj(vec![
            ("gridColumnAnchorIndex", FigValue::Uint(1)),
            ("gridColumnSpan", FigValue::Uint(2)),
        ]));
        assert_eq!(
            column,
            Some(GridPlacement {
                start: Some(2),
                span: 2
            })
        );
        assert_eq!(row, None);
    }
}
