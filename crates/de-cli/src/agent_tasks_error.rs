//! Typed failures of `de agent --tasks <file>`: reading the file, parsing
//! it, and checking each task before anything runs.

use std::fmt;

/// What is wrong with one task entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskProblem {
    /// `prompt` is missing or blank.
    MissingPrompt,
    /// `out` is missing.
    MissingOut,
    /// `out` must name a `.de` file.
    OutNotDe { out: String },
    /// `in` must be a `.de`, pen.dev `.pen` or `.pptx` file.
    UnsupportedInput { input: String },
    /// An `export` entry names no supported format.
    UnknownExport { value: String },
    /// `timeout` is outside 1..=86400 seconds.
    InvalidTimeout { value: u64 },
    /// `maxTurns` is outside 1..=64.
    InvalidMaxTurns { value: u64 },
}

impl fmt::Display for TaskProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrompt => f.write_str("`prompt` is required"),
            Self::MissingOut => f.write_str("`out` is required"),
            Self::OutNotDe { out } => write!(f, "`out` must be a .de file, got {out:?}"),
            Self::UnsupportedInput { input } => {
                write!(f, "`in` must be a .de, .pen or .pptx file, got {input:?}")
            }
            Self::UnknownExport { value } => write!(
                f,
                "unknown export format {value:?} (use png, pdf, html or svg)"
            ),
            Self::InvalidTimeout { value } => {
                write!(f, "`timeout` must be 1 to 86400 seconds, got {value}")
            }
            Self::InvalidMaxTurns { value } => {
                write!(f, "`maxTurns` must be 1 to 64, got {value}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TasksFileError {
    /// The tasks file could not be read.
    Read { path: String, detail: String },
    /// The file is not valid YAML / JSON of the tasks shape (the parser's
    /// message carries the line and column).
    Parse { path: String, detail: String },
    /// `tasks` is empty.
    NoTasks { path: String },
    /// Task number `index` (1-based) is invalid.
    Task {
        index: usize,
        name: Option<String>,
        problem: TaskProblem,
    },
    /// Two tasks write the same `out` file.
    DuplicateOut {
        out: String,
        first: usize,
        second: usize,
    },
    /// Two tasks share a `name`.
    DuplicateName { name: String },
}

impl fmt::Display for TasksFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, detail } => write!(f, "cannot read tasks file {path}: {detail}"),
            Self::Parse { path, detail } => write!(f, "invalid tasks file {path}: {detail}"),
            Self::NoTasks { path } => write!(f, "tasks file {path} lists no tasks"),
            Self::Task {
                index,
                name: Some(name),
                problem,
            } => write!(f, "task {index} ({name}): {problem}"),
            Self::Task {
                index,
                name: None,
                problem,
            } => write!(f, "task {index}: {problem}"),
            Self::DuplicateOut { out, first, second } => write!(
                f,
                "tasks {first} and {second} both write {out}; give each task its own `out`"
            ),
            Self::DuplicateName { name } => {
                write!(f, "two tasks are named {name:?}; task names must be unique")
            }
        }
    }
}

impl std::error::Error for TasksFileError {}
