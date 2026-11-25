//! The property panel's view of component properties: an instance's inputs,
//! a master's list, and the "make this a property" offers on a node inside
//! a master. Every write goes through [`ComponentPropertyCommand`], so the
//! panel and MCP share one undo step and one set of checks.

use jian_ops_schema::component_props::{BindingField, ComponentPropertyType, PropertyBinding};
use jian_ops_schema::node::PenNode;
use serde_json::{Map, Value};

use super::check::{properties_of, PropsContext};
use super::{property_summaries, ComponentPropertyCommand, PropertyUpsert};
use crate::pen_node_ext::PenNodeExt;
use crate::EditorState;

/// What the panel shows for a single selection.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PanelProperties {
    #[default]
    None,
    /// An instance: one input per property of its master.
    Instance(Vec<PanelPropRow>),
    /// A master that declares properties: rename / remove rows.
    Master(Vec<PanelPropRow>),
    /// A node inside a master: the fields it can turn into a property.
    Bindable(Vec<BindingField>),
}

impl PanelProperties {
    /// Rows (or offers) the panel lays out.
    pub fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Instance(rows) | Self::Master(rows) => rows.len(),
            Self::Bindable(fields) => fields.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Row `index` of an instance or master list.
    pub fn row(&self, index: usize) -> Option<&PanelPropRow> {
        match self {
            Self::Instance(rows) | Self::Master(rows) => rows.get(index),
            _ => None,
        }
    }
}

/// One property row.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelPropRow {
    pub name: String,
    pub kind: ComponentPropertyType,
    /// The instance's value, else the default (the master's own value).
    pub value: Value,
    /// The instance sets its own value.
    pub set: bool,
    /// No binding reaches a node.
    pub unbound: bool,
    /// Instance properties: the components the picker cycles through
    /// (`preferred`, else every registered component).
    pub choices: Vec<String>,
    /// Instance properties: the shown component's name.
    pub component_name: Option<String>,
}

impl PanelPropRow {
    /// The value as the input shows it.
    pub fn display(&self) -> String {
        if let Some(name) = &self.component_name {
            return name.clone();
        }
        match &self.value {
            Value::String(text) => text.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        }
    }
}

impl EditorState {
    fn props_context(&self) -> PropsContext<'_> {
        PropsContext {
            doc: &self.doc,
            components: &self.components,
        }
    }

    /// The component-property rows for the current single selection.
    pub fn panel_properties(&self) -> PanelProperties {
        if self.selection_count() != 1 {
            return PanelProperties::None;
        }
        let Some(node) = self.selected_node() else {
            return PanelProperties::None;
        };
        let context = self.props_context();
        match node {
            PenNode::Ref(reference) => {
                let Ok(master) = context.master(&reference.target) else {
                    return PanelProperties::None;
                };
                let own = reference.props.as_ref();
                let rows = self.rows_for(master, |name| own.and_then(|props| props.get(name)));
                if rows.is_empty() {
                    PanelProperties::None
                } else {
                    PanelProperties::Instance(rows)
                }
            }
            PenNode::Frame(frame) if frame.component_properties.is_some() => {
                PanelProperties::Master(self.rows_for(node, |_| None))
            }
            _ => match self.enclosing_master(node.id_str()) {
                Some(master) => PanelProperties::Bindable(bindable_fields(master, node)),
                None => PanelProperties::None,
            },
        }
    }

    fn rows_for<'a>(
        &self,
        master: &PenNode,
        own: impl Fn(&str) -> Option<&'a Value>,
    ) -> Vec<PanelPropRow> {
        let all_components = || {
            self.components
                .sorted_options()
                .iter()
                .map(|option| option.id.clone())
                .collect::<Vec<_>>()
        };
        property_summaries(self.props_context(), master)
            .into_iter()
            .map(|summary| {
                let set = own(&summary.name).cloned();
                let unbound = summary.is_unbound();
                let choices = if summary.kind == ComponentPropertyType::Instance {
                    if summary.preferred.is_empty() {
                        all_components()
                    } else {
                        summary.preferred.clone()
                    }
                } else {
                    Vec::new()
                };
                let value = set
                    .clone()
                    .or(summary.default.clone())
                    .unwrap_or(Value::Null);
                let component_name = (summary.kind == ComponentPropertyType::Instance)
                    .then(|| value.as_str())
                    .flatten()
                    .and_then(|id| self.components.find_by_id(&crate::NodeId::new(id)))
                    .map(|component| component.name.clone());
                PanelPropRow {
                    value,
                    component_name,
                    set: set.is_some(),
                    unbound,
                    choices,
                    name: summary.name,
                    kind: summary.kind,
                }
            })
            .collect()
    }

    /// The nearest component master that holds `id` below its root. Walks
    /// the registered masters only, not the page: the panel asks every
    /// frame.
    fn enclosing_master(&self, id: &str) -> Option<&PenNode> {
        let node_id = crate::NodeId::new(id);
        let mut nearest: Option<&PenNode> = None;
        for component in &self.components.components {
            let Some(root) = self.components.resolved_root(&self.doc, &component.id) else {
                continue;
            };
            let inside = root
                .children()
                .is_some_and(|children| crate::walkers::contains_node(children, &node_id));
            // A master nested inside another is the nearer one.
            let nearer = nearest.is_none_or(|outer| {
                crate::walkers::descendant_contains(outer, &crate::NodeId::new(root.id_str()))
            });
            if inside && nearer {
                nearest = Some(root);
            }
        }
        nearest
    }

    fn selected_props_row(&self, index: usize) -> Option<(String, PanelPropRow)> {
        let row = self.panel_properties().row(index)?.clone();
        Some((self.selection.anchor.as_str().to_string(), row))
    }

    fn set_instance_value(&mut self, node_id: String, name: String, value: Value) -> bool {
        let command = ComponentPropertyCommand::SetInstanceProps {
            node_id,
            props: Map::from_iter([(name, value)]),
        };
        self.apply_component_property(&command).is_ok()
    }

    /// Flip a boolean property of the selected instance.
    pub fn toggle_selected_instance_prop(&mut self, index: usize) -> bool {
        let Some((id, row)) = self.selected_props_row(index) else {
            return false;
        };
        if row.kind != ComponentPropertyType::Boolean {
            return false;
        }
        let next = !row.value.as_bool().unwrap_or(true);
        self.set_instance_value(id, row.name, Value::Bool(next))
    }

    /// Show the next of an instance property's choices.
    pub fn cycle_selected_instance_prop(&mut self, index: usize) -> bool {
        let Some((id, row)) = self.selected_props_row(index) else {
            return false;
        };
        if row.kind != ComponentPropertyType::Instance || row.choices.is_empty() {
            return false;
        }
        let current = row.value.as_str().unwrap_or_default();
        let next = row
            .choices
            .iter()
            .position(|choice| choice == current)
            .map_or(0, |at| (at + 1) % row.choices.len());
        self.set_instance_value(id, row.name, Value::String(row.choices[next].clone()))
    }

    /// Commit a typed value: an instance's text or colour value (blank
    /// goes back to the default), or a master property's new name.
    pub fn commit_selected_prop_text(&mut self, index: usize, draft: &str) -> bool {
        let draft = draft.trim();
        let panel = self.panel_properties();
        let Some(row) = panel.row(index).cloned() else {
            return false;
        };
        let id = self.selection.anchor.as_str().to_string();
        match panel {
            PanelProperties::Master(_) => {
                if draft.is_empty() || draft == row.name {
                    return false;
                }
                let command = ComponentPropertyCommand::Upsert(PropertyUpsert {
                    component_id: id,
                    name: draft.to_string(),
                    rename_from: Some(row.name),
                    ..Default::default()
                });
                self.apply_component_property(&command).is_ok()
            }
            PanelProperties::Instance(_) => {
                if !matches!(
                    row.kind,
                    ComponentPropertyType::Text | ComponentPropertyType::Color
                ) {
                    return false;
                }
                let value = if draft.is_empty() {
                    Value::Null
                } else {
                    Value::String(draft.to_string())
                };
                if value == row.value && row.set {
                    return false;
                }
                self.set_instance_value(id, row.name, value)
            }
            _ => false,
        }
    }

    /// Drop a property of the selected master.
    pub fn remove_selected_component_property(&mut self, index: usize) -> bool {
        let PanelProperties::Master(rows) = self.panel_properties() else {
            return false;
        };
        let Some(row) = rows.get(index) else {
            return false;
        };
        let command = ComponentPropertyCommand::Remove {
            component_id: self.selection.anchor.as_str().to_string(),
            name: row.name.clone(),
        };
        self.apply_component_property(&command).is_ok()
    }

    /// Turn `field` of the selected node (inside a master) into a new
    /// property of that master, named after the node.
    pub fn bind_selected_to_new_property(&mut self, field: BindingField) -> bool {
        let Some(node) = self.selected_node() else {
            return false;
        };
        let Some(master) = self.enclosing_master(node.id_str()) else {
            return false;
        };
        if !bindable_fields(master, node).contains(&field) {
            return false;
        }
        let kind = match field {
            BindingField::Content => ComponentPropertyType::Text,
            BindingField::Visible => ComponentPropertyType::Boolean,
            BindingField::Ref => ComponentPropertyType::Instance,
            BindingField::Fill => ComponentPropertyType::Color,
            _ => return false,
        };
        let base = node
            .base()
            .name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| node.type_tag().to_string())
            .replace(['.', '/'], " ");
        let base = match field {
            BindingField::Visible => format!("Show {base}"),
            BindingField::Fill => format!("{base} color"),
            _ => base,
        };
        let taken = properties_of(master);
        let name = (1..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{base} {n}")
                }
            })
            .find(|name| taken.is_none_or(|taken| !taken.contains_key(name)))
            .unwrap_or(base);
        let command = ComponentPropertyCommand::Upsert(PropertyUpsert {
            component_id: master.id_str().to_string(),
            name,
            kind: Some(kind),
            bind: Some(vec![PropertyBinding {
                node: node.id_str().to_string(),
                field,
            }]),
            ..Default::default()
        });
        self.apply_component_property(&command).is_ok()
    }
}

/// The fields of `node` a new property of `master` could drive, skipping
/// ones a property already binds.
fn bindable_fields(master: &PenNode, node: &PenNode) -> Vec<BindingField> {
    let bound = |field: &BindingField| {
        properties_of(master).is_some_and(|properties| {
            properties.values().any(|property| {
                property
                    .bind
                    .iter()
                    .any(|binding| binding.node == node.id_str() && &binding.field == field)
            })
        })
    };
    let has_solid_fill = super::resolve::bound_value(node, &BindingField::Fill).is_some();
    let mut fields = Vec::new();
    if matches!(node, PenNode::Text(_)) {
        fields.push(BindingField::Content);
    }
    if matches!(node, PenNode::Ref(_)) {
        fields.push(BindingField::Ref);
    } else if has_solid_fill {
        fields.push(BindingField::Fill);
    }
    fields.push(BindingField::Visible);
    fields.retain(|field| !bound(field));
    fields
}
