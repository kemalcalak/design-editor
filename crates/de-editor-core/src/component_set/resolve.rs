//! Which master an instance shows, and keeping instances on the right
//! master when a set changes.
//!
//! Stored form: an instance of a variant keeps `ref` on the variant
//! master it shows and `variant` with its full combination. A `ref` that
//! names the set itself (pen.dev's form, or an agent's insert) is accepted
//! everywhere and rewritten to the master on insert and on load; the
//! render resolver picks the master for one it meets unrewritten. Keeping
//! the master in `ref` means every reader that only follows `ref`
//! (codegen, exports, detach, properties) sees the right component.

use std::collections::HashMap;

use jian_ops_schema::component_set::{ComponentSet, VariantValues};
use jian_ops_schema::node::{PenNode, RefNode};
use jian_ops_schema::PenDocument;

use super::error::join;
use super::pick::{
    component_set_of, full_values, pick_variant, set_containing, variant_values_of, SetIndex,
};
use super::report::{DroppedOverride, VariantFallback, VariantReport};
use super::tree::{find, for_each_ref_mut};
use super::VariantError;
use crate::pen_node_ext::PenNodeExt;

/// Check that `requested` names only axes and values of `set`.
pub fn check_values(set: &ComponentSet, requested: &VariantValues) -> Result<(), VariantError> {
    for (axis, value) in requested {
        let Some(values) = set.axes.values(axis) else {
            return Err(VariantError::UnknownAxis {
                axis: axis.clone(),
                allowed: join(set.axes.names()),
            });
        };
        if !values.contains(value) {
            return Err(VariantError::UnknownValue {
                axis: axis.clone(),
                value: value.clone(),
                allowed: join(values),
            });
        }
    }
    Ok(())
}

/// The master an instance resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceTarget {
    pub set_id: String,
    pub master_id: String,
    pub values: VariantValues,
    pub exact: bool,
}

/// Resolve an instance of `target` asking for `requested`: `Ok(None)` when
/// `target` is no set and no variant (a plain component, or unknown), and
/// nothing was asked. Asking a variant of a plain component, or for an
/// axis / value the set lacks, is refused.
pub fn resolve_target(
    doc: &PenDocument,
    target: &str,
    requested: Option<&VariantValues>,
) -> Result<Option<InstanceTarget>, VariantError> {
    resolve_target_in(doc, target, requested, &|id| set_containing(doc, id))
}

/// [`resolve_target`] with the caller's way to find the set holding a
/// master (the editor reads it from the registry's stored location
/// instead of walking the document).
pub(crate) fn resolve_target_in<'a>(
    doc: &'a PenDocument,
    target: &str,
    requested: Option<&VariantValues>,
    set_of: &dyn Fn(&str) -> Option<&'a PenNode>,
) -> Result<Option<InstanceTarget>, VariantError> {
    let Some(node) = find(doc, target) else {
        return Ok(None);
    };
    let (set_node, current) = if component_set_of(node).is_some() {
        (node, None)
    } else {
        match set_of(target) {
            Some(set_node) => (set_node, variant_values_of(node)),
            None if requested.is_some_and(|r| !r.is_empty()) => {
                return Err(VariantError::NotAVariant(target.to_string()))
            }
            None => return Ok(None),
        }
    };
    let Some(set) = component_set_of(set_node) else {
        return Ok(None);
    };
    if let Some(requested) = requested {
        check_values(set, requested)?;
    }
    let values = full_values(set, requested, current);
    let Some(pick) = pick_variant(set_node, &values) else {
        return Err(VariantError::NoMasters);
    };
    Ok(Some(InstanceTarget {
        set_id: set_node.id_str().to_string(),
        master_id: pick.master.id_str().to_string(),
        values,
        exact: pick.exact,
    }))
}

/// The master an instance shows now: its `ref`, or the variant its
/// `variant` picks when `ref` names the set.
pub(crate) fn shown_master<'a>(doc: &'a PenDocument, reference: &RefNode) -> Option<&'a PenNode> {
    let node = find(doc, &reference.target)?;
    match component_set_of(node) {
        Some(set) => {
            let values = full_values(set, reference.variant.as_ref(), None);
            pick_variant(node, &values).map(|pick| pick.master)
        }
        None => Some(node),
    }
}

/// Rewrite instances in `nodes` (about to be inserted) that name a set or
/// ask for a variant onto the master they show. Refuses unknown axes and
/// values; returns the instances that fell back to the nearest variant.
pub fn normalize_inserted(
    doc: &PenDocument,
    nodes: &mut [PenNode],
) -> Result<Vec<VariantFallback>, VariantError> {
    let index = SetIndex::build(doc);
    // Without sets only a `variant` (refused below) needs a look.
    if index.is_empty() && !nodes.iter().any(asks_variant) {
        return Ok(Vec::new());
    }
    normalize_with(doc, &index, nodes)
}

fn asks_variant(node: &PenNode) -> bool {
    matches!(node, PenNode::Ref(reference) if reference.variant.is_some())
        || node
            .children()
            .is_some_and(|children| children.iter().any(asks_variant))
}

fn normalize_with(
    doc: &PenDocument,
    index: &SetIndex,
    nodes: &mut [PenNode],
) -> Result<Vec<VariantFallback>, VariantError> {
    let mut fallbacks = Vec::new();
    for node in nodes {
        if let PenNode::Ref(reference) = node {
            let set_of = |id: &str| index.set_of(id).and_then(|set| find(doc, set));
            if let Some(target) =
                resolve_target_in(doc, &reference.target, reference.variant.as_ref(), &set_of)?
            {
                if !target.exact {
                    fallbacks.push(VariantFallback {
                        instance: reference.base.id.clone(),
                        requested: target.values.clone(),
                        shown: target.master_id.clone(),
                    });
                }
                if reference.target == target.set_id {
                    move_root_override(reference, &target.master_id);
                    reference.target = target.master_id;
                    reference.variant = Some(target.values);
                } else if let Some(new_master) = find(doc, &target.master_id) {
                    // `ref` names a variant master: its overrides follow
                    // to the variant `variant` asks for.
                    let old = find(doc, &reference.target);
                    super::switch::retarget(reference, old, new_master, target.values);
                }
            }
        }
        if let Some(children) = node.children_mut() {
            fallbacks.extend(normalize_with(doc, index, children)?);
        }
    }
    Ok(fallbacks)
}

/// On load: point every instance that names a set at the master it
/// shows. Never refuses; values the set lacks fall back to the defaults.
pub fn normalize_set_refs(doc: &mut PenDocument) -> Vec<VariantFallback> {
    let index = SetIndex::build(doc);
    if index.is_empty() {
        return Vec::new();
    }
    let mut picks: HashMap<String, InstanceTarget> = HashMap::new();
    let snapshot = &*doc;
    let mut wanted: Vec<(String, String, Option<VariantValues>)> = Vec::new();
    super::pick::for_each_node(snapshot, &mut |node| {
        if let PenNode::Ref(reference) = node {
            if find(snapshot, &reference.target).is_some_and(|t| component_set_of(t).is_some()) {
                wanted.push((
                    reference.base.id.clone(),
                    reference.target.clone(),
                    reference.variant.clone(),
                ));
            }
        }
    });
    for (instance, target, requested) in wanted {
        let Some(set_node) = find(snapshot, &target) else {
            continue;
        };
        let Some(set) = component_set_of(set_node) else {
            continue;
        };
        let values = full_values(set, requested.as_ref(), None);
        if let Some(pick) = pick_variant(set_node, &values) {
            picks.insert(
                instance,
                InstanceTarget {
                    set_id: target,
                    master_id: pick.master.id_str().to_string(),
                    values,
                    exact: pick.exact,
                },
            );
        }
    }
    let mut fallbacks = Vec::new();
    for_each_ref_mut(doc, &mut |reference| {
        if let Some(target) = picks
            .get(&reference.base.id)
            .filter(|target| target.set_id == reference.target)
        {
            if !target.exact {
                fallbacks.push(VariantFallback {
                    instance: reference.base.id.clone(),
                    requested: target.values.clone(),
                    shown: target.master_id.clone(),
                });
            }
            move_root_override(reference, &target.master_id);
            reference.target = target.master_id.clone();
            reference.variant = Some(target.values.clone());
        }
    });
    fallbacks
}

/// The instance's top-level override is keyed by its `ref`
/// (`descendants[<ref>]`): it follows the instance from the set (or an
/// earlier variant) to `master_id`.
fn move_root_override(reference: &mut RefNode, master_id: &str) {
    if reference.target == master_id {
        return;
    }
    if let Some(descendants) = reference.descendants.as_mut() {
        if let Some(root) = descendants.remove(&reference.target) {
            descendants.insert(master_id.to_string(), root);
        }
    }
}

/// After set `set_id` changed: move every instance of its variants onto
/// the master its combination now resolves to. `old_masters` holds the
/// masters as they were (for removed ones and override remapping);
/// `rewrite` adjusts an instance's requested values first (renames,
/// removed values).
pub(crate) fn reresolve_instances(
    doc: &mut PenDocument,
    set_id: &str,
    old_masters: &HashMap<String, PenNode>,
    rewrite: &mut dyn FnMut(&mut VariantValues) -> bool,
    report: &mut VariantReport,
) {
    let Some(set_node) = find(doc, set_id).cloned() else {
        return;
    };
    let Some(set) = component_set_of(&set_node).cloned() else {
        return;
    };
    let masters: HashMap<String, PenNode> = super::pick::variant_masters(&set_node)
        .into_iter()
        .map(|master| (master.id_str().to_string(), master.clone()))
        .collect();
    let mut visit = |reference: &mut RefNode| {
        let in_set = reference.target == set_id
            || masters.contains_key(&reference.target)
            || old_masters.contains_key(&reference.target);
        if !in_set {
            return;
        }
        // A `ref` still naming the set shows the variant it picks.
        let picked = (reference.target == set_id)
            .then(|| {
                let values = full_values(&set, reference.variant.as_ref(), None);
                pick_variant(&set_node, &values).map(|pick| pick.master.clone())
            })
            .flatten();
        let old_master = masters
            .get(&reference.target)
            .or_else(|| old_masters.get(&reference.target))
            .or(picked.as_ref());
        let mut requested = reference
            .variant
            .clone()
            .or_else(|| old_master.and_then(variant_values_of).cloned())
            .unwrap_or_default();
        if rewrite(&mut requested) {
            report.moved_to_default.push(reference.base.id.clone());
        }
        let values = full_values(&set, Some(&requested), None);
        let Some(pick) = pick_variant(&set_node, &values) else {
            return;
        };
        if !pick.exact && reference.variant.is_some() {
            report.fallbacks.push(VariantFallback {
                instance: reference.base.id.clone(),
                requested: values.clone(),
                shown: pick.master.id_str().to_string(),
            });
        }
        let keep_variant = reference.variant.is_some() || pick.master.id_str() != reference.target;
        let values = if keep_variant {
            values
        } else {
            VariantValues::new()
        };
        for key in super::switch::retarget(reference, old_master, pick.master, values) {
            report.dropped_overrides.push(DroppedOverride {
                instance: reference.base.id.clone(),
                key,
            });
        }
    };
    for_each_ref_mut(doc, &mut visit);
}
