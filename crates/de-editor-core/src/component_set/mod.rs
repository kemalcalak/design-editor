//! Component variants (#368): sets of masters grouped by named axes.
//!
//! A component set is a frame (`componentSet`: axes in order, defaults)
//! holding its variant masters, each a `reusable` frame with its
//! `variantValues`. An instance names the variant it shows in `variant`;
//! its `ref` stays on that variant's master (see [`resolve`] for why), and
//! a `ref` naming the set is accepted and rewritten. A combination with no
//! master falls back to the nearest one, deterministically, and is
//! reported ([`pick`]).
//!
//! Writes ([`ComponentSetCommand`]) are checked against the document
//! first, so MCP refuses a bad call with the message the apply would give:
//! unknown axes and values list the allowed ones. Switching variants keeps
//! `props` and every override whose target exists in the new variant
//! ([`switch`]); the rest are reported in a [`VariantReport`].

mod apply;
mod create;
mod edit;
mod error;
mod panel;
mod pick;
mod report;
mod resolve;
mod summary;
mod switch;
mod tree;

pub use create::{layout_grid as layout_set_grid, CreateSet, PlannedSet, DEFAULT_AXIS};
pub use edit::{preview_command, ComponentSetCommand};
pub use error::VariantError;
pub use panel::{PanelVariantAxis, PanelVariants};
pub use pick::{
    component_set_of, full_values, parse_variant_name, pick_variant, set_containing,
    variant_masters, variant_name, variant_values_of, SetIndex, VariantPick,
};
pub use report::{values_label, DroppedOverride, VariantFallback, VariantReport};
pub use resolve::{check_values, normalize_inserted, normalize_set_refs, resolve_target, InstanceTarget};
pub use tree::find as find_node;
pub use summary::{set_summaries, set_summary, SetSummary, VariantSummary};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edit;
