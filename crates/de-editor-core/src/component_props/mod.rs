//! Component properties (#367): named text / boolean / instance / colour
//! inputs a master declares (`componentProperties`) and its instances set
//! by name (`props`).
//!
//! An instance's `props` become `descendants` patches right before it
//! expands ([`fold_props_into_descendants`], called by the render resolver,
//! the inspector's display merge and detach), so canvas, export, codegen
//! and preview share the one override path. An explicit `descendants`
//! entry for the same field wins.
//!
//! Writes ([`ComponentPropertyCommand`]) are planned against the document
//! first, so MCP refuses a bad call with the same message the apply would
//! give: unknown names list the allowed `name (type)` pairs.

mod check;
mod edit;
mod error;
mod resolve;
mod summary;

pub use check::{describe, find_in_subtree, properties_of, PropsContext};
pub use edit::{ComponentPropertyCommand, PlannedUpsert, PropertyUpsert};
pub use error::PropsError;
pub use summary::{property_names, property_summaries, PropertySummary};

pub(crate) use resolve::fold_props_into_descendants;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edit;
