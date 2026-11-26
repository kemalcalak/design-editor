//! Typed failures of one `de agent` task, by the step that stopped it.

use std::fmt;

use de_host_services::design_agent_run::DesignAgentRunError;
use de_host_services::doc_io::DocIoError;
use de_host_services::headless_export::{HeadlessExportError, HeadlessExportFormat};

/// The step a task stopped in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Load,
    Agent,
    Save,
    Export,
}

impl Stage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Stage::Load => "load",
            Stage::Agent => "agent",
            Stage::Save => "save",
            Stage::Export => "export",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskError {
    /// `--in` could not be opened.
    Load { path: String, source: DocIoError },
    /// The design agent failed, timed out or was cancelled mid-run.
    Agent(DesignAgentRunError),
    /// The agent's landing command did not apply to the document.
    Apply,
    /// The folder for `--out` could not be created.
    CreateDir { path: String, detail: String },
    /// The atomic save of `--out` failed; the file is unchanged.
    Save { path: String, source: DocIoError },
    /// Ctrl+C after the agent finished and before the save: nothing written.
    CancelledBeforeSave,
    /// One export format failed outright.
    Export {
        format: HeadlessExportFormat,
        source: HeadlessExportError,
    },
    /// Some frames of one export format failed; the others were written.
    ExportPartial {
        format: HeadlessExportFormat,
        failed: Vec<(String, String)>,
    },
    /// Ctrl+C after `--out` was saved: these exports were not written.
    Interrupted { skipped: Vec<HeadlessExportFormat> },
}

impl TaskError {
    pub(crate) fn stage(&self) -> Stage {
        match self {
            Self::Load { .. } => Stage::Load,
            Self::Agent(_) | Self::Apply => Stage::Agent,
            Self::CreateDir { .. } | Self::Save { .. } | Self::CancelledBeforeSave => Stage::Save,
            Self::Export { .. } | Self::ExportPartial { .. } | Self::Interrupted { .. } => {
                Stage::Export
            }
        }
    }

    /// Whether the task stopped because the run was cancelled.
    pub(crate) fn is_cancellation(&self) -> bool {
        matches!(
            self,
            Self::Agent(DesignAgentRunError::Cancelled)
                | Self::CancelledBeforeSave
                | Self::Interrupted { .. }
        )
    }
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load { path, source } => write!(f, "cannot open {path}: {source}"),
            Self::Agent(error) => write!(f, "{error}"),
            Self::Apply => f.write_str("the agent's design could not be applied to the document"),
            Self::CreateDir { path, detail } => write!(f, "cannot create {path}: {detail}"),
            Self::Save { path, source } => write!(f, "cannot save {path}: {source}"),
            Self::CancelledBeforeSave => {
                f.write_str("cancelled before saving; the output file is unchanged")
            }
            Self::Export { format, source } => write!(f, "{}: {source}", format.as_str()),
            Self::ExportPartial { format, failed } => write!(
                f,
                "{}: {} file(s) failed: {}",
                format.as_str(),
                failed.len(),
                failed
                    .iter()
                    .map(|(file, error)| format!("{file} ({error})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Interrupted { skipped } => write!(
                f,
                "cancelled after saving; not exported: {}",
                skipped
                    .iter()
                    .map(|format| format.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for TaskError {}

impl From<DesignAgentRunError> for TaskError {
    fn from(error: DesignAgentRunError) -> Self {
        Self::Agent(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runner::{status_for, TaskStatus};

    #[test]
    fn stages_and_statuses_follow_where_the_task_stopped() {
        let interrupted = TaskError::Interrupted {
            skipped: vec![HeadlessExportFormat::Pdf, HeadlessExportFormat::Html],
        };
        assert_eq!(interrupted.stage(), Stage::Export);
        assert_eq!(
            interrupted.to_string(),
            "cancelled after saving; not exported: pdf, html"
        );
        assert_eq!(status_for(&interrupted, true), TaskStatus::Interrupted);

        let mid_run = TaskError::Agent(DesignAgentRunError::Cancelled);
        assert_eq!(mid_run.stage(), Stage::Agent);
        assert_eq!(status_for(&mid_run, false), TaskStatus::Cancelled);
        assert_eq!(
            status_for(&TaskError::CancelledBeforeSave, false),
            TaskStatus::Cancelled
        );
        assert_eq!(TaskError::CancelledBeforeSave.stage(), Stage::Save);

        let failed = TaskError::ExportPartial {
            format: HeadlessExportFormat::Png,
            failed: vec![("01-A.png".into(), "boom".into())],
        };
        assert_eq!(status_for(&failed, true), TaskStatus::Failed);
        assert_eq!(failed.to_string(), "png: 1 file(s) failed: 01-A.png (boom)");
        assert!(TaskStatus::Interrupted.stopped() && TaskStatus::Cancelled.stopped());
        assert!(!TaskStatus::Failed.stopped());
    }
}
