//! Sets as `list_components` / `get_component` describe them.

use jian_ops_schema::component_set::VariantValues;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{json, Map, Value};

use super::pick::{component_set_of, for_each_node, variant_masters, variant_values_of};
use crate::pen_node_ext::PenNodeExt;

/// One variant of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantSummary {
    pub id: String,
    pub name: String,
    pub values: VariantValues,
}

/// A component set with its axes and variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSummary {
    pub id: String,
    pub name: String,
    /// Axes in order with their values.
    pub axes: Vec<(String, Vec<String>)>,
    /// The full default combination.
    pub default: VariantValues,
    pub variants: Vec<VariantSummary>,
}

impl SetSummary {
    /// `{"id", "name", "axes": {axis: [values]}, "default", "variants": [...]}`;
    /// `axes` keeps the set's order.
    pub fn to_json(&self) -> Value {
        let mut axes = Map::new();
        for (axis, values) in &self.axes {
            axes.insert(axis.clone(), json!(values));
        }
        json!({
            "id": self.id,
            "name": self.name,
            "axes": Value::Object(axes),
            "default": self.default,
            "variants": self.variants.iter().map(|variant| json!({
                "id": variant.id,
                "name": variant.name,
                "values": variant.values,
            })).collect::<Vec<_>>(),
        })
    }

    /// `variant[primary,ghost] × size[sm,md]` for a one-line listing.
    pub fn axes_label(&self) -> String {
        self.axes
            .iter()
            .map(|(axis, values)| format!("{axis}[{}]", values.join(",")))
            .collect::<Vec<_>>()
            .join(" × ")
    }
}

fn summarize(node: &PenNode) -> Option<SetSummary> {
    let set = component_set_of(node)?;
    Some(SetSummary {
        id: node.id_str().to_string(),
        name: node
            .base()
            .name
            .clone()
            .unwrap_or_else(|| node.id_str().to_string()),
        axes: set
            .axes
            .iter()
            .map(|(axis, values)| (axis.to_string(), values.to_vec()))
            .collect(),
        default: super::pick::full_values(set, None, None),
        variants: variant_masters(node)
            .into_iter()
            .map(|master| VariantSummary {
                id: master.id_str().to_string(),
                name: master
                    .base()
                    .name
                    .clone()
                    .unwrap_or_else(|| master.id_str().to_string()),
                values: variant_values_of(master).cloned().unwrap_or_default(),
            })
            .collect(),
    })
}

/// Every set of the document, in document order.
pub fn set_summaries(doc: &PenDocument) -> Vec<SetSummary> {
    let mut out = Vec::new();
    for_each_node(doc, &mut |node| {
        if let Some(summary) = summarize(node) {
            out.push(summary);
        }
    });
    out
}

/// The set `id`, if it is one.
pub fn set_summary(doc: &PenDocument, id: &str) -> Option<SetSummary> {
    summarize(super::tree::find(doc, id)?)
}
