//! Built-in kit components that install as one component set (#368).
//!
//! The shadcn kit's six buttons are one Button set, `variant` ×
//! `size`. The first linked use of any of them installs the whole set on
//! the `Components` page; the kit's own templates are the `md` variants
//! with their ids verbatim (so `K("shadcn/btn-primary")`, the
//! `insert_shadcn_btn_*` tools and documents made before the set existed
//! keep working), and `sm` / `lg` are derived from them. Each master
//! carries a `label` text property.

use jian_ops_schema::component_set::{ComponentSet, VariantAxes, VariantValues};
use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use crate::pen_node_ext::PenNodeExt;
use crate::uikit::UIKit;
use crate::uikit_shadcn::SHADCN_KIT_ID;

/// A kit component set: which kit components are its `md` variants.
pub struct KitSetSpec {
    pub kit_id: &'static str,
    /// The set frame's id when it installs with its own ids.
    pub set_id: &'static str,
    pub name: &'static str,
    /// `variant` value → kit component id.
    pub members: &'static [(&'static str, &'static str)],
}

/// Sizes every kit button comes in: name, height, horizontal padding,
/// label size and width scale against the kit's `md` template.
const SIZES: [(&str, f64, f64, f64, f64); 3] = [
    ("sm", 32.0, 12.0, 13.0, 0.8),
    ("md", 40.0, 20.0, 14.0, 1.0),
    ("lg", 48.0, 28.0, 16.0, 1.2),
];

/// The default size.
const MD: &str = "md";

/// The built-in kit sets.
pub const KIT_SETS: &[KitSetSpec] = &[KitSetSpec {
    kit_id: SHADCN_KIT_ID,
    set_id: "shadcn-button",
    name: "Button",
    members: &[
        ("primary", "shadcn-btn-primary"),
        ("secondary", "shadcn-btn-secondary"),
        ("outline", "shadcn-btn-outline"),
        ("ghost", "shadcn-btn-ghost"),
        ("destructive", "shadcn-btn-destructive"),
        ("link", "shadcn-btn-link"),
    ],
}];

/// The set a kit component belongs to, and its values there.
pub fn kit_set_of(
    kit_id: &str,
    component_id: &str,
) -> Option<(&'static KitSetSpec, VariantValues)> {
    KIT_SETS.iter().find_map(|spec| {
        if spec.kit_id != kit_id {
            return None;
        }
        let (variant, _) = spec.members.iter().find(|(_, id)| *id == component_id)?;
        Some((
            spec,
            VariantValues::from([
                ("variant".to_string(), variant.to_string()),
                ("size".to_string(), MD.to_string()),
            ]),
        ))
    })
}

/// Check a `K()` `variant` override against the kit set holding the
/// component (installed or not).
pub fn check_kit_variant(
    kit_id: &str,
    component_id: &str,
    requested: &VariantValues,
) -> Result<(), crate::component_set::VariantError> {
    use crate::component_set::VariantError;
    let Some((spec, _)) = kit_set_of(kit_id, component_id) else {
        return Err(VariantError::NotAVariant(format!(
            "{kit_id}/{component_id}"
        )));
    };
    let variants: Vec<String> = spec.members.iter().map(|(v, _)| v.to_string()).collect();
    let sizes: Vec<String> = SIZES.iter().map(|(s, ..)| s.to_string()).collect();
    let set = ComponentSet {
        axes: VariantAxes(vec![
            ("variant".to_string(), variants),
            ("size".to_string(), sizes),
        ]),
        default: VariantValues::new(),
    };
    crate::component_set::check_values(&set, requested)
}

/// How many variants the set installs.
pub fn variant_count(spec: &KitSetSpec) -> usize {
    spec.members.len() * SIZES.len()
}

/// The kit component id and suffix-free template id of a member's `md`
/// master, by `variant` value.
fn member_id(spec: &KitSetSpec, variant: &str) -> Option<&'static str> {
    spec.members
        .iter()
        .find(|(value, _)| *value == variant)
        .map(|(_, id)| *id)
}

/// `id` for a derived size: the `md` ids stay verbatim.
fn sized_id(id: &str, size: &str) -> String {
    if size == MD {
        id.to_string()
    } else {
        format!("{id}-{size}")
    }
}

fn resize(node: &mut Value, size: &str, height: f64, pad: f64, font: f64, scale: f64) {
    if let Some(id) = node["id"].as_str() {
        node["id"] = json!(sized_id(id, size));
    }
    if let Some(width) = node["width"].as_f64() {
        node["width"] = json!((width * scale).round());
    }
    node["height"] = json!(height);
    node["padding"] = json!([0.0, pad, 0.0, pad]);
    for child in node["children"].as_array_mut().into_iter().flatten() {
        if let Some(id) = child["id"].as_str() {
            child["id"] = json!(sized_id(id, size));
        }
        if child["type"] == "text" {
            child["fontSize"] = json!(font);
        }
    }
}

/// The installable set frame for `spec`, built from `kit`'s templates.
/// `None` when a member is missing from the kit.
pub fn build_kit_set(spec: &KitSetSpec, kit: &UIKit) -> Option<PenNode> {
    let variants: Vec<String> = spec.members.iter().map(|(v, _)| v.to_string()).collect();
    let sizes: Vec<String> = SIZES.iter().map(|(s, ..)| s.to_string()).collect();
    let set = ComponentSet {
        axes: VariantAxes(vec![
            ("variant".to_string(), variants.clone()),
            ("size".to_string(), sizes.clone()),
        ]),
        default: VariantValues::from([
            ("variant".to_string(), variants[0].clone()),
            ("size".to_string(), MD.to_string()),
        ]),
    };
    let mut masters = Vec::new();
    for variant in &variants {
        let id = member_id(spec, variant)?;
        let template = kit.components.iter().find(|c| c.id == id)?;
        let base = serde_json::to_value(&template.template).ok()?;
        for (size, height, pad, font, scale) in SIZES {
            let mut node = base.clone();
            resize(&mut node, size, height, pad, font, scale);
            let values = VariantValues::from([
                ("variant".to_string(), variant.clone()),
                ("size".to_string(), size.to_string()),
            ]);
            node["reusable"] = json!(true);
            node["name"] = json!(crate::component_set::variant_name(&set, &values));
            node["variantValues"] = json!(values);
            if let Some(label) = node["children"][0]["id"].as_str().map(str::to_string) {
                let content = node["children"][0]["content"].clone();
                node["componentProperties"] = json!({"label": {
                    "type": "text", "default": content,
                    "bind": [{"node": label, "field": "content"}]}});
            }
            masters.push(serde_json::from_value::<PenNode>(node).ok()?);
        }
    }
    let mut set_node: PenNode = serde_json::from_value(json!({
        "type": "frame", "id": spec.set_id, "name": spec.name, "layout": "none",
        "cornerRadius": 8,
        "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": "#9747FF"}]},
    }))
    .ok()?;
    if let PenNode::Frame(frame) = &mut set_node {
        frame.component_set = Some(set);
        frame.children = Some(masters);
    }
    crate::component_set::layout_set_grid(&mut set_node);
    Some(set_node)
}

/// Every id in `node`'s subtree.
pub(crate) fn subtree_ids(node: &PenNode, out: &mut Vec<String>) {
    out.push(node.id_str().to_string());
    for child in node.children().into_iter().flatten() {
        subtree_ids(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shadcn_buttons_build_one_variant_by_size_set() {
        let kit = crate::uikit_shadcn::shadcn_kit();
        let set = build_kit_set(&KIT_SETS[0], &kit).expect("set builds");
        let masters = crate::component_set::variant_masters(&set);
        assert_eq!(masters.len(), 18);
        let ids: Vec<&str> = masters.iter().map(|m| m.id_str()).collect();
        assert!(ids.contains(&"shadcn-btn-primary"), "md keeps the kit id");
        assert!(ids.contains(&"shadcn-btn-ghost-lg"));
        let mut all = Vec::new();
        subtree_ids(&set, &mut all);
        let unique: std::collections::HashSet<&String> = all.iter().collect();
        assert_eq!(unique.len(), all.len(), "ids are unique");
        let lg = masters
            .iter()
            .find(|m| m.id_str() == "shadcn-btn-primary-lg")
            .unwrap();
        assert_eq!(lg.height_px(), Some(48.0));
        assert_eq!(
            kit_set_of(SHADCN_KIT_ID, "shadcn-btn-outline").unwrap().1["variant"],
            "outline"
        );
        assert!(kit_set_of(SHADCN_KIT_ID, "shadcn-badge").is_none());
    }
}
