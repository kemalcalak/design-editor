//! MCP tool `export_deck` — writes the active page's boards as a
//! presentation deck in PowerPoint, self-contained HTML, or PDF, or as a
//! HyperFrames video composition.
//!
//! Input: `{ "format": "pptx"|"html"|"pdf"|"hyperframes", "outputPath": String }`
//!
//! Output: `{ "format": String, "path": String, "slides": Number,
//!            "structuredNodes"?: Number, "rasterFallbacks"?: Number }`,
//! plus for `hyperframes`: `{ "compositionPath": String,
//! "renderNotesPath": String, "durationSeconds": Number, "fps": Number,
//! "width": Number, "height": Number }`.
//!
//! **For `hyperframes`, `outputPath` is a directory.** The renderer treats
//! a directory holding one `index.html` as the project, so the exporter
//! writes `index.html` + `RENDER.md` inside it. A path naming an `.html`
//! file is refused rather than turned into `deck.html/index.html`.
//!
//! The deck exporters have been in the desktop app since `v0.8.3` but were
//! reachable only from its File menu, so an agent driving the editor over
//! MCP could build a deck and then had no way to hand it to anyone.
//!
//! **The output argument is `outputPath`, not `filePath`.** `filePath` is
//! reserved workspace-wide for "the `.de` document this call targets" and is
//! intercepted before dispatch (`mcp_serve/file_path.rs`); naming the
//! destination `filePath` would route the call at a document that does not
//! exist instead of writing a deck.
//!
//! Unlike `export_item` / `export_nodes` this tool always writes a file
//! rather than returning base64. A deck is a multi-megabyte artifact and the
//! caller's goal is a shareable file; base64 stays the node-scoped answer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use de_editor_core::EditorState;
use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};

use crate::export_html::export_deck_html;
use crate::export_hyperframes::{
    export_deck_hyperframes, HyperframesExport, FPS, TICKS_PER_SECOND,
};
use crate::export_pdf::export_deck_pdf;
use crate::export_pptx::export_deck_pptx;

pub struct ExportDeck {
    state: EditorState,
}

impl McpTool for ExportDeck {
    fn name(&self) -> &str {
        "export_deck"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let format = match args.get("format").map(|value| value.trim()) {
            Some(format) if !format.is_empty() => format,
            _ => {
                return ToolOutcome::Err(
                    ToolErrorCode::MissingArgument,
                    "format is required (pptx|html|pdf|hyperframes)".into(),
                );
            }
        };
        let output_path = match args.get("outputPath").map(|value| value.trim()) {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => {
                return ToolOutcome::Err(
                    ToolErrorCode::MissingArgument,
                    "outputPath is required — export_deck writes a file".into(),
                );
            }
        };

        match format {
            "pptx" => match export_deck_pptx(&self.state, &output_path) {
                Ok(summary) => ToolOutcome::OkJson(
                    deck_result(
                        "pptx",
                        &output_path,
                        summary.slides,
                        Some((summary.structured_nodes, summary.raster_fallbacks)),
                    )
                    .to_string(),
                ),
                Err(error) => export_failed(error),
            },
            "html" => match export_deck_html(&self.state, &output_path) {
                Ok(summary) => ToolOutcome::OkJson(
                    deck_result(
                        "html",
                        &output_path,
                        summary.slides,
                        Some((summary.structured_nodes, summary.raster_fallbacks)),
                    )
                    .to_string(),
                ),
                Err(error) => export_failed(error),
            },
            // The PDF writer reports success by returning, not by counting:
            // its slide total is the board count it just walked.
            "pdf" => match export_deck_pdf(&self.state, &output_path) {
                Ok(()) => {
                    let slides =
                        de_editor_core::preview_slideshow::active_page_boards(&self.state).len();
                    ToolOutcome::OkJson(deck_result("pdf", &output_path, slides, None).to_string())
                }
                Err(error) => export_failed(error),
            },
            "hyperframes" => {
                if names_an_html_file(&output_path) {
                    return ToolOutcome::Err(
                        ToolErrorCode::InvalidArgument,
                        "outputPath for hyperframes is a directory: the composition is \
                         written as <outputPath>/index.html"
                            .into(),
                    );
                }
                match export_deck_hyperframes(&self.state, &output_path) {
                    Ok(written) => {
                        ToolOutcome::OkJson(hyperframes_result(&output_path, &written).to_string())
                    }
                    Err(error) => export_failed(error),
                }
            }
            other => ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("unknown format {other:?}: must be one of pptx, html, pdf, hyperframes"),
            ),
        }
    }
}

/// Snapshot constructor. Registration is per-call and gated on the requested
/// tool (`should_register`), so this clone happens only when `export_deck` is
/// the tool being invoked — never on unrelated calls.
pub fn export_deck_snapshot(state: &EditorState) -> ExportDeck {
    ExportDeck {
        state: state.clone(),
    }
}

fn deck_result(
    format: &str,
    path: &Path,
    slides: usize,
    structured: Option<(usize, usize)>,
) -> serde_json::Value {
    let mut result = serde_json::json!({
        "format": format,
        "path": path.display().to_string(),
        "slides": slides,
    });
    // The PDF writer reports no structural split, so those keys are absent
    // rather than zero — a zero would claim every node rasterized.
    if let Some((structured_nodes, raster_fallbacks)) = structured {
        result["structuredNodes"] = structured_nodes.into();
        result["rasterFallbacks"] = raster_fallbacks.into();
    }
    result
}

/// The deck summary plus where the composition landed and how long it
/// runs, so the caller can hand `path` straight to `hyperframes render`.
fn hyperframes_result(dir: &Path, written: &HyperframesExport) -> serde_json::Value {
    let composition = &written.composition;
    let mut result = deck_result(
        "hyperframes",
        dir,
        composition.scenes,
        Some((composition.structured_nodes, composition.raster_fallbacks)),
    );
    result["compositionPath"] = written.composition_path.display().to_string().into();
    result["renderNotesPath"] = written.render_notes_path.display().to_string().into();
    // From the integer tick count in f64, not `total_seconds()`: an f32
    // 6.6 widens to 6.599999904632568 on the wire.
    result["durationSeconds"] =
        (f64::from(composition.total_ticks) / f64::from(TICKS_PER_SECOND)).into();
    result["fps"] = FPS.into();
    result["width"] = composition.width.into();
    result["height"] = composition.height.into();
    result
}

/// Whether `path` names an HTML file rather than a directory.
fn names_an_html_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
        })
}

fn export_failed(error: crate::export::ExportError) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::Internal, error.to_string())
}

#[cfg(test)]
#[path = "export_deck_tool_tests.rs"]
mod tests;
