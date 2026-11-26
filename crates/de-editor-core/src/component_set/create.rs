//! Combining masters into a component set ("Combine as variants").
//!
//! Each master's values come from the call, else from its `key=value`
//! name, else (one axis given with a value per master) from its position;
//! when no master says anything the set gets one `variant` axis with the
//! masters' names as values. The set frame takes the first master's place
//! and lays its variants out as a grid: the first axis runs across, the
//! other axes' combinations down.

use std::collections::HashSet;

use jian_ops_schema::component_set::{ComponentSet, VariantAxes, VariantValues};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::PenDocument;
use serde_json::json;

use super::pick::{component_set_of, parse_variant_name, set_containing, variant_masters};
use super::report::values_label;
use super::tree::{find, place_of, siblings_mut, take};
use super::VariantError;
use crate::pen_node_ext::PenNodeExt;

/// The default axis when the masters' names say nothing.
pub const DEFAULT_AXIS: &str = "variant";
/// Space around and between variants in a set's grid.
const GRID_PAD: f64 = 24.0;
const GRID_GAP: f64 = 24.0;
/// Figma's component-set outline.
const SET_STROKE: &str = "#9747FF";

/// Combine masters into a new set.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CreateSet {
    pub name: String,
    pub master_ids: Vec<String>,
    /// Axes in order with their values; inferred when `None`.
    pub axes: Option<Vec<(String, Vec<String>)>>,
    /// Each master's values, in `master_ids` order; inferred when `None`.
    pub values: Option<Vec<VariantValues>>,
    /// The set frame's id; a fresh one from the name when `None`.
    pub set_id: Option<String>,
}

/// A checked create.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSet {
    pub set_id: String,
    pub set: ComponentSet,
    /// Each master's values, in `master_ids` order.
    pub values: Vec<VariantValues>,
}

/// An axis or value name a set accepts.
pub(crate) fn check_name(name: &str) -> Result<(), VariantError> {
    if name.trim().is_empty() || name.contains([',', '=']) || name.trim() != name {
        Err(VariantError::InvalidName(name.to_string()))
    } else {
        Ok(())
    }
}

/// `base` made unique against `taken` with `-2`, `-3`, ….
pub(crate) fn unique_id(base: &str, taken: &HashSet<String>) -> String {
    let slug: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() {
        "set".to_string()
    } else {
        slug
    };
    if !taken.contains(&slug) {
        return slug;
    }
    (2..)
        .map(|n| format!("{slug}-{n}"))
        .find(|id| !taken.contains(id))
        .unwrap_or(slug)
}

pub(crate) fn taken_ids(doc: &PenDocument) -> HashSet<String> {
    crate::id_allocator::collect_document_ids(doc)
        .into_iter()
        .map(|id| id.as_str().to_string())
        .collect()
}

impl CreateSet {
    /// Check the create against the document.
    pub fn plan(&self, doc: &PenDocument) -> Result<PlannedSet, VariantError> {
        if self.master_ids.is_empty() {
            return Err(VariantError::NoMasters);
        }
        let mut seen = HashSet::new();
        let mut masters = Vec::new();
        for id in &self.master_ids {
            if !seen.insert(id.as_str()) {
                return Err(VariantError::DuplicateMaster(id.clone()));
            }
            let master = match find(doc, id) {
                Some(node @ PenNode::Frame(frame)) if frame.reusable == Some(true) => node,
                _ => return Err(VariantError::MasterNotFound(id.clone())),
            };
            if let Some(set) = set_containing(doc, id) {
                return Err(VariantError::AlreadyInSet {
                    master: id.clone(),
                    set: set.id_str().to_string(),
                });
            }
            masters.push(master);
        }
        let values = self.master_values(&masters)?;
        // Axis order: as given, else as the masters' names first spell it
        // (`variant=…, size=…` orders variant before size).
        let mut name_order: Vec<String> = Vec::new();
        for master in &masters {
            let pairs = master.base().name.as_deref().and_then(parse_variant_name);
            for (axis, _) in pairs.into_iter().flatten() {
                if !name_order.contains(&axis) {
                    name_order.push(axis);
                }
            }
        }
        let axes = self.axes_for(&values, name_order)?;
        let mut combos = HashSet::new();
        for entry in &values {
            if !combos.insert(entry.clone()) {
                return Err(VariantError::DuplicateVariant {
                    values: values_label(entry),
                });
            }
        }
        let default = complete(&axes, values.first());
        let set_id = match &self.set_id {
            Some(id) => id.clone(),
            None => unique_id(&self.name, &taken_ids(doc)),
        };
        Ok(PlannedSet {
            set_id,
            set: ComponentSet { axes, default },
            values,
        })
    }

    fn master_values(&self, masters: &[&PenNode]) -> Result<Vec<VariantValues>, VariantError> {
        if let Some(values) = &self.values {
            if values.len() != masters.len() {
                return Err(VariantError::MissingValues {
                    master: masters[values.len().min(masters.len() - 1)]
                        .id_str()
                        .to_string(),
                });
            }
            return Ok(values.clone());
        }
        let parsed: Vec<Option<Vec<(String, String)>>> = masters
            .iter()
            .map(|master| master.base().name.as_deref().and_then(parse_variant_name))
            .collect();
        let positional = match self.axes.as_deref() {
            Some([(axis, values)]) if values.len() == masters.len() => Some((axis, values)),
            _ => None,
        };
        if parsed.iter().all(Option::is_none) && self.axes.is_none() {
            // Nothing to read: one axis whose values are the masters' names.
            let mut used = HashSet::new();
            return Ok(masters
                .iter()
                .map(|master| {
                    let name = master
                        .base()
                        .name
                        .clone()
                        .filter(|name| check_name(name).is_ok() && !used.contains(name))
                        .unwrap_or_else(|| master.id_str().to_string());
                    used.insert(name.clone());
                    VariantValues::from([(DEFAULT_AXIS.to_string(), name)])
                })
                .collect());
        }
        masters
            .iter()
            .zip(parsed)
            .enumerate()
            .map(|(index, (master, parsed))| match (parsed, positional) {
                (Some(pairs), _) => Ok(pairs.into_iter().collect()),
                (None, Some((axis, values))) => {
                    Ok(VariantValues::from([(axis.clone(), values[index].clone())]))
                }
                (None, None) => Err(VariantError::MissingValues {
                    master: master.id_str().to_string(),
                }),
            })
            .collect()
    }

    fn axes_for(
        &self,
        values: &[VariantValues],
        name_order: Vec<String>,
    ) -> Result<VariantAxes, VariantError> {
        let mut axes: Vec<(String, Vec<String>)> = match &self.axes {
            Some(axes) => axes.clone(),
            None => Vec::new(),
        };
        let explicit = self.axes.is_some();
        let mut order: Vec<String> = if explicit {
            axes.iter().map(|(axis, _)| axis.clone()).collect()
        } else {
            name_order
        };
        for entry in values {
            for (axis, value) in entry {
                check_name(axis)?;
                check_name(value)?;
                if !order.contains(axis) {
                    if explicit {
                        return Err(VariantError::UnknownAxis {
                            axis: axis.clone(),
                            allowed: super::error::join(axes.iter().map(|(a, _)| a)),
                        });
                    }
                    order.push(axis.clone());
                }
                match axes.iter_mut().find(|(name, _)| name == axis) {
                    Some((_, list)) if list.contains(value) => {}
                    Some((_, list)) if explicit => {
                        return Err(VariantError::UnknownValue {
                            axis: axis.clone(),
                            value: value.clone(),
                            allowed: super::error::join(list.iter()),
                        })
                    }
                    Some((_, list)) => list.push(value.clone()),
                    None => axes.push((axis.clone(), vec![value.clone()])),
                }
            }
        }
        for (axis, list) in &axes {
            check_name(axis)?;
            if list.is_empty() {
                return Err(VariantError::LastValue { axis: axis.clone() });
            }
            for value in list {
                check_name(value)?;
            }
        }
        axes.sort_by_key(|(axis, _)| order.iter().position(|name| name == axis));
        Ok(VariantAxes(axes))
    }
}

/// `values` with every axis named: missing axes take their first value.
fn complete(axes: &VariantAxes, values: Option<&VariantValues>) -> VariantValues {
    axes.iter()
        .filter_map(|(axis, list)| {
            let value = values
                .and_then(|values| values.get(axis))
                .or_else(|| list.first())?;
            Some((axis.to_string(), value.clone()))
        })
        .collect()
}

/// Build the set: masters move into a new set frame at the first
/// master's place. The plan must come from [`CreateSet::plan`] on `doc`.
pub(crate) fn apply_create(doc: &mut PenDocument, create: &CreateSet, planned: PlannedSet) {
    let Some(place) = place_of(doc, &create.master_ids[0]) else {
        return;
    };
    let (x, y) = find(doc, &create.master_ids[0])
        .map(|node| (node.base().x, node.base().y))
        .unwrap_or_default();
    let mut index = place.index;
    let mut masters = Vec::new();
    for id in &create.master_ids {
        if let Some(other) = place_of(doc, id) {
            if other.page == place.page && other.parent == place.parent && other.index < index {
                index -= 1;
            }
        }
        if let Some(mut master) = take(doc, id) {
            if let PenNode::Frame(frame) = &mut master {
                let values = planned.values[masters.len()].clone();
                frame.base.name = Some(super::pick::variant_name(&planned.set, &values));
                frame.variant_values = Some(values);
            }
            masters.push(master);
        }
    }
    let mut set_node: PenNode = match serde_json::from_value(json!({
        "type": "frame",
        "id": planned.set_id,
        "name": create.name.trim(),
        "layout": "none",
        "cornerRadius": 8,
        "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": SET_STROKE}]},
    })) {
        Ok(node) => node,
        Err(_) => return,
    };
    if let PenNode::Frame(frame) = &mut set_node {
        frame.base.x = x;
        frame.base.y = y;
        frame.component_set = Some(planned.set);
        frame.children = Some(masters);
    }
    layout_grid(&mut set_node);
    if let Some(siblings) = siblings_mut(doc, &place) {
        let at = index.min(siblings.len());
        siblings.insert(at, set_node);
    }
}

/// Lay a set's variants out as a grid and size the set around them.
pub fn layout_grid(set_node: &mut PenNode) {
    let Some(set) = component_set_of(set_node).cloned() else {
        return;
    };
    let ids: Vec<String> = variant_masters(set_node)
        .iter()
        .map(|master| master.id_str().to_string())
        .collect();
    let PenNode::Frame(frame) = set_node else {
        return;
    };
    let Some(children) = frame.children.as_mut() else {
        return;
    };
    let axes: Vec<(&str, &[String])> = set.axes.iter().collect();
    let position = |values: Option<&VariantValues>, axis: usize| -> usize {
        axes.get(axis)
            .and_then(|(name, list)| {
                let value = values?.get(*name)?;
                list.iter().position(|v| v == value)
            })
            .unwrap_or(0)
    };
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cells: Vec<(usize, Vec<usize>)> = Vec::new();
    let (mut cell_w, mut cell_h) = (0.0_f64, 0.0_f64);
    for child in children.iter() {
        if !ids.iter().any(|id| id == child.id_str()) {
            continue;
        }
        let values = super::pick::variant_values_of(child);
        let column = position(values, 0);
        let row: Vec<usize> = (1..axes.len()).map(|axis| position(values, axis)).collect();
        if !rows.contains(&row) {
            rows.push(row.clone());
        }
        cells.push((column, row));
        cell_w = cell_w.max(child.width_px().unwrap_or(100.0));
        cell_h = cell_h.max(child.height_px().unwrap_or(40.0));
    }
    rows.sort();
    let columns = axes.first().map_or(1, |(_, list)| list.len().max(1));
    let mut cell_iter = cells.into_iter();
    let mut taken: HashSet<(usize, usize)> = HashSet::new();
    for child in children.iter_mut() {
        if !ids.iter().any(|id| id == child.id_str()) {
            continue;
        }
        let Some((column, row)) = cell_iter.next() else {
            break;
        };
        let mut row_index = rows.iter().position(|r| *r == row).unwrap_or(0);
        // Two variants on one cell (an axis left out) stack below.
        while !taken.insert((column, row_index)) {
            row_index += rows.len();
        }
        let base = child.base_mut();
        base.x = Some(GRID_PAD + column as f64 * (cell_w + GRID_GAP));
        base.y = Some(GRID_PAD + row_index as f64 * (cell_h + GRID_GAP));
    }
    let row_count = taken.iter().map(|(_, row)| row + 1).max().unwrap_or(1);
    let width = GRID_PAD * 2.0 + columns as f64 * cell_w + (columns as f64 - 1.0) * GRID_GAP;
    let height = GRID_PAD * 2.0 + row_count as f64 * cell_h + (row_count as f64 - 1.0) * GRID_GAP;
    frame.container.width = Some(SizingBehavior::Number(width));
    frame.container.height = Some(SizingBehavior::Number(height));
}
