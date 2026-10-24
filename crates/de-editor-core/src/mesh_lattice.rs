//! Mesh gradient lattice edits from the Fill body (#142).
//!
//! A `MeshGradientBody` is a uniform `rows`×`cols` lattice, so vertices
//! come and go a whole row or column at a time. An add inserts the new
//! line in the middle gap of the axis with every vertex the midpoint of
//! its two neighbours; a remove drops the middle interior line, so a
//! remove undoes an add. The four corners never move, and every other
//! vertex keeps its colour. Each edit rewrites the stops as a complete
//! row-major grid: a vertex the body was missing becomes transparent,
//! which is how the renderers already paint a hole.

use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{MeshGradientBody, MeshVertexStop, PenFill};

use crate::fills::{node_fills, node_fills_mut};
use crate::state::EditorState;
use crate::walkers::find_node_mut;

/// Fewest lines per axis: the renderers need a 2×2 lattice at least.
pub const MESH_MIN_LINES: u32 = 2;
/// Most lines per axis the panel adds, keeping the vertex list short.
/// A larger lattice from a file or MCP still renders and can shrink.
pub const MESH_MAX_LINES: u32 = 5;
/// Largest lattice an edit will rebuild as a dense grid; a corrupt or
/// hostile file's huge `rows`×`cols` is left alone instead of
/// allocating billions of vertices.
const MESH_EDIT_MAX_VERTICES: u64 = 4096;

/// Colour a vertex missing from the body reads as (the renderers leave
/// it transparent).
const HOLE: &str = "#00000000";

/// One lattice axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshAxis {
    Rows,
    Columns,
}

/// One step of a lattice resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshLatticeEdit {
    Add(MeshAxis),
    Remove(MeshAxis),
}

/// `(rows, cols)` of the node's primary mesh fill, as the renderers read
/// them (at least 2 each). `None` when the primary fill is not a mesh.
pub fn primary_mesh_lattice(node: &PenNode) -> Option<(u32, u32)> {
    match node_fills(node)?.first()? {
        PenFill::MeshGradient(body) => {
            Some((body.rows.max(MESH_MIN_LINES), body.cols.max(MESH_MIN_LINES)))
        }
        _ => None,
    }
}

/// Apply `edit` to the node's primary mesh fill. `false` when the
/// primary fill is not a mesh or the axis is already at its limit.
pub fn set_primary_mesh_lattice(node: &mut PenNode, edit: MeshLatticeEdit) -> bool {
    match node_fills_mut(node).and_then(|fills| fills.first_mut()) {
        Some(PenFill::MeshGradient(body)) => apply_mesh_lattice_edit(body, edit),
        _ => false,
    }
}

/// Apply `edit` to `body`; `false` (body untouched) at the axis limit
/// or for a lattice too large to rebuild.
pub fn apply_mesh_lattice_edit(body: &mut MeshGradientBody, edit: MeshLatticeEdit) -> bool {
    if u64::from(body.rows) * u64::from(body.cols) > MESH_EDIT_MAX_VERTICES {
        return false;
    }
    let mut grid = vertex_grid(body);
    let axis = match edit {
        MeshLatticeEdit::Add(axis) | MeshLatticeEdit::Remove(axis) => axis,
    };
    if axis == MeshAxis::Columns {
        grid = transpose(grid);
    }
    let lines = grid.len() as u32;
    match edit {
        MeshLatticeEdit::Add(_) if lines < MESH_MAX_LINES => {
            // Middle gap: between lines `at - 1` and `at`.
            let at = grid.len() / 2;
            let line = grid[at - 1]
                .iter()
                .zip(&grid[at])
                .map(|(a, b)| midpoint_hex(a, b))
                .collect();
            grid.insert(at, line);
        }
        // The middle interior line; the inverse of the add above.
        MeshLatticeEdit::Remove(_) if lines > MESH_MIN_LINES => {
            grid.remove((grid.len() - 1) / 2);
        }
        _ => return false,
    }
    if axis == MeshAxis::Columns {
        grid = transpose(grid);
    }
    write_grid(body, grid);
    true
}

/// The body as a complete `rows`×`cols` grid of hex colours. Stops
/// outside the lattice are dropped; a repeated position keeps the last.
fn vertex_grid(body: &MeshGradientBody) -> Vec<Vec<String>> {
    let rows = body.rows.max(MESH_MIN_LINES) as usize;
    let cols = body.cols.max(MESH_MIN_LINES) as usize;
    let mut grid = vec![vec![HOLE.to_string(); cols]; rows];
    for stop in &body.stops {
        if let Some(cell) = grid
            .get_mut(stop.row as usize)
            .and_then(|line| line.get_mut(stop.col as usize))
        {
            *cell = stop.color.clone();
        }
    }
    grid
}

fn transpose(grid: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let cols = grid.first().map_or(0, Vec::len);
    let mut out = vec![Vec::with_capacity(grid.len()); cols];
    for line in grid {
        for (col, color) in line.into_iter().enumerate() {
            out[col].push(color);
        }
    }
    out
}

/// Store `grid` back as row-major stops.
fn write_grid(body: &mut MeshGradientBody, grid: Vec<Vec<String>>) {
    body.rows = grid.len() as u32;
    body.cols = grid.first().map_or(0, Vec::len) as u32;
    body.stops = grid
        .into_iter()
        .enumerate()
        .flat_map(|(row, line)| {
            line.into_iter()
                .enumerate()
                .map(move |(col, color)| MeshVertexStop {
                    row: row as u32,
                    col: col as u32,
                    color,
                })
        })
        .collect();
}

/// Per-channel midpoint of two hex colours. An unparseable colour reads
/// as transparent, as the renderers paint it; an opaque result drops the
/// alpha byte.
fn midpoint_hex(a: &str, b: &str) -> String {
    let parse = |hex: &str| {
        de_util::hex_color::parse_hex_rgba8(hex, de_util::hex_color::HexOptions::LENIENT)
            .unwrap_or([0; 4])
    };
    let (a, b) = (parse(a), parse(b));
    let mid: [u8; 4] = std::array::from_fn(|i| ((a[i] as u16 + b[i] as u16 + 1) / 2) as u8);
    if mid[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", mid[0], mid[1], mid[2])
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", mid[0], mid[1], mid[2], mid[3])
    }
}

impl EditorState {
    /// Resize the anchor node's primary mesh lattice by one line.
    /// Editable-gated; marks the document changed on a real change. The
    /// host records the undo step (`updates_document`).
    pub fn set_selected_mesh_lattice(&mut self, edit: MeshLatticeEdit) -> bool {
        let sel = self.selection.anchor.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return false;
        }
        let changed = find_node_mut(self.active_children_mut(), &sel)
            .is_some_and(|node| set_primary_mesh_lattice(node, edit));
        if changed {
            self.mark_document_changed();
        }
        changed
    }
}

#[cfg(test)]
#[path = "mesh_lattice_tests.rs"]
mod tests;
