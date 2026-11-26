//! [`ConvertError`]: why a conversion of copies into instances (#369)
//! cannot run at all. Members that only differ are not errors; they are
//! `SkippedMember`s of the plan.

use super::SkippedMember;
use crate::id_allocator::IdAllocError;

/// Why a conversion cannot run. Members that only differ are not errors;
/// they are [`SkippedMember`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    TooFewNodes(usize),
    DuplicateNode(String),
    NodeNotFound(String),
    NotConvertible { id: String, reason: String },
    NestedNodes { outer: String, inner: String },
    MasterNotListed(String),
    MasterUnusable { id: String, reason: String },
    NothingToConvert(Vec<SkippedMember>),
    IdAllocation(IdAllocError),
    Serialize,
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewNodes(count) => write!(
                f,
                "convert_to_instances needs at least 2 frames, got {count}"
            ),
            Self::DuplicateNode(id) => write!(f, "node {id} is listed twice"),
            Self::NodeNotFound(id) => write!(f, "node not found: {id}"),
            Self::NotConvertible { id, reason } => write!(f, "node {id} {reason}"),
            Self::NestedNodes { outer, inner } => {
                write!(
                    f,
                    "node {inner} is inside listed node {outer}; list one of them"
                )
            }
            Self::MasterNotListed(id) => {
                write!(f, "masterFrom {id} is not one of the listed nodes")
            }
            Self::MasterUnusable { id, reason } => {
                write!(f, "master source {id} cannot become the master: {reason}")
            }
            Self::NothingToConvert(skipped) => {
                f.write_str("no frame can become an instance of the master")?;
                if let Some(first) = skipped.first() {
                    write!(f, "; {}: {}", first.id.as_str(), first.reason)?;
                    if skipped.len() > 1 {
                        write!(f, " (and {} more)", skipped.len() - 1)?;
                    }
                }
                Ok(())
            }
            Self::IdAllocation(error) => error.fmt(f),
            Self::Serialize => f.write_str("the document could not be read as JSON"),
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<IdAllocError> for ConvertError {
    fn from(error: IdAllocError) -> Self {
        Self::IdAllocation(error)
    }
}
