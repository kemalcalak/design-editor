use std::path::Path;

use de_editor_core::scene_template_catalog::TemplateScene;
use serde_json::json;

use super::{flag_value, required_pos, Command, Flags};
use crate::cli_error::CliError;

pub(super) fn map_import_pptx(positionals: &[String], flags: &Flags) -> Result<Command, CliError> {
    let pptx_path = required_pos(
        positionals,
        1,
        "Usage: de import:pptx <file.pptx> [--out output.de]",
    )?;
    let out_path = flag_value(flags, "out").unwrap_or_else(|| pptx_default_out_path(&pptx_path));
    crate::app_control_cli::require_document_extension(Path::new(&out_path))?;
    Ok(Command::ImportPptx {
        pptx_path,
        out_path,
    })
}

pub(super) fn pptx_default_out_path(pptx_path: &str) -> String {
    de_util::document_file::file_name(de_util::document_file::strip_known_suffix(pptx_path))
}

/// Convert a PowerPoint deck into a new `.de`: one board per slide, tagged
/// as a deck so Preview presents it slide by slide (#246). The source is
/// never written.
pub(super) fn run_import_pptx(pptx_path: &str, out_path: &str) -> Result<String, CliError> {
    let bytes =
        std::fs::read(pptx_path).map_err(|e| CliError::Io(format!("read {pptx_path:?}: {e}")))?;
    let name = Path::new(pptx_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Deck");
    let import = de_pptx::import_pptx(&bytes, name)
        .map_err(|e| CliError::Document(format!("import {pptx_path:?}: {e}")))?;
    let mut value = serde_json::to_value(&import.document)
        .map_err(|e| CliError::Payload(format!("serialize {out_path:?}: {e}")))?;
    let meta = de_pen_loader::EditorMeta {
        scenario: Some(TemplateScene::Slides),
        ..de_pen_loader::EditorMeta::default()
    };
    value["editorMeta"] = serde_json::to_value(meta)
        .map_err(|e| CliError::Payload(format!("serialize {out_path:?}: {e}")))?;
    // Slide pictures land once each in `<out>.assets/`, like a Figma import.
    de_pen_loader::asset_folder::write_document_value(Path::new(out_path), &value)
        .map_err(|e| crate::cli_error::write_error(out_path, e))?;
    Ok(json!({
        "ok": true,
        "filePath": out_path,
        "slides": import.slides,
        "nodeCount": count_nodes(&import.document.children),
        "warnings": import.warnings,
    })
    .to_string())
}

/// Every node of the imported boards and notes, not just the roots.
fn count_nodes(nodes: &[jian_ops_schema::node::PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            1 + de_editor_core::PenNodeExt::children(node).map_or(0, |kids| count_nodes(kids))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_out_path_swaps_the_extension() {
        assert_eq!(pptx_default_out_path("/tmp/Deck.pptx"), "/tmp/Deck.de");
        assert_eq!(pptx_default_out_path("/tmp/Deck.PPTX"), "/tmp/Deck.de");
        assert_eq!(pptx_default_out_path("/tmp/deck"), "/tmp/deck.de");
    }

    #[test]
    fn a_non_deck_is_refused() {
        let dir = std::env::temp_dir().join(format!("de-cli-pptx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let source = dir.join("broken.pptx");
        std::fs::write(&source, b"not a zip").expect("write");
        let out = dir.join("broken.de");
        let error = run_import_pptx(&source.display().to_string(), &out.display().to_string())
            .expect_err("garbage must not import");
        assert!(
            error.to_string().contains("not a PowerPoint file"),
            "{error}"
        );
        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
