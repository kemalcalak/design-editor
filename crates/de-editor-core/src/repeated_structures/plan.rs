//! Read-only planning: check the request, build the master and every
//! instance, and report the members that cannot become one.

use std::collections::{HashMap, HashSet};

use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::Value;

use super::diff::{build_instance, node_id, node_type, structure_difference};
use super::locate::{inner_links, locate, Blocked};
use super::scan::ref_targets;
use super::{
    ConversionPlan, ConvertError, ConvertRequest, ConvertedMember, MasterPlacement, SkippedMember,
};
use crate::id_allocator::{collect_document_ids, IdAllocator};
use crate::page_mutators::COMPONENTS_PAGE_NAME;
use crate::{ComponentLibrary, NodeId};

/// Space between masters on the `Components` page (the kit masters' gap).
const MASTER_GAP: f64 = 40.0;

/// Check `request` against `doc` and build the conversion without changing
/// anything. `allocator` gives the new master its id (Components page).
pub fn plan_conversion(
    doc: &PenDocument,
    components: &ComponentLibrary,
    request: &ConvertRequest,
    allocator: &mut dyn IdAllocator,
) -> Result<ConversionPlan, ConvertError> {
    let doc_value = serde_json::to_value(doc).map_err(|_| ConvertError::Serialize)?;
    plan_conversion_in(doc, &doc_value, components, request, allocator)
}

/// [`plan_conversion`] over `doc_value`, the document already as JSON
/// (`serde_json::to_value(doc)`), for callers that plan several requests
/// against one document.
pub fn plan_conversion_in(
    doc: &PenDocument,
    doc_value: &Value,
    components: &ComponentLibrary,
    request: &ConvertRequest,
    allocator: &mut dyn IdAllocator,
) -> Result<ConversionPlan, ConvertError> {
    let ids: Vec<&str> = request.node_ids.iter().map(NodeId::as_str).collect();
    if ids.len() < 2 {
        return Err(ConvertError::TooFewNodes(ids.len()));
    }
    let mut wanted = HashSet::new();
    for id in &ids {
        if !wanted.insert(*id) {
            return Err(ConvertError::DuplicateNode(id.to_string()));
        }
    }
    let source_id = request.master_from.as_ref().map_or(ids[0], NodeId::as_str);
    if !wanted.contains(source_id) {
        return Err(ConvertError::MasterNotListed(source_id.to_string()));
    }

    let masters: HashSet<&str> = components
        .components
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    let found = locate(doc_value, &wanted, &masters);
    let mut targets = HashSet::new();
    ref_targets(doc_value, &mut targets);
    for id in &ids {
        let Some(node) = found.get(*id) else {
            return Err(ConvertError::NodeNotFound(id.to_string()));
        };
        let reason = match node.blocked {
            Blocked::ComponentsPage => Some("is on the Components page".to_string()),
            Blocked::InsideMaster => Some("is inside a component master".to_string()),
            Blocked::InsideInstance => Some("is inside a component instance".to_string()),
            Blocked::None if node_type(node.value) != "frame" => Some(format!(
                "is a {}; only frames can become a component",
                node_type(node.value)
            )),
            Blocked::None if super::scan::is_master(node.value, &masters) => {
                Some("is already a component master".to_string())
            }
            Blocked::None if holds_master(node.value, &masters) => {
                Some("holds a component master".to_string())
            }
            Blocked::None if targets.contains(*id) => {
                Some("is the master of other instances".to_string())
            }
            Blocked::None => None,
        };
        if let Some(reason) = reason {
            return Err(ConvertError::NotConvertible {
                id: id.to_string(),
                reason,
            });
        }
        if let Some(outer) = node
            .ancestors
            .iter()
            .find(|ancestor| wanted.contains(*ancestor))
        {
            return Err(ConvertError::NestedNodes {
                outer: outer.to_string(),
                inner: id.to_string(),
            });
        }
    }

    let source = found[source_id].value;
    let members: HashMap<&str, &Value> = ids.iter().map(|id| (*id, found[*id].value)).collect();
    let links = inner_links(doc_value, &members);
    // A node inside a copy that something names cannot move into an
    // instance: links from elsewhere would lose it, links from inside the
    // copy would land on the master's node (instance content renders under
    // `instance__child` ids).
    let referenced_in = |member: &Value| -> Option<String> {
        inner_ids(member).into_iter().find_map(|inner| {
            if let Some(holder) = links.outside.get(inner) {
                return Some(format!("inner node {inner} is referenced by node {holder}"));
            }
            links.inside.get(inner).map(|holder| {
                format!("inner node {inner} is linked from inside the copy (node {holder})")
            })
        })
    };

    let placement = request.placement;
    let master_name = source
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Component")
        .to_string();
    let mut master = source.clone();
    let master_id = match placement {
        MasterPlacement::InPlace => NodeId::new(source_id),
        MasterPlacement::ComponentsPage => {
            if let Some(reason) = referenced_in(source) {
                return Err(ConvertError::MasterUnusable {
                    id: source_id.to_string(),
                    reason,
                });
            }
            let mut taken = collect_document_ids(doc);
            let id = allocator.allocate(&mut taken)?;
            let (x, y) = next_master_slot(doc_value);
            if let Value::Object(map) = &mut master {
                map.insert("id".into(), Value::from(id.as_str()));
                map.insert("x".into(), Value::from(x));
                map.insert("y".into(), Value::from(y));
                map.insert("name".into(), Value::from(master_name.as_str()));
                // A link only some copies had stays on those instances.
                let events = source.get("events");
                if !ids
                    .iter()
                    .all(|id| found[*id].value.get("events") == events)
                {
                    map.remove("events");
                }
            }
            id
        }
    };
    if let Value::Object(map) = &mut master {
        map.insert("reusable".into(), Value::Bool(true));
    }
    let master_node: PenNode =
        serde_json::from_value(master.clone()).map_err(|_| ConvertError::Serialize)?;

    let mut converted = Vec::new();
    let mut skipped = Vec::new();
    // In place, the master source stays and only gains its flag.
    let mut bytes_before = match placement {
        MasterPlacement::ComponentsPage => 0,
        MasterPlacement::InPlace => source.to_string().len(),
    };
    let mut bytes_after = master.to_string().len();
    for id in &ids {
        let is_source = *id == source_id;
        if is_source && placement == MasterPlacement::InPlace {
            continue;
        }
        let member = found[*id].value;
        let outcome = if is_source {
            Ok(())
        } else if let Some(reason) = structure_difference(source, member) {
            Err(reason)
        } else {
            referenced_in(member).map_or(Ok(()), Err)
        };
        let outcome = outcome
            .and_then(|()| build_instance(&master, &master_node, member, found[*id].in_flow));
        match outcome {
            Ok(instance) => {
                bytes_before += member.to_string().len();
                bytes_after += instance.json_len;
                converted.push(ConvertedMember {
                    id: NodeId::new(*id),
                    overrides: instance.overrides,
                    instance: instance.node,
                });
            }
            Err(reason) if is_source => {
                return Err(ConvertError::MasterUnusable {
                    id: id.to_string(),
                    reason,
                })
            }
            Err(reason) => skipped.push(SkippedMember {
                id: NodeId::new(*id),
                reason,
            }),
        }
    }
    if !converted
        .iter()
        .any(|member| member.id.as_str() != source_id)
    {
        return Err(ConvertError::NothingToConvert(skipped));
    }
    Ok(ConversionPlan {
        master_id,
        master_from: NodeId::new(source_id),
        placement,
        master_name,
        master: master_node,
        converted,
        skipped,
        bytes_before,
        bytes_after,
    })
}

fn holds_master(node: &Value, masters: &HashSet<&str>) -> bool {
    super::diff::children(node).iter().any(|child| {
        super::scan::is_master(child, masters)
            || (node_type(child) != "ref" && holds_master(child, masters))
    })
}

/// Ids of every node inside `member` (not the root, not inside instances).
fn inner_ids(member: &Value) -> Vec<&str> {
    let mut out = Vec::new();
    let mut stack: Vec<&Value> = super::diff::children(member).iter().collect();
    while let Some(node) = stack.pop() {
        out.push(node_id(node));
        if node_type(node) != "ref" {
            stack.extend(super::diff::children(node));
        }
    }
    out
}

/// Right of the masters already on the `Components` page, top-aligned.
fn next_master_slot(doc: &Value) -> (f64, f64) {
    let right = doc
        .get("pages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|page| page.get("name").and_then(Value::as_str) == Some(COMPONENTS_PAGE_NAME))
        .flat_map(super::diff::children)
        .map(|node| {
            let number = |key: &str| node.get(key).and_then(Value::as_f64).unwrap_or(0.0);
            number("x") + number("width")
        })
        .fold(None, |max: Option<f64>, right| {
            Some(max.map_or(right, |max| max.max(right)))
        });
    (right.map_or(0.0, |right| right + MASTER_GAP), 0.0)
}
