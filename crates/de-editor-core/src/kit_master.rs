//! UIKit components as linked masters (#365).
//!
//! The first linked use of a kit component installs its template as a
//! `reusable` master on the hidden `Components` page; every use then
//! inserts a `ref` to it, so editing that master changes every instance.
//! Ids stay verbatim, like library masters, unless one is already taken
//! or holds the `descendants` path separator: then the master gets fresh
//! ids and `editor_ui.kit_masters` (persisted in `editorMeta.kitMasters`)
//! records which master stands for the kit component. The record travels
//! with undo and batch rollback (it is part of the history snapshot) and is
//! dropped with the document; a recorded id that is no longer a master on
//! the `Components` page still installs the master again.

use std::collections::{BTreeMap, HashMap, HashSet};

use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::VariableDefinition;
use serde_json::{Map, Value};

use crate::component_instances::{build_instance_node, InstanceSpec};
use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::instance_ids::split_first_segment;
use crate::page_mutators::is_components_page;
use crate::pen_node_ext::PenNodeExt;
use crate::{walkers, ComponentLibrary, EditorState, NodeId};

/// Horizontal space between kit masters laid out on the `Components` page.
const MASTER_GAP: f64 = 40.0;

/// Where a kit component goes and how it is customised.
#[derive(Debug, Clone, Copy)]
pub(crate) struct KitPlacement<'a> {
    pub parent: &'a NodeId,
    pub doc_x: f64,
    pub doc_y: f64,
    pub overrides_json: Option<&'a str>,
}

/// The `editorMeta.kitMasters` key of a kit component.
pub fn kit_master_key(kit_id: &str, component_id: &str) -> String {
    format!("{kit_id}/{component_id}")
}

struct KitTemplate {
    template: PenNode,
    /// The component's display name (the master's layer name).
    name: String,
    variables: Option<BTreeMap<String, VariableDefinition>>,
}

/// An installed master and the kit template's ids mapped onto its ids.
struct KitMaster {
    node: PenNode,
    ids: HashMap<String, String>,
    fresh_ids: bool,
}

impl EditorState {
    /// Whether the kit component exists and its template can be a master
    /// (a frame); anything else is inserted as a copy.
    pub(crate) fn kit_template_is_frame(&self, kit_id: &str, component_id: &str) -> bool {
        self.ui_kits
            .iter()
            .find(|kit| kit.id == kit_id)
            .and_then(|kit| kit.components.iter().find(|c| c.id == component_id))
            .is_some_and(|component| matches!(component.template, PenNode::Frame(_)))
    }

    /// Id of the master this document holds for the kit component, if it
    /// was installed.
    pub fn installed_kit_master(&self, kit_id: &str, component_id: &str) -> Option<String> {
        let kit = self.kit_template(kit_id, component_id)?;
        self.find_kit_master(&kit_master_key(kit_id, component_id), &kit.template)
            .map(|master| master.node.id_str().to_string())
    }

    /// Insert a linked instance of a kit component, installing its master
    /// first when the document has none. Install and insert are one undo
    /// step.
    pub(crate) fn insert_linked_kit_component_with_allocator(
        &mut self,
        kit_id: &str,
        component_id: &str,
        placement: KitPlacement<'_>,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        if placement.parent.is_real() {
            match walkers::find_node(self.active_children(), placement.parent) {
                Some(parent) if parent.is_container() => {}
                _ => return Ok(None),
            }
        }
        let Some(kit) = self.kit_template(kit_id, component_id) else {
            return Ok(None);
        };
        let overrides = match placement.overrides_json {
            None => Map::new(),
            Some(raw) => match serde_json::from_str::<Value>(raw) {
                Ok(Value::Object(map)) => map,
                _ => return Ok(None),
            },
        };
        let key = kit_master_key(kit_id, component_id);
        let snap = self.snapshot_for_history();
        let (master, installed) = match self.find_kit_master(&key, &kit.template) {
            Some(master) => (master, false),
            None => match self.install_kit_master(&kit, allocator) {
                Ok(Some(master)) => (master, true),
                Ok(None) => {
                    // Installing may already have moved the design into pages.
                    self.restore(snap);
                    return Ok(None);
                }
                Err(error) => {
                    self.restore(snap);
                    return Err(error);
                }
            },
        };
        let inserted = self.insert_kit_instance(&master, &overrides, placement, allocator);
        match inserted {
            Ok(Some(id)) => {
                if master.fresh_ids {
                    self.editor_ui
                        .kit_masters
                        .insert(key, master.node.id_str().to_string());
                } else {
                    self.editor_ui.kit_masters.remove(&key);
                }
                self.set_single_selection(id.clone());
                self.history_push_past(snap);
                Ok(Some(id))
            }
            other => {
                if installed {
                    self.restore(snap);
                }
                other
            }
        }
    }

    fn insert_kit_instance(
        &mut self,
        master: &KitMaster,
        overrides: &Map<String, Value>,
        placement: KitPlacement<'_>,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<NodeId>, IdAllocError> {
        let master_id = NodeId::new(master.node.id_str());
        if self
            .instance_view()
            .check_instance_insert(&master_id, placement.parent)
            .is_err()
        {
            return Ok(None);
        }
        let mut spec = kit_overrides_to_instance(overrides, master);
        spec.name = spec.name.or_else(|| master.node.base().name.clone());
        spec.x = spec.x.or(Some(placement.doc_x));
        spec.y = spec.y.or(Some(placement.doc_y));
        let mut taken = self.collect_node_ids();
        let Some(node) = build_instance_node(spec, allocator, &mut taken)? else {
            return Ok(None);
        };
        let id = NodeId::new(node.id_str());
        Ok(self.place_instance(placement.parent, node).then_some(id))
    }

    fn kit_template(&self, kit_id: &str, component_id: &str) -> Option<KitTemplate> {
        let kit = self.ui_kits.iter().find(|kit| kit.id == kit_id)?;
        let component = kit
            .components
            .iter()
            .find(|component| component.id == component_id)?;
        Some(KitTemplate {
            template: component.template.clone(),
            name: component.name.clone(),
            variables: kit.variables.clone(),
        })
    }

    /// The recorded master, else a master on the `Components` page under
    /// the template's own root id.
    fn find_kit_master(&self, key: &str, template: &PenNode) -> Option<KitMaster> {
        let recorded = self.editor_ui.kit_masters.get(key).map(String::as_str);
        recorded
            .into_iter()
            .chain(std::iter::once(template.id_str()))
            .find_map(|id| {
                let node = self.components_page_master(id)?;
                let fresh_ids = id != template.id_str();
                // A master kept the kit's own ids, so keys map to themselves
                // however the master was edited since; fresh ids map by
                // place.
                let mut ids = HashMap::new();
                if fresh_ids {
                    zip_ids(template, node, &mut ids);
                }
                Some(KitMaster {
                    node: node.clone(),
                    ids,
                    fresh_ids,
                })
            })
    }

    fn components_page_master(&self, id: &str) -> Option<&PenNode> {
        self.doc
            .pages
            .iter()
            .flatten()
            .filter(|page| is_components_page(page))
            .flat_map(|page| page.children.iter())
            .find(|node| {
                node.id_str() == id
                    && matches!(node, PenNode::Frame(frame) if frame.reusable == Some(true))
            })
    }

    /// Put the template on the `Components` page as a registered master,
    /// with its kit variables. `Ok(None)` when the template is no frame.
    fn install_kit_master(
        &mut self,
        kit: &KitTemplate,
        allocator: &mut dyn IdAllocator,
    ) -> Result<Option<KitMaster>, IdAllocError> {
        let mut master = kit.template.clone();
        let PenNode::Frame(frame) = &mut master else {
            return Ok(None);
        };
        frame.reusable = Some(true);
        let (x, y) = self.next_master_slot();
        let base = master.base_mut();
        base.name = Some(kit.name.clone());
        base.x = Some(x);
        base.y = Some(y);
        let mut taken = self.collect_node_ids();
        let mut template_ids = HashSet::new();
        subtree_ids(&master, &mut template_ids);
        // An id with the path separator (the starter kit's `kit/…/label`)
        // could not be named in `descendants`, so it is re-minted too.
        let fresh_ids = template_ids.iter().any(|id| {
            id.contains(crate::instance_descendants::PATH_SEPARATOR)
                || taken.contains(&NodeId::new(id.as_str()))
        });
        if fresh_ids {
            master = walkers::deep_clone_with_allocator(&master, allocator, &mut taken)?;
        }
        let had_pages = self
            .doc
            .pages
            .as_ref()
            .is_some_and(|pages| !pages.is_empty());
        if self.append_components_page_masters_with_allocator(vec![master.clone()], allocator)? == 0
        {
            return Ok(None);
        }
        let id = NodeId::new(master.id_str());
        if had_pages {
            self.components
                .register_document_component(&self.doc, id, kit.name.clone());
        } else {
            // The design moved into page 0, so every stored location moved.
            self.components = ComponentLibrary::from_document(&self.doc);
        }
        self.install_kit_variables(&kit.template, kit.variables.as_ref());
        let mut ids = HashMap::new();
        if fresh_ids {
            zip_ids(&kit.template, &master, &mut ids);
        }
        Ok(Some(KitMaster {
            node: master,
            ids,
            fresh_ids,
        }))
    }

    /// Right of the masters already on the `Components` page, top-aligned.
    fn next_master_slot(&self) -> (f64, f64) {
        let right = self
            .doc
            .pages
            .iter()
            .flatten()
            .filter(|page| is_components_page(page))
            .flat_map(|page| page.children.iter())
            .map(|node| node.base().x.unwrap_or(0.0) + node.width_px().unwrap_or(0.0))
            .fold(None, |max: Option<f64>, right| {
                Some(max.map_or(right, |max| max.max(right)))
            });
        (right.map_or(0.0, |right| right + MASTER_GAP), 0.0)
    }
}

/// The instance a `K()` call's overrides describe. Overrides keep the kit
/// template's ids (they were written against the kit); they are mapped
/// onto the master's ids. `label` / `text` set the first text's content,
/// `name` / `x` / `y` sit on the instance, `descendants` entries go to the
/// matching master nodes and every other key overrides the master's root.
fn kit_overrides_to_instance<'a>(
    overrides: &Map<String, Value>,
    master: &'a KitMaster,
) -> InstanceSpec<'a> {
    let root = master.node.id_str();
    let mut descendants: BTreeMap<String, Value> = BTreeMap::new();
    let mut root_patch = Map::new();
    if let Some(Value::Object(entries)) = overrides.get("descendants") {
        for (key, value) in entries {
            // A kit id may itself hold the separator: map the whole key
            // first, else its first segment (`nested/leaf` paths).
            let key = match master.ids.get(key.as_str()) {
                Some(mapped) => mapped.clone(),
                None => {
                    let (first, rest) = split_first_segment(key);
                    let mapped = master.ids.get(first).map_or(first, String::as_str);
                    format!("{mapped}{rest}")
                }
            };
            if key == root {
                if let Value::Object(patch) = value {
                    for (field, field_value) in patch {
                        // `descendants[<root>]` is a patch: identity and
                        // structure never move (the top-level list below also
                        // drops the K()-only keys).
                        if !matches!(field.as_str(), "id" | "type" | "children" | "descendants") {
                            root_patch.insert(field.clone(), field_value.clone());
                        }
                    }
                }
                continue;
            }
            merge_entry(&mut descendants, key, value.clone());
        }
    }
    for (field, value) in overrides {
        if !matches!(
            field.as_str(),
            "id" | "type" | "ref" | "children" | "descendants" | "label" | "text"
        ) {
            root_patch.insert(field.clone(), value.clone());
        }
    }
    let name = match root_patch.remove("name") {
        Some(Value::String(name)) => Some(name),
        _ => None,
    };
    let x = root_patch.remove("x").and_then(|value| value.as_f64());
    let y = root_patch.remove("y").and_then(|value| value.as_f64());
    let label = overrides
        .get("label")
        .or_else(|| overrides.get("text"))
        .and_then(Value::as_str);
    if let (Some(label), Some(text_id)) = (label, first_text_id(&master.node)) {
        let mut patch = Map::new();
        patch.insert("content".into(), Value::from(label));
        merge_entry(&mut descendants, text_id.to_string(), Value::Object(patch));
    }
    if !root_patch.is_empty() {
        merge_entry(
            &mut descendants,
            root.to_string(),
            Value::Object(root_patch),
        );
    }
    InstanceSpec {
        target: root,
        x,
        y,
        name,
        descendants,
    }
}

/// Add `value` under `key`, merging two patch objects field by field.
fn merge_entry(descendants: &mut BTreeMap<String, Value>, key: String, value: Value) {
    match (descendants.get_mut(&key), value) {
        (Some(Value::Object(existing)), Value::Object(patch)) => existing.extend(patch),
        (_, value) => {
            descendants.insert(key, value);
        }
    }
}

/// The first text node under `node`, depth first, not inside instances.
fn first_text_id(node: &PenNode) -> Option<&str> {
    if matches!(node, PenNode::Text(_)) {
        return Some(node.id_str());
    }
    if matches!(node, PenNode::Ref(_)) {
        return None;
    }
    node.children()
        .into_iter()
        .flatten()
        .find_map(first_text_id)
}

/// Map each template id onto the master node in the same place, while the
/// kinds still match (an edited master may have moved things).
fn zip_ids(template: &PenNode, master: &PenNode, out: &mut HashMap<String, String>) {
    if template.type_tag() != master.type_tag() {
        return;
    }
    out.insert(template.id_str().to_string(), master.id_str().to_string());
    if let (Some(left), Some(right)) = (template.children(), master.children()) {
        for (left, right) in left.iter().zip(right) {
            zip_ids(left, right, out);
        }
    }
}

fn subtree_ids(node: &PenNode, out: &mut HashSet<String>) {
    out.insert(node.id_str().to_string());
    crate::instance_ids::for_each_owned_id(node, &mut |id| {
        out.insert(id.to_string());
    });
    for child in node.children().into_iter().flatten() {
        subtree_ids(child, out);
    }
}
