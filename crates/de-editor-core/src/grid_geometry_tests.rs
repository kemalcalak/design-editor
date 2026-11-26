//! The rebuilt tracks must agree with where jian's taffy pass put the
//! children: each item's origin is its cell's corner.

use super::*;
use jian_core::document::NodeTree;
use jian_core::layout::LayoutEngine;
use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

/// The grid's container props, content box, items and each child's
/// laid-out `(x, y)`.
type LaidOut = (
    ContainerProps,
    (f64, f64, f64, f64),
    Vec<GridItem>,
    Vec<(f64, f64)>,
);

/// Lay `root` out with jian's engine.
fn layout(root: Value) -> LaidOut {
    let node: PenNode = serde_json::from_value(root).unwrap();
    let PenNode::Frame(frame) = &node else {
        panic!()
    };
    let container = frame.container.clone();
    let kids = frame.children.clone().unwrap_or_default();
    let mut tree = NodeTree::new();
    tree.insert_subtree(node.clone(), None);
    let mut engine = LayoutEngine::new();
    let roots = engine.build(&tree).unwrap();
    engine.compute(roots[0], (2000.0, 2000.0)).unwrap();
    let rect = |id: &str| {
        let r = engine.node_rect(tree.get(id).unwrap()).unwrap();
        (
            r.origin.x as f64,
            r.origin.y as f64,
            r.size.width as f64,
            r.size.height as f64,
        )
    };
    let bounds = rect(&frame.base.id);
    let content = content_box(&container, bounds);
    let mut items = Vec::new();
    let mut origins = Vec::new();
    for kid in &kids {
        let base = match kid {
            PenNode::Rectangle(r) => &r.base,
            PenNode::Frame(f) => &f.base,
            _ => panic!(),
        };
        let (x, y, w, h) = rect(&base.id);
        items.push(GridItem {
            column: base.grid_column,
            row: base.grid_row,
            size: (w, h),
        });
        origins.push((x, y));
    }
    (container, content, items, origins)
}

fn cell(id: &str, extra: Value) -> Value {
    let mut v = json!({"type":"rectangle","id":id,"width":"fill_container","height":40});
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    v
}

fn assert_cells_match(root: Value) -> GridGeometry {
    let (container, content, items, origins) = layout(root);
    let g = grid_geometry(&container, (content.2, content.3), true, &items);
    for ((cell, origin), item) in g.cells.iter().zip(&origins).zip(&items) {
        let x = content.0 + g.columns[cell.column].start;
        let y = content.1 + g.rows[cell.row].start;
        assert!(
            (x - origin.0).abs() < 0.5 && (y - origin.1).abs() < 0.5,
            "cell {cell:?} at ({x}, {y}) but laid out at {origin:?}; item {item:?}; {g:?}"
        );
    }
    g
}

#[test]
fn equal_columns_with_a_span_and_gaps() {
    let g = assert_cells_match(json!({
        "type":"frame","id":"g","width":340,"height":"fit_content","layout":"grid",
        "columns":3,"gap":20,"padding":10,
        "children":[cell("a", json!({"gridColumn":"span 2"})), cell("b", json!({})),
                    cell("c", json!({})), cell("d", json!({}))]
    }));
    assert_eq!(g.columns.len(), 3);
    assert_eq!(g.rows.len(), 2);
    assert!(
        (g.columns[1].start - (280.0 / 3.0 + 20.0)).abs() < 0.5,
        "{g:?}"
    );
    assert_eq!(
        g.cells[0],
        GridCell {
            column: 0,
            row: 0,
            column_span: 2,
            row_span: 1
        }
    );
    assert_eq!(g.cells[1].column, 2);
    assert_eq!((g.cells[2].row, g.cells[3].column), (1, 1));
}

#[test]
fn fixed_fr_auto_and_minmax_tracks() {
    let fixed = |id: &str, w: f64| json!({"type":"rectangle","id":id,"width":w,"height":30});
    assert_cells_match(json!({
        "type":"frame","id":"g","width":600,"height":"fit_content","layout":"grid",
        "columns":[120,"1fr","auto","minmax(80px, 2fr)"],"columnGap":10,"rowGap":6,
        "children":[fixed("a",50.0), fixed("b",50.0), fixed("c",70.0), fixed("d",50.0),
                    fixed("e",50.0)]
    }));
}

#[test]
fn pinned_lines_and_rows() {
    let g = assert_cells_match(json!({
        "type":"frame","id":"g","width":300,"height":200,"layout":"grid",
        "columns":3,"rows":[50,"1fr"],
        "children":[cell("a", json!({"gridColumn":3,"gridRow":2})), cell("b", json!({})),
                    cell("c", json!({"gridColumn":2}))]
    }));
    assert_eq!(
        (g.cells[0].column, g.cells[0].row),
        (2, 1),
        "definite row placed first"
    );
    assert_eq!((g.cells[1].column, g.cells[1].row), (0, 0));
    assert_eq!((g.cells[2].column, g.cells[2].row), (1, 0));
    // The fr row takes the rest of the fixed 200 px height.
    assert!((g.rows[1].size - 150.0).abs() < 0.5, "{g:?}");
}

#[test]
fn content_box_reads_css_padding_order() {
    let c = ContainerProps {
        padding: Some(Padding::LtrB([1.0, 2.0, 3.0, 4.0])),
        ..Default::default()
    };
    assert_eq!(
        content_box(&c, (10.0, 20.0, 100.0, 50.0)),
        (14.0, 21.0, 94.0, 46.0)
    );
}
