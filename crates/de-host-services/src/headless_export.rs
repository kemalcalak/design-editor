//! Export a finished document without a window — what `de agent --export`
//! writes after the design agent lands its design.
//!
//! No renderer of its own: every format goes through the exporter the app
//! and the MCP tools already use, so a headless export matches the same
//! export made by hand.
//!
//! - `png` / `svg`: one file per top-level frame on the active page
//!   (`plan_frame_exports` names them), as MCP `export_frames` writes.
//! - `pdf`: one page per board, as MCP `export_deck format:pdf` writes.
//! - `html`: the page as HTML with CSS and an `assets/` folder, as MCP
//!   `export_html format:html-css` writes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use de_editor_core::export_batch::plan_frame_exports;
use de_editor_core::EditorState;
use de_mcp::{McpTool, ToolOutcome};

pub use crate::headless_export_error::HeadlessExportError;
use crate::export_batch::{export_frames_svg_to_dir, export_frames_to_dir};

/// The formats a headless run can export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeadlessExportFormat {
    Png,
    Svg,
    Pdf,
    Html,
}

impl HeadlessExportFormat {
    /// Every format, in the order help text lists them.
    pub const ALL: [HeadlessExportFormat; 4] = [Self::Png, Self::Pdf, Self::Html, Self::Svg];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Html => "html",
        }
    }

    /// Parse a format name (case-insensitive).
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.as_str().eq_ignore_ascii_case(name.trim()))
    }
}

/// What one format's export wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadlessExportReport {
    /// Files written, in export order.
    pub files: Vec<PathBuf>,
    /// `(file name, error)` for frames that failed; the rest still landed.
    pub failed: Vec<(String, String)>,
    /// What a file leaves out or approximates (SVG).
    pub warnings: Vec<String>,
}

/// Export `state` as `format` into `directory` (created when missing).
/// Single-file formats are named `<stem>.<ext>`.
pub fn export_document(
    state: &EditorState,
    format: HeadlessExportFormat,
    directory: &Path,
    stem: &str,
) -> Result<HeadlessExportReport, HeadlessExportError> {
    std::fs::create_dir_all(directory).map_err(|error| HeadlessExportError::CreateDir {
        path: directory.display().to_string(),
        detail: error.to_string(),
    })?;
    match format {
        HeadlessExportFormat::Png | HeadlessExportFormat::Svg => {
            export_frames(state, format, directory)
        }
        HeadlessExportFormat::Pdf => {
            let target = directory.join(format!("{stem}.pdf"));
            crate::export_pdf::export_deck_pdf(state, &target).map_err(|error| {
                HeadlessExportError::Render {
                    format: format.as_str(),
                    detail: error.to_string(),
                }
            })?;
            Ok(HeadlessExportReport {
                files: vec![target],
                ..HeadlessExportReport::default()
            })
        }
        HeadlessExportFormat::Html => export_html(state, directory, stem),
    }
}

fn export_frames(
    state: &EditorState,
    format: HeadlessExportFormat,
    directory: &Path,
) -> Result<HeadlessExportReport, HeadlessExportError> {
    let targets = plan_frame_exports(state, format.as_str());
    if targets.is_empty() {
        return Err(HeadlessExportError::NoFrames);
    }
    let (report, warnings) = if format == HeadlessExportFormat::Svg {
        export_frames_svg_to_dir(state, directory, &targets)
    } else {
        (export_frames_to_dir(state, directory, &targets), Vec::new())
    };
    Ok(HeadlessExportReport {
        files: report
            .written
            .iter()
            .map(|name| directory.join(name))
            .collect(),
        failed: report.failed,
        warnings,
    })
}

fn export_html(
    state: &EditorState,
    directory: &Path,
    stem: &str,
) -> Result<HeadlessExportReport, HeadlessExportError> {
    // `export_html` takes absolute paths only.
    let directory =
        std::path::absolute(directory).map_err(|error| HeadlessExportError::CreateDir {
            path: directory.display().to_string(),
            detail: error.to_string(),
        })?;
    let target = directory.join(format!("{stem}.html"));
    let args = BTreeMap::from([
        ("format".to_string(), "html-css".to_string()),
        ("outputPath".to_string(), target.display().to_string()),
    ]);
    let render_error = |detail: String| HeadlessExportError::Render {
        format: "html",
        detail,
    };
    match de_mcp::export_html_snapshot(state).call(&args) {
        ToolOutcome::OkJson(json) => {
            let value: serde_json::Value =
                serde_json::from_str(&json).map_err(|error| render_error(error.to_string()))?;
            let mut files = vec![target];
            files.extend(
                value["assets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .map(|asset| directory.join(asset)),
            );
            Ok(HeadlessExportReport {
                files,
                ..HeadlessExportReport::default()
            })
        }
        ToolOutcome::Err(_, message) => Err(render_error(message)),
        other => Err(render_error(format!("unexpected result: {other:?}"))),
    }
}

#[cfg(test)]
#[path = "headless_export_tests.rs"]
mod tests;
