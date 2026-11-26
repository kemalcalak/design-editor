//! Why a scale (#394) cannot apply. The command applier reports a refusal
//! as `Ok(false)`; the MCP tool and the panel check first and show these.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ScaleError {
    /// No node ids were given.
    NoTargets,
    /// The factor is not a finite number inside the allowed range.
    FactorOutOfRange(f64),
    /// The factor is 1: nothing would change.
    Unchanged,
    /// No node with this id on the active page.
    NodeNotFound(String),
    /// The id names a node inside a component instance.
    InstanceChild(String),
    /// The node (or an ancestor) is locked or hidden.
    NotEditable(String),
    /// The target size is not a positive finite number.
    InvalidSize(f64),
    /// The selection has no width (or height) to scale from.
    EmptyBounds,
}

impl fmt::Display for ScaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTargets => write!(f, "nodeIds must name at least one node"),
            Self::FactorOutOfRange(v) => write!(
                f,
                "scale factor must be between {} and {}, got {v}",
                crate::selection_scale::MIN_SCALE,
                crate::selection_scale::MAX_SCALE
            ),
            Self::Unchanged => write!(f, "scale factor 1 leaves the nodes unchanged"),
            Self::NodeNotFound(id) => write!(f, "node {id:?} is not on the active page"),
            Self::InstanceChild(id) => write!(
                f,
                "node {id:?} is inside a component instance: scale the instance instead"
            ),
            Self::NotEditable(id) => write!(f, "node {id:?} is locked or hidden"),
            Self::InvalidSize(v) => {
                write!(f, "target size must be a positive number, got {v}")
            }
            Self::EmptyBounds => write!(f, "the nodes have no size to scale from"),
        }
    }
}

impl std::error::Error for ScaleError {}
