//! Why a component-set write was refused. Messages name the allowed axes
//! or values so an agent can fix the call.

use std::fmt;

/// A refused component-set write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantError {
    /// No component set has this id.
    SetNotFound(String),
    /// No node has this id, or it is not an instance.
    InstanceNotFound(String),
    /// No component master has this id.
    MasterNotFound(String),
    /// The instance's component is no variant of a set.
    NotAVariant(String),
    /// A master to combine already belongs to a set.
    AlreadyInSet { master: String, set: String },
    /// A set needs at least one master.
    NoMasters,
    /// The same master was named twice.
    DuplicateMaster(String),
    /// An axis the set does not have.
    UnknownAxis { axis: String, allowed: String },
    /// A value the axis does not list.
    UnknownValue {
        axis: String,
        value: String,
        allowed: String,
    },
    /// Two variants would have the same values.
    DuplicateVariant { values: String },
    /// A master's values could not be read from its name or the call.
    MissingValues { master: String },
    /// An axis or value name is blank or holds a separator.
    InvalidName(String),
    /// The axis already exists.
    AxisTaken(String),
    /// The axis already lists the value.
    ValueTaken { axis: String, value: String },
    /// An axis keeps at least one value.
    LastValue { axis: String },
}

impl fmt::Display for VariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SetNotFound(id) => write!(f, "component set {id} not found"),
            Self::InstanceNotFound(id) => write!(f, "instance {id} not found"),
            Self::MasterNotFound(id) => write!(f, "component {id} not found"),
            Self::NotAVariant(id) => write!(
                f,
                "instance {id} is not a variant of a component set; create one with create_component_set"
            ),
            Self::AlreadyInSet { master, set } => {
                write!(f, "component {master} is already a variant of set {set}")
            }
            Self::NoMasters => f.write_str("a component set needs at least one master"),
            Self::DuplicateMaster(id) => write!(f, "component {id} is named twice"),
            Self::UnknownAxis { axis, allowed } => {
                write!(f, "unknown variant axis {axis}; axes: {allowed}")
            }
            Self::UnknownValue {
                axis,
                value,
                allowed,
            } => write!(f, "unknown value {value} for axis {axis}; values: {allowed}"),
            Self::DuplicateVariant { values } => {
                write!(f, "a variant with {values} already exists")
            }
            Self::MissingValues { master } => write!(
                f,
                "no variant values for component {master}: name it key=value, key=value or pass values"
            ),
            Self::InvalidName(name) => write!(
                f,
                "invalid axis or value name {name:?}: it must be non-empty without , or ="
            ),
            Self::AxisTaken(axis) => write!(f, "axis {axis} already exists"),
            Self::ValueTaken { axis, value } => {
                write!(f, "axis {axis} already has value {value}")
            }
            Self::LastValue { axis } => write!(f, "axis {axis} must keep at least one value"),
        }
    }
}

impl std::error::Error for VariantError {}

/// `a, b, c` for an error message.
pub(crate) fn join(items: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let joined: Vec<String> = items
        .into_iter()
        .map(|item| item.as_ref().to_string())
        .collect();
    if joined.is_empty() {
        "(none)".to_string()
    } else {
        joined.join(", ")
    }
}
