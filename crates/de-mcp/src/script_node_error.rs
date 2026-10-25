//! Typed failures of a script node run (`script_node.rs`, #19). The
//! `Display` text is what the property panel shows under the node.

use std::fmt;

/// Everything `run_script_node` can refuse on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptNodeError {
    /// The source exceeds the size cap.
    SourceTooLarge { bytes: usize, max: usize },
    /// The QuickJS runtime or context could not start — a host fault.
    Sandbox(String),
    /// The program threw (message), or a JS-side step failed.
    Threw(String),
    /// The run took longer than its budget and was interrupted.
    TimedOut { limit_ms: u64 },
    /// The program returned something that is not an array of nodes.
    NotAnArray,
    /// Element `index` of the returned array is not a valid node.
    InvalidNode { index: usize, detail: String },
    /// A program may not generate another script node.
    NestedScript { index: usize },
    /// The program returned more nodes than allowed.
    TooManyNodes { count: usize, max: usize },
}

impl fmt::Display for ScriptNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptNodeError::SourceTooLarge { bytes, max } => {
                write!(f, "script too large: {bytes} bytes (max {max})")
            }
            ScriptNodeError::Sandbox(detail) => write!(f, "script sandbox: {detail}"),
            ScriptNodeError::Threw(message) => write!(f, "script error: {message}"),
            ScriptNodeError::TimedOut { limit_ms } => {
                write!(f, "script stopped after {limit_ms} ms")
            }
            ScriptNodeError::NotAnArray => f.write_str("script must return an array of nodes"),
            ScriptNodeError::InvalidNode { index, detail } => {
                write!(f, "node {index} is invalid: {detail}")
            }
            ScriptNodeError::NestedScript { index } => {
                write!(f, "node {index} is a script node; scripts cannot nest")
            }
            ScriptNodeError::TooManyNodes { count, max } => {
                write!(f, "script returned {count} nodes (max {max})")
            }
        }
    }
}

impl std::error::Error for ScriptNodeError {}
