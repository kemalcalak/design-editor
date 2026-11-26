//! Repeated hand-built copies → component instances (#369).
//!
//! Documents built before linked instances hold many identical copies
//! (buttons, cards, table rows). This module finds them and turns them into
//! one master plus `ref` instances:
//!
//! - **Find** ([`find_repeated_structures`]): a structural fingerprint per
//!   frame subtree — node kinds, child order, layout, sizing modes, paint and
//!   text style by value; ids, names, positions, text content, image sources,
//!   visibility, lock and interactions are left out (they become overrides).
//!   Frames sharing a fingerprint form a group; the largest unit wins, so a
//!   card group is reported, not the buttons inside its cards.
//! - **Plan** ([`plan_conversion`]): read only. The master source's frame
//!   becomes the master (on the `Components` page, or in place); every other
//!   member becomes a `ref` at its own slot that keeps its id, name and
//!   position, with its differences as `descendants` patches. Each instance
//!   is expanded again and compared with the frame it replaces; a member
//!   whose structure differs, or whose instance would not reproduce it, is
//!   skipped with the first difference, never merged.
//! - **Apply** (`EditorCommand::ConvertToInstances`): plans against the live
//!   document first and mutates only a valid plan, as one undo step.

use jian_ops_schema::node::PenNode;

use crate::NodeId;

mod apply;
mod convert_error;
mod diff;
mod locate;
mod plan;
mod scan;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_plan;

pub use convert_error::ConvertError;
pub use plan::{plan_conversion, plan_conversion_in};
pub use scan::{find_repeated_structures, group_members, similar_layers};

/// Smallest subtree (in nodes) a group member must have by default.
pub const DEFAULT_MIN_NODES: usize = 3;

/// Prefix of a group id; the rest is the fingerprint in hex.
pub const GROUP_ID_PREFIX: &str = "rs-";

/// Frames that share one structure.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatedGroup {
    /// `rs-<fingerprint>`; stable while the structure stays the same.
    pub group_id: String,
    /// Members in document order.
    pub node_ids: Vec<NodeId>,
    /// The page index of each member (`None`: a document without pages).
    pub pages: Vec<Option<usize>>,
    /// Nodes in one member's subtree.
    pub nodes_per_member: usize,
    /// The first member's layer name.
    pub name: Option<String>,
}

/// Where the master goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MasterPlacement {
    /// A copy on the hidden `Components` page (the kit / library master
    /// convention); every member becomes an instance.
    #[default]
    ComponentsPage,
    /// The master source frame itself becomes the master where it is.
    InPlace,
}

impl MasterPlacement {
    /// The wire name MCP uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ComponentsPage => "components",
            Self::InPlace => "inPlace",
        }
    }
}

/// What to convert, carried by `EditorCommand::ConvertToInstances`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConvertRequest {
    /// The frames to convert, on any page.
    pub node_ids: Vec<NodeId>,
    /// The member the master is made from; the first listed when `None`.
    pub master_from: Option<NodeId>,
    pub placement: MasterPlacement,
}

/// One member that becomes an instance.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertedMember {
    /// The member's id, which its instance keeps.
    pub id: NodeId,
    /// `descendants` entries the instance needs.
    pub overrides: usize,
    /// The `ref` node that replaces the member.
    pub instance: PenNode,
}

/// One member left as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedMember {
    pub id: NodeId,
    /// The first difference (or the reason it cannot be an instance).
    pub reason: String,
}

/// A checked conversion, ready to apply.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionPlan {
    /// The master's id (predicted by the allocator for a new master).
    pub master_id: NodeId,
    /// The member the master is made from.
    pub master_from: NodeId,
    pub placement: MasterPlacement,
    /// The master's name in the component registry.
    pub master_name: String,
    /// The master node (the `Components` page copy, or the source frame
    /// marked reusable in place).
    pub master: PenNode,
    pub converted: Vec<ConvertedMember>,
    pub skipped: Vec<SkippedMember>,
    /// Compact JSON bytes of the frames that are replaced (and, in place,
    /// of the master source).
    pub bytes_before: usize,
    /// The same for the instances plus the master that replace them.
    pub bytes_after: usize,
}
