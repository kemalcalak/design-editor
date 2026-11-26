//! Grid cells in `snapshot_layout` (#379).
//!
//! A `layout: grid` container's entry carries `grid: { columns: [{x,
//! width}], rows: [{y, height}] }` (the resolved tracks, in the entry's
//! coordinate space), and each child of it `cell: { column, row,
//! columnSpan, rowSpan }` (1-based lines, like `gridColumn` / `gridRow`).
//! The tracks are rebuilt from the laid-out scene by
//! `de_editor_core::grid_geometry`.

use std::collections::BTreeMap;

use de_editor_core::grid_geometry::{content_box, grid_geometry, GridCell, GridItem};
use jian_ops_schema::node::container::{ContainerProps, LayoutMode};
use jian_ops_schema::node::{GridPlacement, PenNode, ScriptNode};
use jian_scene::layout_scene::SceneNode;
use serde_json::{json, Value};

/// What a node contributes to grid reporting.
#[derive(Clone, Default)]
pub(crate) struct GridMeta {
    /// The container props of a grid container.
    pub container: Option<ContainerProps>,
    pub column: Option<GridPlacement>,
    pub row: Option<GridPlacement>,
}

/// Resolved tracks in document space: `(start, size)` per track.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GridTracks {
    pub columns: Vec<(f64, f64)>,
    pub rows: Vec<(f64, f64)>,
}

pub(crate) fn grid_meta(node: &PenNode) -> GridMeta {
    let container = match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => Some(&n.container),
        PenNode::Group(n) => Some(&n.container),
        PenNode::Rectangle(n) => Some(&n.container),
        _ => None,
    };
    let base = de_editor_core::pen_node_ext::PenNodeExt::base(node);
    GridMeta {
        container: container
            .filter(|c| c.layout == Some(LayoutMode::Grid))
            .cloned(),
        column: base.grid_column,
        row: base.grid_row,
    }
}

/// Tracks of `node` when it is a grid container, and the cell of each of
/// its children by id.
pub(crate) fn grid_of(
    node: &SceneNode,
    meta: &BTreeMap<String, GridMeta>,
) -> Option<(GridTracks, Vec<(String, GridCell)>)> {
    let container = meta.get(&node.id)?.container.as_ref()?;
    let b = node.bounds;
    let content = content_box(
        container,
        (
            f64::from(b.origin.x),
            f64::from(b.origin.y),
            f64::from(b.size.x),
            f64::from(b.size.y),
        ),
    );
    let items: Vec<GridItem> = node
        .children
        .iter()
        .map(|child| {
            let m = meta.get(&child.id).cloned().unwrap_or_default();
            GridItem {
                column: m.column,
                row: m.row,
                size: (
                    f64::from(child.bounds.size.x),
                    f64::from(child.bounds.size.y),
                ),
            }
        })
        .collect();
    // A laid-out frame always has a height, so `fr` rows can share it.
    let geometry = grid_geometry(container, (content.2, content.3), true, &items);
    let tracks = GridTracks {
        columns: geometry
            .columns
            .iter()
            .map(|t| (content.0 + t.start, t.size))
            .collect(),
        rows: geometry
            .rows
            .iter()
            .map(|t| (content.1 + t.start, t.size))
            .collect(),
    };
    let cells = node
        .children
        .iter()
        .map(|c| c.id.clone())
        .zip(geometry.cells)
        .collect();
    Some((tracks, cells))
}

fn round(v: f64) -> Value {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        json!(r as i64)
    } else {
        json!(r)
    }
}

/// The `grid` value of an entry, shifted by the query's `offset`.
pub(crate) fn tracks_json(t: &GridTracks, offset: (f64, f64)) -> Value {
    json!({
        "columns": t.columns.iter().map(|(x, w)| json!({"x": round(x - offset.0), "width": round(*w)})).collect::<Vec<_>>(),
        "rows": t.rows.iter().map(|(y, h)| json!({"y": round(y - offset.1), "height": round(*h)})).collect::<Vec<_>>(),
    })
}

/// The `cell` value of a grid child (1-based lines).
pub(crate) fn cell_json(c: &GridCell) -> Value {
    json!({
        "column": c.column + 1,
        "row": c.row + 1,
        "columnSpan": c.column_span,
        "rowSpan": c.row_span,
    })
}
