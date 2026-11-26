//! Typed failures of `de agent`'s command line.

use std::fmt;

use crate::agent_tasks_error::{TaskProblem, TasksFileError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentArgsError {
    /// A flag that takes a value came last.
    MissingValue { flag: String },
    /// A flag `de agent` does not know.
    UnknownFlag { flag: String },
    /// A bare word where none is expected.
    UnexpectedArgument { arg: String },
    /// Two flags that exclude each other.
    Conflict { flag: String, with: String },
    /// A numeric flag outside its range.
    InvalidNumber {
        flag: String,
        value: String,
        min: u64,
        max: u64,
    },
    /// `--prompt-file` could not be read.
    PromptFile { path: String, detail: String },
    /// The single task the flags describe is invalid.
    Task(TaskProblem),
    /// The `--tasks` file is invalid.
    Tasks(TasksFileError),
}

impl fmt::Display for AgentArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue { flag } => write!(f, "{flag} needs a value"),
            Self::UnknownFlag { flag } => {
                write!(
                    f,
                    "unknown flag {flag} for `de agent` (see `de agent --help`)"
                )
            }
            Self::UnexpectedArgument { arg } => write!(
                f,
                "unexpected argument {arg:?}; pass the prompt with -p \"...\""
            ),
            Self::Conflict { flag, with } => write!(f, "{flag} cannot be combined with {with}"),
            Self::InvalidNumber {
                flag,
                value,
                min,
                max,
            } => write!(
                f,
                "{flag} must be a whole number from {min} to {max}, got {value:?}"
            ),
            Self::PromptFile { path, detail } => {
                write!(f, "cannot read --prompt-file {path}: {detail}")
            }
            Self::Task(problem) => f.write_str(&flag_message(problem)),
            Self::Tasks(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AgentArgsError {}

impl From<TasksFileError> for AgentArgsError {
    fn from(error: TasksFileError) -> Self {
        Self::Tasks(error)
    }
}

/// A task problem worded with the flag the user typed.
fn flag_message(problem: &TaskProblem) -> String {
    match problem {
        TaskProblem::MissingPrompt => "-p/--prompt (or --tasks <file>) is required".into(),
        TaskProblem::MissingOut => "--out <file.de> is required".into(),
        TaskProblem::OutNotDe { out } => format!("--out must be a .de file, got {out:?}"),
        TaskProblem::UnsupportedInput { input } => {
            format!("--in must be a .de, .pen or .pptx file, got {input:?}")
        }
        TaskProblem::UnknownExport { value } => {
            format!("unknown --export format {value:?} (use png, pdf, html or svg)")
        }
        TaskProblem::InvalidTimeout { value } => {
            format!("--timeout must be 1 to 86400 seconds, got {value}")
        }
        TaskProblem::InvalidMaxTurns { value } => {
            format!("--max-turns must be 1 to 64, got {value}")
        }
    }
}
