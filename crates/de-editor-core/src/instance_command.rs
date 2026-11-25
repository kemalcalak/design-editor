//! Component-instance commands (#365): insert a linked instance, insert a
//! detached copy, detach an instance, or point it at another master.
//!
//! The applier lives in `command_apply/instance.rs`; the state mutations in
//! `component_instances.rs` and `components.rs`.

use crate::node_id::NodeId;

/// One component-instance mutation, carried by
/// [`crate::EditorCommand::Instance`].
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceCommand {
    /// Insert a linked instance (`{"type":"ref","ref":component_id}`) of a
    /// master that may live on any page. `parent = NONE` inserts at the
    /// page root; `x` / `y` default to the master's origin offset by 20px
    /// for a page root and stay unset inside a parent. `descendants_json`
    /// is the instance's `descendants` override object, keyed by the
    /// master's node ids.
    Insert {
        component_id: NodeId,
        parent: NodeId,
        x: Option<f64>,
        y: Option<f64>,
        descendants_json: Option<String>,
        page_id: Option<String>,
    },
    /// Drop an independent deep copy of a registered master onto the active
    /// page (fresh ids, `reusable` cleared) — the behaviour
    /// `instantiate_component` had before #365.
    Copy { component_id: NodeId },
    /// Turn the instance `node_id` into an independent subtree. The root
    /// keeps the instance's id; inner nodes get fresh ids. `page_id = None`
    /// finds the node on any page.
    Detach {
        node_id: NodeId,
        page_id: Option<String>,
    },
    /// Point the instance `node_id` at another master, keeping its
    /// descendants overrides, bindings and events. `page_id = None` finds
    /// the node on any page.
    Swap {
        node_id: NodeId,
        component_id: NodeId,
        page_id: Option<String>,
    },
}

impl InstanceCommand {
    /// The `descendants` JSON the command stores, for the motion guard.
    pub(crate) fn node_json_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Insert {
                descendants_json, ..
            } => descendants_json.as_mut(),
            Self::Copy { .. } | Self::Detach { .. } | Self::Swap { .. } => None,
        }
    }
}
