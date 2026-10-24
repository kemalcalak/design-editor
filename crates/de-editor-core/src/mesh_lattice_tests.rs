use super::*;
use crate::node_id::NodeId;
use crate::test_support::{rect, state_with};
use crate::walkers::find_node;
use crate::FillType;

use MeshAxis::{Columns, Rows};
use MeshLatticeEdit::{Add, Remove};

fn body(rows: u32, cols: u32, colors: &[&str]) -> MeshGradientBody {
    MeshGradientBody {
        rows,
        cols,
        stops: colors
            .iter()
            .enumerate()
            .map(|(i, color)| MeshVertexStop {
                row: i as u32 / cols,
                col: i as u32 % cols,
                color: color.to_string(),
            })
            .collect(),
        explain: None,
        opacity: Some(0.5),
        blend_mode: None,
    }
}

fn colors(body: &MeshGradientBody) -> Vec<&str> {
    body.stops.iter().map(|s| s.color.as_str()).collect()
}

/// Every stop sits at its row-major position of a full lattice.
fn assert_row_major(body: &MeshGradientBody) {
    assert_eq!(body.stops.len(), (body.rows * body.cols) as usize);
    for (i, stop) in body.stops.iter().enumerate() {
        assert_eq!(
            (stop.row, stop.col),
            (i as u32 / body.cols, i as u32 % body.cols)
        );
    }
}

#[test]
fn adding_a_row_inserts_the_neighbour_midpoints() {
    let mut b = body(2, 2, &["#000000", "#ffffff", "#ff0000", "#0000ff"]);
    assert!(apply_mesh_lattice_edit(&mut b, Add(Rows)));
    assert_eq!((b.rows, b.cols), (3, 2));
    assert_row_major(&b);
    assert_eq!(
        colors(&b),
        ["#000000", "#ffffff", "#800000", "#8080ff", "#ff0000", "#0000ff"]
    );
    assert_eq!(b.opacity, Some(0.5), "the shared tail stays");
}

#[test]
fn adding_a_column_inserts_the_neighbour_midpoints() {
    let mut b = body(2, 2, &["#000000", "#ffffff", "#ff0000", "#0000ff"]);
    assert!(apply_mesh_lattice_edit(&mut b, Add(Columns)));
    assert_eq!((b.rows, b.cols), (2, 3));
    assert_row_major(&b);
    assert_eq!(
        colors(&b),
        ["#000000", "#808080", "#ffffff", "#ff0000", "#800080", "#0000ff"]
    );
}

#[test]
fn remove_undoes_add_and_keeps_the_corners() {
    let original = body(2, 2, &["#112233", "#445566", "#778899", "#aabbcc"]);
    let mut b = original.clone();
    for _ in 0..3 {
        assert!(apply_mesh_lattice_edit(&mut b, Add(Rows)));
        assert!(apply_mesh_lattice_edit(&mut b, Add(Columns)));
    }
    assert_eq!((b.rows, b.cols), (5, 5));
    let corners = |b: &MeshGradientBody| {
        let last = (b.rows * b.cols) as usize - 1;
        let c = b.cols as usize;
        [0, c - 1, last + 1 - c, last].map(|i| b.stops[i].color.clone())
    };
    assert_eq!(corners(&b), corners(&original));
    for _ in 0..3 {
        assert!(apply_mesh_lattice_edit(&mut b, Remove(Columns)));
        assert!(apply_mesh_lattice_edit(&mut b, Remove(Rows)));
    }
    assert_eq!(b, original);
}

#[test]
fn limits_reject_without_touching_the_body() {
    let mut b = body(2, 2, &["#000000", "#ffffff", "#ff0000", "#0000ff"]);
    let before = b.clone();
    assert!(!apply_mesh_lattice_edit(&mut b, Remove(Rows)));
    assert!(!apply_mesh_lattice_edit(&mut b, Remove(Columns)));
    assert_eq!(b, before);

    let mut wide = body(2, MESH_MAX_LINES, &["#000000"; 2 * MESH_MAX_LINES as usize]);
    let before = wide.clone();
    assert!(!apply_mesh_lattice_edit(&mut wide, Add(Columns)));
    assert_eq!(wide, before);
    assert!(apply_mesh_lattice_edit(&mut wide, Remove(Columns)));

    // A corrupt file's huge lattice is refused, never densified.
    let mut huge = body(100_000, 100_000, &[]);
    let before = huge.clone();
    assert!(!apply_mesh_lattice_edit(&mut huge, Remove(Rows)));
    assert_eq!(huge, before);
}

#[test]
fn a_sparse_body_becomes_a_full_row_major_grid() {
    // Out of order, one vertex missing, one outside the lattice.
    let mut b = MeshGradientBody {
        rows: 2,
        cols: 2,
        stops: vec![
            MeshVertexStop {
                row: 1,
                col: 1,
                color: "#0000ff".into(),
            },
            MeshVertexStop {
                row: 0,
                col: 0,
                color: "#ff0000".into(),
            },
            MeshVertexStop {
                row: 0,
                col: 1,
                color: "#00ff00".into(),
            },
            MeshVertexStop {
                row: 7,
                col: 0,
                color: "#ffffff".into(),
            },
        ],
        explain: None,
        opacity: None,
        blend_mode: None,
    };
    assert!(apply_mesh_lattice_edit(&mut b, Add(Rows)));
    assert_row_major(&b);
    assert_eq!(
        colors(&b),
        [
            "#ff0000",
            "#00ff00",
            "#80000080",
            "#008080",
            HOLE,
            "#0000ff"
        ]
    );
}

#[test]
fn selected_edit_reaches_the_primary_mesh_only() {
    let mut node = rect("a", "A", 0.0, 0.0, 40.0, 40.0);
    let mut s = state_with(vec![node.clone()]);
    s.set_single_selection(NodeId::new("a"));
    assert!(!s.set_selected_mesh_lattice(Add(Rows)), "solid fill");

    assert!(crate::fills::set_primary_fill_type(
        &mut node,
        FillType::MeshGradient
    ));
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("a"));
    let lattice = |s: &EditorState| {
        primary_mesh_lattice(find_node(s.active_children(), &NodeId::new("a")).unwrap())
    };
    assert_eq!(lattice(&s), Some((2, 2)));
    assert!(s.set_selected_mesh_lattice(Add(Columns)));
    assert_eq!(lattice(&s), Some((2, 3)));
}
