//! Component-set writes: create a set, add a variant or an axis, rename an
//! axis or a value, remove a value, switch an instance's variant. Each is
//! one undo step, checked against the document before anything changes.

use std::collections::HashMap;

use jian_ops_schema::component_set::{ComponentSet, VariantValues};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use super::create::{apply_create, check_name, layout_grid, taken_ids, unique_id, CreateSet};
use super::error::join;
use super::pick::{component_set_of, full_values, variant_masters, variant_values_of};
use super::report::{values_label, VariantReport};
use super::resolve::{reresolve_instances, resolve_target};
use super::tree::{find, find_mut, take};
use super::VariantError;
use crate::id_allocator::{IdAllocError, IdAllocator};
use crate::pen_node_ext::PenNodeExt;

/// A component-set write, carried by [`crate::EditorCommand::ComponentSet`].
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentSetCommand {
    /// Combine masters into a new set.
    Create(CreateSet),
    /// Add a variant: a copy of `from` (default: the set's default
    /// variant) with `values`; values new to an axis are appended to it.
    AddVariant {
        set_id: String,
        values: VariantValues,
        from: Option<String>,
        /// The new master's id; a fresh one from the values when `None`.
        new_id: Option<String>,
    },
    /// Add an axis; every variant takes `value` on it.
    AddAxis {
        set_id: String,
        axis: String,
        value: String,
    },
    /// Rename an axis on the set, its variants and every instance.
    RenameAxis {
        set_id: String,
        from: String,
        to: String,
    },
    /// Rename a value on the set, its variants and every instance.
    RenameValue {
        set_id: String,
        axis: String,
        from: String,
        to: String,
    },
    /// Drop a value and the variants that carry it; instances using it
    /// move to the axis default.
    RemoveValue {
        set_id: String,
        axis: String,
        value: String,
    },
    /// Switch an instance to another variant; axes left out keep their
    /// current value.
    SetInstanceVariant {
        node_id: String,
        variant: VariantValues,
    },
}

fn set_of<'a>(
    doc: &'a PenDocument,
    set_id: &str,
) -> Result<(&'a PenNode, &'a ComponentSet), VariantError> {
    let node = find(doc, set_id).ok_or_else(|| VariantError::SetNotFound(set_id.to_string()))?;
    let set =
        component_set_of(node).ok_or_else(|| VariantError::SetNotFound(set_id.to_string()))?;
    Ok((node, set))
}

fn unknown_axis(set: &ComponentSet, axis: &str) -> VariantError {
    VariantError::UnknownAxis {
        axis: axis.to_string(),
        allowed: join(set.axes.names()),
    }
}

fn axis_values<'a>(set: &'a ComponentSet, axis: &str) -> Result<&'a Vec<String>, VariantError> {
    set.axes.values(axis).ok_or_else(|| unknown_axis(set, axis))
}

impl ComponentSetCommand {
    /// Check the command against the document without changing it.
    pub fn check(&self, doc: &PenDocument) -> Result<(), VariantError> {
        match self {
            Self::Create(create) => create.plan(doc).map(|_| ()),
            Self::AddVariant {
                set_id,
                values,
                from,
                new_id,
            } => {
                let (node, set) = set_of(doc, set_id)?;
                let source = source_master(node, set, from.as_deref())?;
                for (axis, value) in values {
                    axis_values(set, axis)?;
                    check_name(value)?;
                }
                let full = new_variant_values(set, values, variant_values_of(source));
                if variant_masters(node)
                    .iter()
                    .any(|master| variant_values_of(master) == Some(&full))
                {
                    return Err(VariantError::DuplicateVariant {
                        values: values_label(&full),
                    });
                }
                if let Some(id) = new_id {
                    if find(doc, id).is_some() {
                        return Err(VariantError::DuplicateMaster(id.clone()));
                    }
                }
                Ok(())
            }
            Self::AddAxis {
                set_id,
                axis,
                value,
            } => {
                let (_, set) = set_of(doc, set_id)?;
                check_name(axis)?;
                check_name(value)?;
                if set.axes.values(axis).is_some() {
                    return Err(VariantError::AxisTaken(axis.clone()));
                }
                Ok(())
            }
            Self::RenameAxis { set_id, from, to } => {
                let (_, set) = set_of(doc, set_id)?;
                axis_values(set, from)?;
                check_name(to)?;
                if from != to && set.axes.values(to).is_some() {
                    return Err(VariantError::AxisTaken(to.clone()));
                }
                Ok(())
            }
            Self::RenameValue {
                set_id,
                axis,
                from,
                to,
            } => {
                let (_, set) = set_of(doc, set_id)?;
                let values = axis_values(set, axis)?;
                if !values.contains(from) {
                    return Err(VariantError::UnknownValue {
                        axis: axis.clone(),
                        value: from.clone(),
                        allowed: join(values),
                    });
                }
                check_name(to)?;
                if from != to && values.contains(to) {
                    return Err(VariantError::ValueTaken {
                        axis: axis.clone(),
                        value: to.clone(),
                    });
                }
                Ok(())
            }
            Self::RemoveValue {
                set_id,
                axis,
                value,
            } => {
                let (node, set) = set_of(doc, set_id)?;
                let values = axis_values(set, axis)?;
                if !values.contains(value) {
                    return Err(VariantError::UnknownValue {
                        axis: axis.clone(),
                        value: value.clone(),
                        allowed: join(values),
                    });
                }
                let survivors = variant_masters(node)
                    .iter()
                    .filter(|master| {
                        variant_values_of(master).and_then(|v| v.get(axis)) != Some(value)
                    })
                    .count();
                if values.len() < 2 || survivors == 0 {
                    return Err(VariantError::LastValue { axis: axis.clone() });
                }
                Ok(())
            }
            Self::SetInstanceVariant { node_id, variant } => {
                let Some(PenNode::Ref(reference)) = find(doc, node_id) else {
                    return Err(VariantError::InstanceNotFound(node_id.clone()));
                };
                match resolve_target(doc, &reference.target, Some(variant))? {
                    Some(_) => Ok(()),
                    None => Err(VariantError::NotAVariant(node_id.clone())),
                }
            }
        }
    }

    /// The set this command writes to, when it names one.
    pub fn set_id(&self) -> Option<&str> {
        match self {
            Self::Create(create) => create.set_id.as_deref(),
            Self::AddVariant { set_id, .. }
            | Self::AddAxis { set_id, .. }
            | Self::RenameAxis { set_id, .. }
            | Self::RenameValue { set_id, .. }
            | Self::RemoveValue { set_id, .. } => Some(set_id),
            Self::SetInstanceVariant { .. } => None,
        }
    }
}

/// The master `from` names (it must be a variant of the set), else the
/// set's default variant.
fn source_master<'a>(
    node: &'a PenNode,
    set: &ComponentSet,
    from: Option<&str>,
) -> Result<&'a PenNode, VariantError> {
    let masters = variant_masters(node);
    match from {
        Some(id) => masters
            .into_iter()
            .find(|master| master.id_str() == id)
            .ok_or_else(|| VariantError::MasterNotFound(id.to_string())),
        None => {
            let default = full_values(set, None, None);
            super::pick::pick_variant(node, &default)
                .map(|pick| pick.master)
                .ok_or(VariantError::NoMasters)
        }
    }
}

/// A new variant's full values: the call's, then the source's, then the
/// defaults (values new to an axis count).
fn new_variant_values(
    set: &ComponentSet,
    values: &VariantValues,
    source: Option<&VariantValues>,
) -> VariantValues {
    set.axes
        .names()
        .filter_map(|axis| {
            let value = values
                .get(axis)
                .or_else(|| source.and_then(|source| source.get(axis)))
                .cloned()
                .or_else(|| set.default_value(axis).map(str::to_string))?;
            Some((axis.to_string(), value))
        })
        .collect()
}

fn set_mut<'a>(
    doc: &'a mut PenDocument,
    set_id: &str,
) -> Option<&'a mut jian_ops_schema::node::FrameNode> {
    match find_mut(doc, set_id) {
        Some(PenNode::Frame(frame)) if frame.component_set.is_some() => Some(frame),
        _ => None,
    }
}

/// Every variant master of the set, as it is now, by id.
fn masters_by_id(doc: &PenDocument, set_id: &str) -> HashMap<String, PenNode> {
    find(doc, set_id)
        .map(|node| {
            variant_masters(node)
                .into_iter()
                .map(|master| (master.id_str().to_string(), master.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Rewrite every variant's values with `edit`, renaming a master whose
/// name spelled its values.
fn edit_master_values(
    doc: &mut PenDocument,
    set_id: &str,
    edit: &mut dyn FnMut(&mut VariantValues),
) {
    fn walk(nodes: &mut [PenNode], set: &ComponentSet, edit: &mut dyn FnMut(&mut VariantValues)) {
        for node in nodes {
            match node {
                PenNode::Frame(master) if master.reusable == Some(true) => {
                    let mut values = master.variant_values.clone().unwrap_or_default();
                    let spelled = master
                        .base
                        .name
                        .as_deref()
                        .is_none_or(|name| super::pick::parse_variant_name(name).is_some());
                    edit(&mut values);
                    if spelled {
                        master.base.name = Some(super::pick::variant_name(set, &values));
                    }
                    master.variant_values = Some(values);
                }
                PenNode::Ref(_) => {}
                other if component_set_of(other).is_none() => {
                    if let Some(children) = other.children_mut() {
                        walk(children, set, edit);
                    }
                }
                _ => {}
            }
        }
    }
    let Some(frame) = set_mut(doc, set_id) else {
        return;
    };
    let set = frame.component_set.clone().unwrap_or_default();
    if let Some(children) = frame.children.as_mut() {
        walk(children, &set, edit);
    }
}

/// Apply a set write to the document. `Err` leaves it untouched.
pub(crate) fn apply_to_document(
    doc: &mut PenDocument,
    command: &ComponentSetCommand,
    allocator: &mut dyn IdAllocator,
) -> Result<Result<VariantReport, VariantError>, IdAllocError> {
    if let Err(error) = command.check(doc) {
        return Ok(Err(error));
    }
    let mut report = VariantReport::default();
    match command {
        ComponentSetCommand::Create(create) => {
            let planned = match create.plan(doc) {
                Ok(planned) => planned,
                Err(error) => return Ok(Err(error)),
            };
            report.set_id = Some(planned.set_id.clone());
            apply_create(doc, create, planned);
        }
        ComponentSetCommand::AddVariant {
            set_id,
            values,
            from,
            new_id,
        } => {
            report.set_id = Some(set_id.clone());
            let (copy, full, ordered) = {
                let (node, set) = set_of(doc, set_id).expect("checked");
                let source = source_master(node, set, from.as_deref()).expect("checked");
                let full = new_variant_values(set, values, variant_values_of(source));
                let ordered: Vec<String> = set
                    .axes
                    .names()
                    .filter_map(|axis| full.get(axis).cloned())
                    .collect();
                (source.clone(), full, ordered)
            };
            let mut taken = crate::id_allocator::collect_document_ids(doc);
            let mut master =
                crate::walkers::deep_clone_with_allocator(&copy, allocator, &mut taken)?;
            remap_property_binds(&copy, &mut master);
            let id = new_id.clone().unwrap_or_else(|| {
                let base = format!("{set_id}-{}", ordered.join("-"));
                unique_id(&base, &taken_ids(doc))
            });
            let old_masters = masters_by_id(doc, set_id);
            let Some(frame) = set_mut(doc, set_id) else {
                return Ok(Ok(report));
            };
            let set = frame.component_set.as_mut().expect("set");
            for (axis, value) in &full {
                if let Some(list) = set.axes.values_mut(axis) {
                    if !list.contains(value) {
                        list.push(value.clone());
                    }
                }
            }
            let name = super::pick::variant_name(set, &full);
            if let PenNode::Frame(new_frame) = &mut master {
                new_frame.base.id = id.clone();
                new_frame.base.name = Some(name);
                new_frame.variant_values = Some(full);
            }
            frame.children.get_or_insert_with(Vec::new).push(master);
            let mut node = PenNode::Frame(frame.clone());
            layout_grid(&mut node);
            if let PenNode::Frame(laid_out) = node {
                *frame = laid_out;
            }
            report.created.push(id);
            reresolve_instances(doc, set_id, &old_masters, &mut |_| false, &mut report);
        }
        ComponentSetCommand::AddAxis {
            set_id,
            axis,
            value,
        } => {
            report.set_id = Some(set_id.clone());
            if let Some(frame) = set_mut(doc, set_id) {
                let set = frame.component_set.as_mut().expect("set");
                set.axes.0.push((axis.clone(), vec![value.clone()]));
            }
            edit_master_values(doc, set_id, &mut |values| {
                values.insert(axis.clone(), value.clone());
            });
            let mut instances = |reference: &mut jian_ops_schema::node::RefNode| {
                if let Some(variant) = reference.variant.as_mut() {
                    variant.entry(axis.clone()).or_insert_with(|| value.clone());
                }
            };
            super::tree::for_each_ref_mut(doc, &mut instances);
        }
        ComponentSetCommand::RenameAxis { set_id, from, to } => {
            report.set_id = Some(set_id.clone());
            if let Some(frame) = set_mut(doc, set_id) {
                let set = frame.component_set.as_mut().expect("set");
                for (name, _) in set.axes.0.iter_mut() {
                    if name == from {
                        *name = to.clone();
                    }
                }
                if let Some(value) = set.default.remove(from) {
                    set.default.insert(to.clone(), value);
                }
            }
            let mut rename = |values: &mut VariantValues| {
                if let Some(value) = values.remove(from) {
                    values.insert(to.clone(), value);
                }
            };
            edit_master_values(doc, set_id, &mut rename);
            rename_in_instances(doc, set_id, &mut rename);
        }
        ComponentSetCommand::RenameValue {
            set_id,
            axis,
            from,
            to,
        } => {
            report.set_id = Some(set_id.clone());
            if let Some(frame) = set_mut(doc, set_id) {
                let set = frame.component_set.as_mut().expect("set");
                for value in set.axes.values_mut(axis).into_iter().flatten() {
                    if value == from {
                        *value = to.clone();
                    }
                }
                if set.default.get(axis) == Some(from) {
                    set.default.insert(axis.clone(), to.clone());
                }
            }
            let mut rename = |values: &mut VariantValues| {
                if values.get(axis) == Some(from) {
                    values.insert(axis.clone(), to.clone());
                }
            };
            edit_master_values(doc, set_id, &mut rename);
            rename_in_instances(doc, set_id, &mut rename);
        }
        ComponentSetCommand::RemoveValue {
            set_id,
            axis,
            value,
        } => {
            report.set_id = Some(set_id.clone());
            let old_masters = masters_by_id(doc, set_id);
            let doomed: Vec<String> = old_masters
                .values()
                .filter(|master| variant_values_of(master).and_then(|v| v.get(axis)) == Some(value))
                .map(|master| master.id_str().to_string())
                .collect();
            for id in &doomed {
                take(doc, id);
            }
            report.removed = doomed;
            report.removed.sort();
            let mut fallback = String::new();
            if let Some(frame) = set_mut(doc, set_id) {
                let set = frame.component_set.as_mut().expect("set");
                if let Some(list) = set.axes.values_mut(axis) {
                    list.retain(|v| v != value);
                }
                if set.default.get(axis) == Some(value) {
                    set.default.remove(axis);
                }
                fallback = set.default_value(axis).unwrap_or_default().to_string();
                let mut node = PenNode::Frame(frame.clone());
                layout_grid(&mut node);
                if let PenNode::Frame(laid_out) = node {
                    *frame = laid_out;
                }
            }
            reresolve_instances(
                doc,
                set_id,
                &old_masters,
                &mut |values| {
                    if values.get(axis) == Some(value) {
                        values.insert(axis.clone(), fallback.clone());
                        true
                    } else {
                        false
                    }
                },
                &mut report,
            );
        }
        ComponentSetCommand::SetInstanceVariant { node_id, variant } => {
            let (target, old_master) = {
                let Some(PenNode::Ref(reference)) = find(doc, node_id) else {
                    return Ok(Ok(report));
                };
                let mut requested = reference.variant.clone().unwrap_or_default();
                requested.extend(variant.clone());
                let target = resolve_target(doc, &reference.target, Some(&requested))
                    .ok()
                    .flatten();
                (
                    target,
                    super::resolve::shown_master(doc, reference).cloned(),
                )
            };
            let Some(target) = target else {
                return Ok(Ok(report));
            };
            report.set_id = Some(target.set_id.clone());
            let Some(new_master) = find(doc, &target.master_id).cloned() else {
                return Ok(Ok(report));
            };
            if !target.exact {
                report.fallbacks.push(super::report::VariantFallback {
                    instance: node_id.clone(),
                    requested: target.values.clone(),
                    shown: target.master_id.clone(),
                });
            }
            if let Some(PenNode::Ref(reference)) = find_mut(doc, node_id) {
                let old = old_master.filter(|old| component_set_of(old).is_none());
                for key in
                    super::switch::retarget(reference, old.as_ref(), &new_master, target.values)
                {
                    report
                        .dropped_overrides
                        .push(super::report::DroppedOverride {
                            instance: node_id.clone(),
                            key,
                        });
                }
            }
        }
    }
    Ok(Ok(report))
}

/// Apply `rename` to the `variant` of every instance of the set.
fn rename_in_instances(
    doc: &mut PenDocument,
    set_id: &str,
    rename: &mut dyn FnMut(&mut VariantValues),
) {
    let masters = masters_by_id(doc, set_id);
    super::tree::for_each_ref_mut(doc, &mut |reference| {
        if reference.target == set_id || masters.contains_key(&reference.target) {
            if let Some(variant) = reference.variant.as_mut() {
                rename(variant);
            }
        }
    });
}

/// A copied master's property bindings name the copy's nodes.
fn remap_property_binds(source: &PenNode, copy: &mut PenNode) {
    fn zip(left: &PenNode, right: &PenNode, out: &mut HashMap<String, String>) {
        out.insert(left.id_str().to_string(), right.id_str().to_string());
        if let (Some(left), Some(right)) = (left.children(), right.children()) {
            for (left, right) in left.iter().zip(right) {
                zip(left, right, out);
            }
        }
    }
    let mut ids = HashMap::new();
    zip(source, copy, &mut ids);
    if let PenNode::Frame(frame) = copy {
        for property in frame
            .component_properties
            .iter_mut()
            .flat_map(|p| p.values_mut())
        {
            for binding in &mut property.bind {
                if let Some(new_id) = ids.get(&binding.node) {
                    binding.node = new_id.clone();
                }
            }
        }
    }
}

/// Run `command` on a copy of `doc`: the document it would leave and its
/// report. MCP answers with these before the host applies the command.
pub fn preview_command(
    doc: &PenDocument,
    command: &ComponentSetCommand,
) -> Result<(PenDocument, VariantReport), VariantError> {
    let mut copy = doc.clone();
    let mut allocator = crate::id_allocator::SequentialIdAllocator::for_document(doc, 1)
        .map_err(|_| VariantError::NoMasters)?;
    match apply_to_document(&mut copy, command, &mut allocator) {
        Ok(Ok(report)) => Ok((copy, report)),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(VariantError::NoMasters),
    }
}
