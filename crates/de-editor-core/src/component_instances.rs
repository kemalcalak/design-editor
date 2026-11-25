//! Linked component instances (#365).
//!
//! An instance is a `{"type":"ref","ref":<master id>}` node: it renders its
//! master, so editing the master changes every instance. Masters may live
//! on any page (the hidden `Components` page included); the resolver looks
//! them up document-wide. The read-only checks live in `instance_view.rs`.

use std::collections::{BTreeMap, HashSet};

use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::instance_error::InstanceError;
use crate::pen_node_ext::PenNodeExt;
use crate::{walkers, EditorState, NodeId};

/// Offset of a page-root instance from its master's origin when the caller
/// gives no position (the old copy's offset).
const DEFAULT_INSTANCE_OFFSET: f64 = 20.0;

impl EditorState {
    /// Insert a linked instance of `component_id` under `parent` on the
    /// active page as one undo step and select it. `Ok(None)` when the
    /// insert is refused (`InstanceView::check_instance_insert`, descendants
    /// not an object).
    pub fn insert_component_instance_with_allocator(
        &mut self,
        component_id: &NodeId,
        parent: &NodeId,
        position: (Option<f64>, Option<f64>),
        descendants_json: Option<&str>,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let Ok(master) = self
            .instance_view()
            .check_instance_insert(component_id, parent)
        else {
            return Ok(None);
        };
        let Ok(descendants) = parse_descendants(descendants_json) else {
            return Ok(None);
        };
        let (mut x, mut y) = position;
        if !parent.is_real() {
            let base = master.base();
            x = x.or(Some(base.x.unwrap_or(0.0) + DEFAULT_INSTANCE_OFFSET));
            y = y.or(Some(base.y.unwrap_or(0.0) + DEFAULT_INSTANCE_OFFSET));
        }
        let mut taken = self.collect_node_ids();
        let spec = InstanceSpec {
            target: component_id.as_str(),
            x,
            y,
            // Like a Figma instance, it starts with its master's name.
            name: master.base().name.clone(),
            descendants,
        };
        let Some(node) = build_instance_node(spec, allocator, &mut taken)? else {
            return Ok(None);
        };
        let id = NodeId::new(node.id_str());
        let snap = self.snapshot_for_history();
        if !self.place_instance(parent, node) {
            return Ok(None);
        }
        self.set_single_selection(id.clone());
        self.history_push_past(snap);
        Ok(Some(id))
    }

    /// Append `node` under `parent` on the active page (`NONE` = page
    /// root). False when the parent is gone or holds no children.
    pub(crate) fn place_instance(&mut self, parent: &NodeId, node: PenNode) -> bool {
        if !parent.is_real() {
            self.active_children_mut().push(node);
            return true;
        }
        let Some(parent) = walkers::find_node_mut(self.active_children_mut(), parent) else {
            return false;
        };
        let Some(children) = parent.children_mut_or_insert() else {
            return false;
        };
        children.push(node);
        true
    }

    /// Index of the page holding `node_id` (the page `page_id` names when
    /// given); `None` when no page does.
    pub fn node_page_index(&self, node_id: &NodeId, page_id: Option<&str>) -> Option<usize> {
        if page_id.is_some() {
            return crate::command_apply::command_page_index(self, page_id);
        }
        self.instance_view().find_node_page(node_id)
    }
}

/// What [`build_instance_node`] puts on a new instance.
pub(crate) struct InstanceSpec<'a> {
    pub target: &'a str,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub name: Option<String>,
    pub descendants: BTreeMap<String, Value>,
}

/// A fresh `ref` node. Its own content (replacement subtrees in
/// `descendants`) gets fresh ids, so it never shares ids with the master
/// or another instance (#364). `Ok(None)` when the result is not a valid
/// node.
pub(crate) fn build_instance_node(
    spec: InstanceSpec<'_>,
    allocator: &mut dyn IdAllocator,
    taken: &mut HashSet<NodeId>,
) -> Result<Option<PenNode>, IdAllocError> {
    let id = allocator.allocate(taken)?;
    let mut descendants = spec.descendants;
    crate::instance_ids::remint_owned_ids(&mut descendants, allocator, taken)?;
    let mut node = Map::new();
    node.insert("type".into(), Value::from("ref"));
    node.insert("id".into(), Value::from(id.as_str()));
    node.insert("ref".into(), Value::from(spec.target));
    if let Some(name) = spec.name {
        node.insert("name".into(), Value::from(name));
    }
    if let Some(x) = spec.x {
        node.insert("x".into(), Value::from(x));
    }
    if let Some(y) = spec.y {
        node.insert("y".into(), Value::from(y));
    }
    if !descendants.is_empty() {
        node.insert(
            "descendants".into(),
            Value::Object(descendants.into_iter().collect()),
        );
    }
    Ok(serde_json::from_value(Value::Object(node)).ok())
}

/// `descendants` JSON as a map; absent or blank is empty.
pub fn parse_descendants(json: Option<&str>) -> Result<BTreeMap<String, Value>, InstanceError> {
    let Some(raw) = json.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(BTreeMap::new());
    };
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => Ok(map.into_iter().collect()),
        _ => Err(InstanceError::InvalidDescendants),
    }
}
