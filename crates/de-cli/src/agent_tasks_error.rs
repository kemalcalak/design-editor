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
    /// The task name cannot be a folder name (it names the export folder).
    InvalidName { name: String },
    /// The prompt is larger than the design agent accepts.
    PromptTooLarge { bytes: usize, max: usize },
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
            Self::InvalidName { name } => write!(
                f,
                "`name` {name:?} cannot be a folder name (it names the task's export \
                 folder: no / \\ : * ? \" < > |, not . or .., no trailing dot or space, \
                 at most 100 characters)"
            ),
            Self::PromptTooLarge { bytes, max } => {
                write!(f, "the prompt is too large ({bytes} bytes; maximum {max})")
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
    /// Two tasks share a `name` (compared ignoring case).
    DuplicateName { name: String },
    /// Two tasks' export folders are the same folder or one holds the other.
    ExportDirClash {
        first: usize,
        second: usize,
        dir: String,
    },
    /// A task's export folder is a document or a document's asset folder.
    ExportDirOverDocument {
        index: usize,
        dir: String,
        out: String,
    },
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
            Self::DuplicateName { name } => write!(
                f,
                "two tasks are named {name:?}; task names must be unique (ignoring case)"
            ),
            Self::ExportDirClash { first, second, dir } => write!(
                f,
                "tasks {first} and {second} would export into overlapping folders ({dir}); \
                 give them different names or export roots"
            ),
            Self::ExportDirOverDocument { index, dir, out } => write!(
                f,
                "task {index} would export into {dir}, which is the document {out} or its \
                 asset folder; choose another export root"
            ),
        }
    }
}

impl std::error::Error for TasksFileError {}
