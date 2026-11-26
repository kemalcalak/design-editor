//! What a component-set write did to instances, for the toast and the MCP
//! `warnings`.

use jian_ops_schema::component_set::VariantValues;

/// An instance whose requested combination has no master: it shows the
/// nearest one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantFallback {
    pub instance: String,
    pub requested: VariantValues,
    pub shown: String,
}

/// An instance override dropped because its target is not in the new
/// variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedOverride {
    pub instance: String,
    pub key: String,
}

/// The outcome of a component-set write.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VariantReport {
    /// The set written to (created, for a create).
    pub set_id: Option<String>,
    /// Masters the write created.
    pub created: Vec<String>,
    /// Masters the write removed.
    pub removed: Vec<String>,
    pub fallbacks: Vec<VariantFallback>,
    pub dropped_overrides: Vec<DroppedOverride>,
    /// Instances moved to the axis default because their value went.
    pub moved_to_default: Vec<String>,
    /// Masters a new set moved over from another page.
    pub moved_masters: Vec<String>,
}

impl VariantReport {
    /// One line per thing the caller should know about.
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        for fallback in &self.fallbacks {
            out.push(format!(
                "instance {} asks for {} which has no variant; it shows the nearest, {}",
                fallback.instance,
                values_label(&fallback.requested),
                fallback.shown
            ));
        }
        for dropped in &self.dropped_overrides {
            out.push(format!(
                "instance {}: override {} has no target in the new variant and was dropped",
                dropped.instance, dropped.key
            ));
        }
        for master in &self.moved_masters {
            out.push(format!(
                "component {master} moved from its page into the set"
            ));
        }
        for instance in &self.moved_to_default {
            out.push(format!(
                "instance {instance} used a removed value and moved to the default"
            ));
        }
        out
    }

    /// True when nothing needs reporting.
    pub fn is_quiet(&self) -> bool {
        self.fallbacks.is_empty()
            && self.dropped_overrides.is_empty()
            && self.moved_to_default.is_empty()
    }
}

/// `size=sm, state=loading`.
pub fn values_label(values: &VariantValues) -> String {
    values
        .iter()
        .map(|(axis, value)| format!("{axis}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}
