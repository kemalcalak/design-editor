use std::path::Path;

use serde_json::json;

use super::{flag_value, required_pos, Command, Flags};
use crate::cli_error::CliError;

pub(super) fn map_import_figma(positionals: &[String], flags: &Flags) -> Result<Command, CliError> {
    let fig_path = required_pos(
        positionals,
        1,
        "Usage: de import:figma <file.fig> [--out output.de]",
    )?;
    let out_path = flag_value(flags, "out").unwrap_or_else(|| figma_default_out_path(&fig_path));
    crate::app_control_cli::require_document_extension(std::path::Path::new(&out_path))?;
    Ok(Command::ImportFigma { fig_path, out_path })
}

pub(super) fn figma_default_out_path(fig_path: &str) -> String {
    fig_path
        .strip_suffix(".fig")
        .map(de_util::document_file::file_name)
        .unwrap_or_else(|| fig_path.to_string())
}

pub(super) fn run_import_figma(fig_path: &str, out_path: &str) -> Result<String, CliError> {
    let bytes =
        std::fs::read(fig_path).map_err(|e| CliError::Io(format!("read {fig_path:?}: {e}")))?;
    let file_name = Path::new(fig_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Figma Import");
    let import =
        de_figma::parse_fig_binary(&bytes, file_name, de_figma::FigLayoutMode::DesignEditor)
            .map_err(|e| CliError::Document(format!("import {fig_path:?}: {e}")))?;
    // An image-heavy `.fig` references the same bitmap from many fills;
    // each one lands once in `<out>.assets/` and every fill names that file.
    let value = serde_json::to_value(&import.document)
        .map_err(|e| CliError::Payload(format!("serialize {out_path:?}: {e}")))?;
    de_pen_loader::asset_folder::write_document_value(Path::new(out_path), &value)
        .map_err(|e| crate::cli_error::write_error(out_path, e))?;
    let page_count = import.document.pages.as_ref().map_or(1, Vec::len);
    let node_count = import
        .document
        .pages
        .as_ref()
        .map(|pages| pages.iter().map(|p| p.children.len()).sum())
        .unwrap_or_else(|| import.document.children.len());
    Ok(import_figma_success_json(
        out_path,
        page_count,
        node_count,
        &import.warnings,
    ))
}

fn import_figma_success_json(
    file_path: &str,
    page_count: usize,
    node_count: usize,
    warnings: &[String],
) -> String {
    json!({
        "ok": true,
        "filePath": file_path,
        "pageCount": page_count,
        "nodeCount": node_count,
        "warnings": warnings,
    })
    .to_string()
}
