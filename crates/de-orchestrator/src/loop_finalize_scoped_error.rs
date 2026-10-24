//! Error of [`super::record_loop_finalize_for_roots`].

use crate::RecordLoopFinalizeError;

/// A finalize of some roots that left the document as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopedFinalizeError {
    /// The finalizer's commands could not be recorded.
    Record(RecordLoopFinalizeError),
    /// The recorded commands did not replay onto the given roots alone.
    Replay,
}

impl std::fmt::Display for ScopedFinalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Record(error) => write!(f, "{error}"),
            Self::Replay => {
                f.write_str("its commands did not replay onto the finalized roots alone")
            }
        }
    }
}

impl std::error::Error for ScopedFinalizeError {}
