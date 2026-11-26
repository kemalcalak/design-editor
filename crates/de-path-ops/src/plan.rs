//! Plans: a computed path plus what the document write needs (the
//! sources, the result's frame origin), shared by the desktop canvas and
//! the MCP tools so both write the same `PathCommand::Commit`.

use crate::{
    compute_boolean, flatten_nodes, layout_origin, offset_node, outline_stroke, PathResult,
};
use de_editor_core::path_ops::{CanvasPath, OffsetJoin, PathCommit, PathCommitKind};
use de_editor_core::NodeId;
use jian_ops_schema::node::BooleanOperation;
use jian_scene::layout_scene::LayoutScene;
use skia_safe::Path;

/// A computed path ready to write.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// Scene ids the result replaces, first one giving the slot.
    pub source_ids: Vec<String>,
    pub result: PathResult,
    /// Document origin of the first source's frame.
    pub origin: (f64, f64),
}

impl Planned {
    fn new(scene: &LayoutScene, source_ids: &[String], path: &Path) -> Option<Self> {
        let first = source_ids.first()?;
        Some(Self {
            source_ids: source_ids.to_vec(),
            result: PathResult::from_path(path)?,
            origin: layout_origin(scene, first),
        })
    }

    fn ids(&self) -> Vec<NodeId> {
        self.source_ids.iter().map(NodeId::new).collect()
    }

    /// The canvas form (the host mints the id).
    pub fn into_canvas(self, kind: PathCommitKind) -> CanvasPath {
        CanvasPath {
            source_ids: self.ids(),
            subpaths: self.result.subpaths,
            even_odd: self.result.even_odd,
            origin: self.origin,
            kind,
        }
    }

    /// The command form, for a write computed against `revision`.
    pub fn into_commit(self, kind: PathCommitKind, new_id: NodeId, revision: u64) -> PathCommit {
        PathCommit {
            source_ids: self.ids(),
            subpaths: self.result.subpaths,
            even_odd: self.result.even_odd,
            origin: self.origin,
            new_id,
            revision,
            kind,
        }
    }
}

/// A destructive boolean of `ids` (paint order).
pub fn plan_boolean(scene: &LayoutScene, ids: &[String], op: BooleanOperation) -> Option<Planned> {
    Planned::new(scene, ids, &compute_boolean(scene, ids, op)?)
}

/// `ids` baked into one path.
pub fn plan_flatten(scene: &LayoutScene, ids: &[String]) -> Option<Planned> {
    Planned::new(scene, ids, &flatten_nodes(scene, ids)?)
}

/// The stroke of `id` as a filled path.
pub fn plan_outline_stroke(scene: &LayoutScene, id: &str) -> Option<Planned> {
    let node = scene.active_page()?.find(id)?;
    Planned::new(scene, &[id.to_string()], &outline_stroke(node)?)
}

/// The outline of `id` grown (or shrunk) by `delta`.
pub fn plan_offset(scene: &LayoutScene, id: &str, delta: f32, join: OffsetJoin) -> Option<Planned> {
    let node = scene.active_page()?.find(id)?;
    Planned::new(scene, &[id.to_string()], &offset_node(node, delta, join)?)
}
