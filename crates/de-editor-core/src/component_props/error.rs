//! Why a component-property write was refused. Messages name the allowed
//! properties, types or values so an agent can fix the call.

use std::fmt;

/// A refused component-property write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropsError {
    /// No component master has this id.
    ComponentNotFound(String),
    /// No node has this id, or it is not an instance.
    InstanceNotFound(String),
    /// The instance names a property its master does not declare.
    UnknownProperty {
        component: String,
        name: String,
        allowed: String,
    },
    /// The value does not have the property's type.
    WrongType {
        name: String,
        expected: String,
        got: String,
        allowed: String,
    },
    /// An instance property's value is not a component id.
    UnknownComponent { name: String, value: String },
    /// A colour property's value is neither `#hex` nor a known `$variable`.
    InvalidColor { name: String, value: String },
    /// A property name is blank or already taken.
    InvalidName(String),
    /// The master already declares a property with this name.
    NameTaken(String),
    /// A new property needs a type.
    MissingType(String),
    /// An unknown property type.
    UnknownType(String),
    /// A binding names a node outside the master.
    BindNodeNotFound { node: String, component: String },
    /// A binding's field does not fit its node or the property's type.
    BindMismatch {
        name: String,
        node: String,
        field: String,
        reason: String,
    },
    /// The type changes while instances still set the property.
    TypeChangeInUse { name: String, instances: usize },
}

impl fmt::Display for PropsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComponentNotFound(id) => write!(f, "component {id} not found"),
            Self::InstanceNotFound(id) => write!(f, "instance {id} not found"),
            Self::UnknownProperty {
                component,
                name,
                allowed,
            } => write!(
                f,
                "component {component} has no property {name:?}; allowed: {allowed}"
            ),
            Self::WrongType {
                name,
                expected,
                got,
                allowed,
            } => write!(
                f,
                "property {name:?} takes a {expected} value, got {got}; allowed: {allowed}"
            ),
            Self::UnknownComponent { name, value } => write!(
                f,
                "property {name:?} takes a component id, and {value:?} is no component"
            ),
            Self::InvalidColor { name, value } => write!(
                f,
                "property {name:?} takes a colour (#rrggbb or a $variable), got {value:?}"
            ),
            Self::InvalidName(name) => write!(f, "invalid property name {name:?}"),
            Self::NameTaken(name) => write!(f, "the component already has a property {name:?}"),
            Self::MissingType(name) => write!(
                f,
                "property {name:?} is new: give a type (text, boolean, instance or color)"
            ),
            Self::UnknownType(kind) => write!(
                f,
                "unknown property type {kind:?}; allowed: text, boolean, instance, color"
            ),
            Self::BindNodeNotFound { node, component } => {
                write!(f, "node {node} is not inside component {component}")
            }
            Self::BindMismatch {
                name,
                node,
                field,
                reason,
            } => write!(
                f,
                "property {name:?} cannot bind {field} of node {node}: {reason}"
            ),
            Self::TypeChangeInUse { name, instances } => write!(
                f,
                "property {name:?} is set on {instances} instance(s); remove it before changing its type"
            ),
        }
    }
}

impl std::error::Error for PropsError {}
