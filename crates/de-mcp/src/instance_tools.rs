//! Component-instance MCP tools (#365): `instantiate_component` inserts a
//! linked instance (`ref`) by default, `detach_instance` turns one into an
//! independent subtree, `swap_instance` points one at another master and
//! `list_instances` says where a master is used.
//!
//! Each snapshot keeps the document, the component registry and the active
//! page: enough to check the call and predict the ids the host's apply
//! gives (`EditorState::apply` allocates from the document's high-water
//! mark, like `repair_duplicate_ids`).
//!
//! The argument helpers return a finished `ToolOutcome::Err` that the tool
//! hands back as is, so the large `Err` is deliberate (as in
//! `element_tools.rs`).
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, HashSet};

use de_editor_core::id_allocator::{collect_document_ids, SequentialIdAllocator};
use de_editor_core::{
    parse_descendants, ComponentLibrary, EditorCommand, EditorState, InstanceCommand,
    InstanceError, InstanceView, NodeId, PenNodeExt,
};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

/// Where agents read how instances, overrides and masters work (#366).
const COMPONENTS_GUIDE: &str = "get_guidelines topic components";

/// What every instance tool reads.
pub struct InstanceSnapshot {
    doc: PenDocument,
    components: ComponentLibrary,
    page_index: usize,
}

impl InstanceSnapshot {
    fn new(state: &EditorState) -> Self {
        Self {
            doc: state.doc.clone(),
            components: state.components.clone(),
            page_index: state.ui.active_page_index,
        }
    }

    fn view(&self) -> InstanceView<'_> {
        InstanceView {
            doc: &self.doc,
            components: &self.components,
            page_index: self.page_index,
        }
    }

    /// The id of page `index`, `None` for a document without pages.
    fn page_id(&self, index: usize) -> Option<String> {
        self.doc
            .pages
            .as_ref()
            .and_then(|pages| pages.get(index))
            .map(|page| page.id.clone())
    }

    /// The page a node lives on, as an error the agent can act on.
    fn node_page(&self, node_id: &NodeId) -> Result<usize, ToolOutcome> {
        self.view()
            .find_node_page(node_id)
            .ok_or_else(|| invalid(InstanceError::NodeNotFound(node_id.as_str().to_string())))
    }

    /// A fresh allocator and taken set, as the host's apply starts with.
    fn allocator(&self) -> Option<(SequentialIdAllocator, HashSet<NodeId>)> {
        let allocator = SequentialIdAllocator::for_document(&self.doc, 1).ok()?;
        Some((allocator, collect_document_ids(&self.doc)))
    }
}

/// The first alias with a non-blank value.
fn arg<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|key| args.get(*key))
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
}

fn required<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Result<&'a str, ToolOutcome> {
    arg(args, keys).ok_or_else(|| {
        ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            format!("{} is required", keys[0]),
        )
    })
}

fn invalid(message: impl ToString) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message.to_string())
}

fn number(args: &BTreeMap<String, String>, key: &str) -> Result<Option<f64>, ToolOutcome> {
    match arg(args, &[key]) {
        None => Ok(None),
        Some(raw) => raw
            .parse::<f64>()
            .map(Some)
            .map_err(|_| invalid(format!("{key} must be a number"))),
    }
}

fn flag(args: &BTreeMap<String, String>, key: &str) -> Result<bool, ToolOutcome> {
    match arg(args, &[key]) {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(other) => Err(invalid(format!("{key} must be true or false, got {other}"))),
    }
}

fn node_json(node: &PenNode) -> Value {
    json!({ "id": node.id_str(), "name": node.base().name, "type": node.type_tag() })
}

/// `instantiate_component`: a linked instance by default, a copy with
/// `detach: true`.
pub struct InstantiateComponent(InstanceSnapshot);

pub fn instantiate_component_snapshot(state: &EditorState) -> InstantiateComponent {
    InstantiateComponent(InstanceSnapshot::new(state))
}

impl InstantiateComponent {
    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolOutcome> {
        let snapshot = &self.0;
        let component_id = NodeId::new(required(args, &["component_id", "componentId"])?);
        // The first id the host's apply allocates is the new node's.
        let predicted = || {
            snapshot
                .allocator()
                .and_then(|(mut allocator, mut taken)| {
                    use de_editor_core::IdAllocator;
                    allocator.allocate(&mut taken).ok()
                })
                .map(|id| id.as_str().to_string())
        };
        if flag(args, "detach")? {
            if snapshot.components.find_by_id(&component_id).is_none() {
                return Err(invalid(format!(
                    "component not found: {} (detach copies a registered component)",
                    component_id.as_str()
                )));
            }
            let result = json!({ "nodeId": predicted(), "componentId": component_id.as_str(), "linked": false });
            return Ok(ToolOutcome::OkJsonWithCommand(
                result.to_string(),
                EditorCommand::Instance(Box::new(InstanceCommand::Copy { component_id })),
            ));
        }
        let parent = arg(args, &["parent", "parentId"])
            .map(NodeId::new)
            .unwrap_or(NodeId::NONE);
        let page = if parent.is_real() {
            snapshot.node_page(&parent)?
        } else {
            snapshot.page_index
        };
        let view = snapshot.view().on_page(page);
        view.check_instance_insert(&component_id, &parent)
            .map_err(invalid)?;
        let descendants = arg(args, &["descendants", "overrides"]);
        parse_descendants(descendants).map_err(invalid)?;
        let overridable: Vec<Value> = view
            .override_targets(component_id.as_str())
            .into_iter()
            .map(|target| json!({ "id": target.id, "name": target.name, "type": target.kind }))
            .collect();
        let page_id = snapshot.page_id(page);
        let result = json!({
            "instanceId": predicted(),
            "componentId": component_id.as_str(),
            "linked": true,
            "pageId": page_id,
            "overridable": overridable,
            "hint": format!(
                "Linked instance: editing the master changes every instance. Override per instance with descendants keyed by the ids in overridable, e.g. {{\"<id>\":{{\"content\":\"Save\"}}}}. See {COMPONENTS_GUIDE}."
            ),
        });
        Ok(ToolOutcome::OkJsonWithCommand(
            result.to_string(),
            EditorCommand::Instance(Box::new(InstanceCommand::Insert {
                component_id,
                parent,
                x: number(args, "x")?,
                y: number(args, "y")?,
                descendants_json: descendants.map(str::to_string),
                page_id: (page != snapshot.page_index).then_some(page_id).flatten(),
            })),
        ))
    }
}

impl McpTool for InstantiateComponent {
    fn name(&self) -> &str {
        "instantiate_component"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args).unwrap_or_else(|error| error)
    }
}

/// `detach_instance`: the instance becomes an independent subtree; its root
/// keeps the id, its inner nodes get the fresh ids the result lists.
pub struct DetachInstance(InstanceSnapshot);

pub fn detach_instance_snapshot(state: &EditorState) -> DetachInstance {
    DetachInstance(InstanceSnapshot::new(state))
}

impl DetachInstance {
    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolOutcome> {
        let snapshot = &self.0;
        let node_id = NodeId::new(required(args, &["nodeId", "node_id"])?);
        let page = snapshot.node_page(&node_id)?;
        let view = snapshot.view().on_page(page);
        let instance = view.instance_node(&node_id).map_err(invalid)?;
        let PenNode::Ref(reference) = instance else {
            unreachable!("instance_node returns refs only");
        };
        let (mut allocator, mut taken) = snapshot
            .allocator()
            .ok_or_else(|| ToolOutcome::Err(ToolErrorCode::ToolFailed, "no free ids".into()))?;
        let detached = view
            .detached_instance(instance, &mut allocator, &mut taken)
            .map_err(|error| ToolOutcome::Err(ToolErrorCode::ToolFailed, error.to_string()))?
            .ok_or_else(|| {
                invalid(format!(
                    "the master {} of {} is gone",
                    reference.target,
                    node_id.as_str()
                ))
            })?;
        let mut nodes = Vec::new();
        collect_inner(&detached, &mut nodes);
        let result = json!({
            "nodeId": node_id.as_str(),
            "componentId": reference.target,
            "detached": true,
            "nodes": nodes,
        });
        Ok(ToolOutcome::OkJsonWithCommand(
            result.to_string(),
            EditorCommand::Instance(Box::new(InstanceCommand::Detach {
                node_id,
                page_id: snapshot.page_id(page),
            })),
        ))
    }
}

/// Every node under `node` (not `node` itself), depth first.
fn collect_inner(node: &PenNode, out: &mut Vec<Value>) {
    for child in node.children().into_iter().flatten() {
        out.push(node_json(child));
        collect_inner(child, out);
    }
}

impl McpTool for DetachInstance {
    fn name(&self) -> &str {
        "detach_instance"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args).unwrap_or_else(|error| error)
    }
}

/// `swap_instance`: the instance shows another master, keeping its
/// descendants overrides, bindings and events.
pub struct SwapInstance(InstanceSnapshot);

pub fn swap_instance_snapshot(state: &EditorState) -> SwapInstance {
    SwapInstance(InstanceSnapshot::new(state))
}

impl SwapInstance {
    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolOutcome> {
        let snapshot = &self.0;
        let node_id = NodeId::new(required(args, &["nodeId", "node_id"])?);
        let component_id = NodeId::new(required(args, &["component_id", "componentId"])?);
        let page = snapshot.node_page(&node_id)?;
        let view = snapshot.view().on_page(page);
        view.check_instance_swap(&node_id, &component_id)
            .map_err(invalid)?;
        let previous = match view.instance_node(&node_id) {
            Ok(PenNode::Ref(reference)) => Some(reference.target.clone()),
            _ => None,
        };
        let result = json!({
            "nodeId": node_id.as_str(),
            "componentId": component_id.as_str(),
            "previousComponentId": previous,
        });
        Ok(ToolOutcome::OkJsonWithCommand(
            result.to_string(),
            EditorCommand::Instance(Box::new(InstanceCommand::Swap {
                node_id,
                component_id,
                page_id: snapshot.page_id(page),
            })),
        ))
    }
}

impl McpTool for SwapInstance {
    fn name(&self) -> &str {
        "swap_instance"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args).unwrap_or_else(|error| error)
    }
}

/// `list_instances`: every instance of a master, with its page and how
/// many overrides it carries.
pub struct ListInstances(InstanceSnapshot);

pub fn list_instances_snapshot(state: &EditorState) -> ListInstances {
    ListInstances(InstanceSnapshot::new(state))
}

impl McpTool for ListInstances {
    fn name(&self) -> &str {
        "list_instances"
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let component_id = match required(args, &["component_id", "componentId"]) {
            Ok(id) => id,
            Err(error) => return error,
        };
        let view = self.0.view();
        let master = match view.component_master(component_id) {
            Ok(master) => master,
            Err(error) => return invalid(error),
        };
        let instances: Vec<Value> = view
            .list_instances(component_id)
            .into_iter()
            .map(|used| {
                json!({
                    "id": used.instance_id,
                    "pageId": used.page_id,
                    "pageName": used.page_name,
                    "overrideCount": used.override_count,
                })
            })
            .collect();
        ToolOutcome::OkJson(
            json!({
                "componentId": component_id,
                "name": master.base().name,
                "count": instances.len(),
                "instances": instances,
            })
            .to_string(),
        )
    }
}

#[cfg(test)]
#[path = "instance_tools_tests.rs"]
mod tests;
