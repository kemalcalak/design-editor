//! Why a component-instance command (#365) cannot apply. The command
//! applier reports a refusal as `Ok(false)`; MCP tools check first and
//! show these messages.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceError {
    /// No node with this id anywhere in the document.
    ComponentNotFound(String),
    /// The node exists but is neither a registered component nor a
    /// `reusable` frame.
    NotAComponent(String),
    /// The parent is not on the target page.
    ParentNotFound(String),
    /// The parent cannot hold children.
    ParentNotContainer(String),
    /// The instance would sit inside the master it shows (directly or
    /// through masters that use each other).
    Cycle { component: String, parent: String },
    /// `descendants` is not a JSON object.
    InvalidDescendants,
    /// No node with this id on the page searched.
    NodeNotFound(String),
    /// The node is not a component instance (`ref`).
    NotAnInstance(String),
    /// The instance already shows this master.
    SameComponent(String),
}

impl fmt::Display for InstanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComponentNotFound(id) => write!(f, "component not found: {id}"),
            Self::NotAComponent(id) => write!(
                f,
                "{id} is not a component (create_component or a reusable frame first)"
            ),
            Self::ParentNotFound(id) => write!(f, "parent not found: {id}"),
            Self::ParentNotContainer(id) => write!(f, "parent {id} cannot hold children"),
            Self::Cycle { component, parent } => write!(
                f,
                "an instance of {component} cannot go inside {parent}: it would contain itself"
            ),
            Self::InvalidDescendants => write!(f, "descendants must be a JSON object"),
            Self::NodeNotFound(id) => write!(f, "node not found: {id}"),
            Self::NotAnInstance(id) => write!(f, "{id} is not a component instance (ref)"),
            Self::SameComponent(id) => write!(f, "the instance already uses component {id}"),
        }
    }
}

impl std::error::Error for InstanceError {}
