//! What the property panel shows for variants, and the panel's writes.
//!
//! - An instance of a variant: one dropdown per axis.
//! - A variant master: its value per axis (a field renames the value
//!   across the set) and "Add variant".
//! - A set: its axis names (a field renames the axis), "Add variant" and
//!   "Add axis".
//! - Two or more masters outside any set: "Combine as variants".
//!
//! Every write goes through [`ComponentSetCommand`], one undo step each,
//! and returns the [`VariantReport`] the panel raises as a toast.

use jian_ops_schema::component_set::VariantValues;
use jian_ops_schema::node::PenNode;

use super::pick::{component_set_of, variant_masters, variant_values_of};
use super::{ComponentSetCommand, CreateSet, VariantError, VariantReport};
use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;

/// One axis row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelVariantAxis {
    pub name: String,
    pub values: Vec<String>,
    /// The instance's or master's value on this axis (blank when a master
    /// leaves the axis out).
    pub current: String,
}

/// The variant rows of the selection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PanelVariants {
    #[default]
    None,
    /// An instance of a variant. `fallback`: no master has the
    /// combination, the nearest one shows.
    Instance {
        axes: Vec<PanelVariantAxis>,
        fallback: bool,
    },
    /// A variant master inside a set.
    Master {
        set_id: String,
        axes: Vec<PanelVariantAxis>,
    },
    /// A component set.
    Set {
        set_id: String,
        axes: Vec<PanelVariantAxis>,
    },
    /// Masters that can be combined into a set.
    Combine { master_ids: Vec<String> },
}

impl PanelVariants {
    /// The axis rows, if any.
    pub fn axes(&self) -> &[PanelVariantAxis] {
        match self {
            Self::Instance { axes, .. } | Self::Master { axes, .. } | Self::Set { axes, .. } => {
                axes
            }
            _ => &[],
        }
    }
}

fn axis_rows(
    set: &jian_ops_schema::component_set::ComponentSet,
    values: Option<&VariantValues>,
) -> Vec<PanelVariantAxis> {
    set.axes
        .iter()
        .map(|(name, list)| PanelVariantAxis {
            name: name.to_string(),
            values: list.to_vec(),
            current: values
                .and_then(|values| values.get(name))
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

impl EditorState {
    /// The variant rows for the current selection.
    pub fn panel_variants(&self) -> PanelVariants {
        if self.selection_count() > 1 {
            return self.combinable_selection();
        }
        let Some(node) = self.selected_node() else {
            return PanelVariants::None;
        };
        match node {
            PenNode::Ref(_) => {
                let Some((_, target)) = self.selected_variant_instance() else {
                    return PanelVariants::None;
                };
                let Some(set) =
                    super::tree::find(&self.doc, &target.set_id).and_then(component_set_of)
                else {
                    return PanelVariants::None;
                };
                PanelVariants::Instance {
                    axes: axis_rows(set, Some(&target.values)),
                    fallback: !target.exact,
                }
            }
            node if component_set_of(node).is_some() => PanelVariants::Set {
                set_id: node.id_str().to_string(),
                axes: axis_rows(component_set_of(node).expect("set"), None),
            },
            PenNode::Frame(frame) if frame.reusable == Some(true) => {
                match self.set_holding(node.id_str()) {
                    Some(set_node) => PanelVariants::Master {
                        set_id: set_node.id_str().to_string(),
                        axes: axis_rows(
                            component_set_of(set_node).expect("set"),
                            variant_values_of(node),
                        ),
                    },
                    None => PanelVariants::None,
                }
            }
            _ => PanelVariants::None,
        }
    }

    fn combinable_selection(&self) -> PanelVariants {
        let mut ids = Vec::new();
        for id in &self.selection.set {
            match super::tree::find(&self.doc, id.as_str()) {
                Some(PenNode::Frame(frame))
                    if frame.reusable == Some(true) && self.set_holding(id.as_str()).is_none() =>
                {
                    ids.push(id.as_str().to_string())
                }
                _ => return PanelVariants::None,
            }
        }
        if ids.len() >= 2 {
            PanelVariants::Combine { master_ids: ids }
        } else {
            PanelVariants::None
        }
    }

    /// Show value `value` of axis `axis` on the selected instance.
    pub fn set_selected_instance_variant(
        &mut self,
        axis: usize,
        value: usize,
    ) -> Option<Result<VariantReport, VariantError>> {
        let PanelVariants::Instance { axes, .. } = self.panel_variants() else {
            return None;
        };
        let row = axes.get(axis)?;
        let value = row.values.get(value)?.clone();
        if value == row.current {
            return None;
        }
        let command = ComponentSetCommand::SetInstanceVariant {
            node_id: self.selection.anchor.as_str().to_string(),
            variant: VariantValues::from([(row.name.clone(), value)]),
        };
        Some(self.apply_component_set(&command))
    }

    /// Combine the selected masters into a set named after the first.
    pub fn combine_selected_as_variants(&mut self) -> Option<Result<VariantReport, VariantError>> {
        let PanelVariants::Combine { master_ids } = self.panel_variants() else {
            return None;
        };
        let name = super::tree::find(&self.doc, &master_ids[0])
            .and_then(|node| node.base().name.clone())
            .map(|name| match super::pick::parse_variant_name(&name) {
                Some(_) => "Component set".to_string(),
                None => name,
            })
            .unwrap_or_else(|| "Component set".to_string());
        let command = ComponentSetCommand::Create(CreateSet {
            name,
            master_ids,
            ..Default::default()
        });
        Some(self.apply_component_set(&command))
    }

    /// Add a variant to the selected set (or the set of the selected
    /// master): a copy of the selected master (else the default one) with a
    /// new value on the first axis.
    pub fn add_variant_to_selected_set(&mut self) -> Option<Result<VariantReport, VariantError>> {
        let (set_id, from) = match self.panel_variants() {
            PanelVariants::Set { set_id, .. } => (set_id, None),
            PanelVariants::Master { set_id, .. } => {
                (set_id, Some(self.selection.anchor.as_str().to_string()))
            }
            _ => return None,
        };
        let set_node = super::tree::find(&self.doc, &set_id)?;
        let set = component_set_of(set_node)?;
        let (axis, list) = set.axes.iter().next()?;
        let value = (list.len() + 1..)
            .map(|n| format!("{axis}{n}"))
            .find(|candidate| !list.contains(candidate))?;
        let command = ComponentSetCommand::AddVariant {
            set_id,
            values: VariantValues::from([(axis.to_string(), value)]),
            from,
            new_id: None,
        };
        Some(self.apply_component_set(&command))
    }

    /// Add an axis to the selected set; every variant takes `default`.
    pub fn add_axis_to_selected_set(&mut self) -> Option<Result<VariantReport, VariantError>> {
        let PanelVariants::Set { set_id, axes } = self.panel_variants() else {
            return None;
        };
        let axis = (axes.len() + 1..)
            .map(|n| format!("property{n}"))
            .find(|candidate| !axes.iter().any(|row| &row.name == candidate))?;
        let command = ComponentSetCommand::AddAxis {
            set_id,
            axis,
            value: "default".to_string(),
        };
        Some(self.apply_component_set(&command))
    }

    /// Commit a typed name: on a set, axis `index`'s new name; on a
    /// variant master, its value on axis `index` renamed across the set.
    pub fn commit_selected_variant_text(
        &mut self,
        index: usize,
        draft: &str,
    ) -> Option<Result<VariantReport, VariantError>> {
        let draft = draft.trim().to_string();
        let command = match self.panel_variants() {
            PanelVariants::Set { set_id, axes } => {
                let row = axes.get(index)?;
                if draft.is_empty() || draft == row.name {
                    return None;
                }
                ComponentSetCommand::RenameAxis {
                    set_id,
                    from: row.name.clone(),
                    to: draft,
                }
            }
            PanelVariants::Master { set_id, axes } => {
                let row = axes.get(index)?;
                if draft.is_empty() || draft == row.current || row.current.is_empty() {
                    return None;
                }
                ComponentSetCommand::RenameValue {
                    set_id,
                    axis: row.name.clone(),
                    from: row.current.clone(),
                    to: draft,
                }
            }
            _ => return None,
        };
        Some(self.apply_component_set(&command))
    }

    /// The names of a set's variants, for a set frame's layer label.
    pub fn variant_count(&self, set_id: &str) -> usize {
        super::tree::find(&self.doc, set_id)
            .map(|node| variant_masters(node).len())
            .unwrap_or(0)
    }
}
