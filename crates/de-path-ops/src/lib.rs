//! Vector path operations on skia (#388): curve-preserving booleans,
//! outline stroke, offset path and flatten.
//!
//! Everything reads the layout-resolved [`LayoutScene`] (document space)
//! and hands back skia paths, which [`convert`] turns into the editor's
//! editable [`Subpath`]s without sampling: curves stay curves. The
//! scene builder resolves boolean groups through [`boolean_of_children`];
//! the desktop canvas and the MCP tools commit results through
//! `de_editor_core`'s path commands. Simplify and reverse need no skia and
//! live in `de_editor_core::vector_edit`.

pub mod apply;
pub mod convert;
pub mod outline;
pub mod plan;
pub mod stroke;
#[cfg(test)]
mod test_raster;

pub use apply::apply_path_operation;
pub use convert::{cubic_segment_count, path_to_subpaths, subpaths_to_path};
pub use outline::{fit_path_to_rect, node_geometry, node_outline, union_all, Geometry};
pub use plan::{plan_boolean, plan_flatten, plan_offset, plan_outline_stroke, Planned};
pub use stroke::{offset_node, offset_path, outline_stroke, stroke_to_fill, OffsetJoin};

use de_editor_core::vector_edit::Subpath;
use jian_ops_schema::node::BooleanOperation;
use jian_scene::layout_scene::{LayoutScene, NodeKind, SceneNode};
use skia_safe::{Path, PathFillType, PathOp};

/// skia's operator for a boolean.
pub fn path_op(op: BooleanOperation) -> PathOp {
    match op {
        BooleanOperation::Union => PathOp::Union,
        BooleanOperation::Subtract => PathOp::Difference,
        BooleanOperation::Intersect => PathOp::Intersect,
        BooleanOperation::Exclude => PathOp::XOR,
    }
}

/// Fold `op` over `paths`, first path first. A single path is returned
/// as is; `None` for none or when skia gives up.
pub fn combine(paths: &[Path], op: BooleanOperation) -> Option<Path> {
    let (first, rest) = paths.split_first()?;
    let op = path_op(op);
    rest.iter().try_fold(first.clone(), |acc, p| acc.op(p, op))
}

/// The boolean of a boolean group's children (doc space): what the group
/// paints, hit-tests and exports. Children without an area are skipped.
pub fn boolean_of_children(op: BooleanOperation, children: &[SceneNode]) -> Option<Path> {
    let outlines: Vec<Path> = children.iter().filter_map(node_outline).collect();
    combine(&outlines, op).filter(|p| !p.is_empty())
}

/// A computed path ready to commit: editable subpaths in doc space plus
/// the fill rule.
#[derive(Debug, Clone, PartialEq)]
pub struct PathResult {
    pub subpaths: Vec<Subpath>,
    pub even_odd: bool,
}

impl PathResult {
    /// Convert a skia path; `None` when it has no contour.
    pub fn from_path(path: &Path) -> Option<Self> {
        let subpaths = path_to_subpaths(path);
        (!subpaths.is_empty()).then(|| Self {
            subpaths,
            even_odd: matches!(
                path.fill_type(),
                PathFillType::EvenOdd | PathFillType::InverseEvenOdd
            ),
        })
    }
}

/// The local `d` and doc-space box of a computed path, as a path node
/// stores them: `d` relative to the box origin.
pub fn local_path_data(path: &Path) -> Option<(String, [f32; 4])> {
    let b = path.compute_tight_bounds();
    if path.is_empty() || !b.is_finite() {
        return None;
    }
    let mut m = skia_safe::Matrix::new_identity();
    m.set_translate((-b.left(), -b.top()));
    let local = path.with_transform(&m);
    let d = de_editor_core::vector_edit::subpaths_to_path_data(&path_to_subpaths(&local));
    (!d.is_empty()).then_some((d, [b.left(), b.top(), b.width(), b.height()]))
}

/// Boolean of the active page's nodes `ids` (scene ids), first id first.
/// `None` when an id is missing, has no area, fewer than two remain, or
/// the result is empty.
pub fn compute_boolean(scene: &LayoutScene, ids: &[String], op: BooleanOperation) -> Option<Path> {
    let page = scene.active_page()?;
    let mut outlines = Vec::with_capacity(ids.len());
    for id in ids {
        outlines.push(node_outline(page.find(id)?)?);
    }
    if outlines.len() < 2 {
        return None;
    }
    combine(&outlines, op).filter(|p| !p.is_empty())
}

/// One path covering every node in `ids` (the active page's): the union
/// of their outlines, or a single node's own geometry, curves kept.
pub fn flatten_nodes(scene: &LayoutScene, ids: &[String]) -> Option<Path> {
    let page = scene.active_page()?;
    let mut outlines = Vec::with_capacity(ids.len());
    for id in ids {
        outlines.push(node_outline(page.find(id)?)?);
    }
    if let [single] = outlines.as_slice() {
        return (!single.is_empty()).then(|| single.clone());
    }
    union_all(&outlines).filter(|p| !p.is_empty())
}

/// Document-space origin of the frame `id`'s `x` / `y` are measured in:
/// the box of the nearest ancestor that positions its children (groups
/// and boolean groups do not), `(0, 0)` at the page root.
pub fn layout_origin(scene: &LayoutScene, id: &str) -> (f64, f64) {
    fn walk(nodes: &[SceneNode], id: &str, origin: (f64, f64)) -> Option<(f64, f64)> {
        for n in nodes {
            if n.id == id {
                return Some(origin);
            }
            let positions = !matches!(n.kind, NodeKind::Group | NodeKind::Path);
            let inner = if positions {
                (f64::from(n.bounds.origin.x), f64::from(n.bounds.origin.y))
            } else {
                origin
            };
            if let Some(found) = walk(&n.children, id, inner) {
                return Some(found);
            }
        }
        None
    }
    scene
        .active_page()
        .and_then(|p| walk(&p.children, id, (0.0, 0.0)))
        .unwrap_or((0.0, 0.0))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
