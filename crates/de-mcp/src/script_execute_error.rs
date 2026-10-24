//! Typed failures for the `execute` script tool (`script_execute.rs`).
//!
//! Two layers: [`ExecuteError`] is what one host call or the whole run can
//! refuse with (its `Display` is the sentence the script's `catch` or the
//! tool error shows), and [`ScriptRenderError`] is what a host renderer
//! reports for one screenshot or export.

use std::fmt;

use super::batch_program_error::ProgramError;

/// Everything an `execute` run or one of its host calls can refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecuteError {
    /// Nothing runnable after stripping fences.
    EmptySource,
    /// The source exceeds `MAX_SCRIPT_BYTES`.
    SourceTooLarge { bytes: usize, max: usize },
    /// The QuickJS runtime, context or host binding could not be set up — a
    /// host fault, never the script's.
    Sandbox(String),
    /// The script threw (its own `throw`, a JS error, or a host call error
    /// it did not catch). `location` is the first stack frame when QuickJS
    /// reports one.
    Threw {
        message: String,
        location: Option<String>,
    },
    /// The script ran past the wall-clock budget.
    Timeout { millis: u128 },
    /// A write failed and the script caught the error and went on. The run
    /// still fails: a half-applied script is never committed.
    WriteFailed(String),
    /// A host call got arguments of the wrong shape.
    BadArgument { call: &'static str, detail: String },
    /// A node, path or variable named by the script does not exist.
    NotFound(String),
    /// A write operation the executor refused (wraps the DSL executor's own
    /// classification).
    Program(ProgramError),
    /// A per-run cap was reached (writes, screenshots, exports).
    LimitReached { what: &'static str, max: usize },
    /// `TakeScreenshot` / `Export` on a host that cannot render.
    NoRenderer(&'static str),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecuteError::EmptySource => f.write_str("script is empty"),
            ExecuteError::SourceTooLarge { bytes, max } => {
                write!(f, "script too large: {bytes} bytes (max {max})")
            }
            ExecuteError::Sandbox(detail) => write!(f, "script sandbox: {detail}"),
            ExecuteError::Threw { message, location } => match location {
                Some(at) => write!(f, "{message} ({at})"),
                None => f.write_str(message),
            },
            ExecuteError::Timeout { millis } => {
                write!(f, "script ran longer than {millis} ms and was stopped")
            }
            ExecuteError::WriteFailed(detail) => write!(
                f,
                "a write failed and the script went on after catching it: {detail}"
            ),
            ExecuteError::BadArgument { call, detail } => write!(f, "{call}: {detail}"),
            ExecuteError::NotFound(detail) => f.write_str(detail),
            ExecuteError::Program(error) => write!(f, "{error}"),
            ExecuteError::LimitReached { what, max } => {
                write!(f, "too many {what} in one script (max {max})")
            }
            ExecuteError::NoRenderer(call) => {
                write!(f, "{call} needs a host that renders; this one cannot")
            }
        }
    }
}

impl std::error::Error for ExecuteError {}

impl From<ProgramError> for ExecuteError {
    fn from(error: ProgramError) -> Self {
        ExecuteError::Program(error)
    }
}

/// A host renderer's failure for one screenshot or export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRenderError(pub String);

impl fmt::Display for ScriptRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScriptRenderError {}
