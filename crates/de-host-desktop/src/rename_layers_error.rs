//! Typed failure for the Rename-layers agent turn (`rename_layers_host.rs`,
//! #383). Every variant ends the same way for the user — the preview keeps
//! the offline rules' names and says the agent was unavailable — but the
//! stderr line names which of the three it was.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RenameLayersAiError {
    /// The worker thread would not spawn.
    WorkerSpawn(String),
    /// The provider streamed a `ChatDelta::Error`; its message verbatim.
    Provider(String),
    /// The worker dropped its sender without reporting.
    WorkerVanished,
    /// The preview closed or reopened while the turn ran.
    Cancelled,
}

impl fmt::Display for RenameLayersAiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerSpawn(err) => write!(f, "could not start the rename worker: {err}"),
            Self::Provider(message) => write!(f, "{message}"),
            Self::WorkerVanished => write!(f, "rename worker exited without a result"),
            Self::Cancelled => write!(f, "rename layers turn cancelled"),
        }
    }
}
