use std::path::Path;

use serde_json::json;

use super::{flag_value, required_pos, Command, Flags};
use crate::cli_error::CliError;

pub(super) fn map_import_pen(positionals: &[String], flags: &Flags) -> Result<Command, CliError> {
    let pen_path = required_pos(
        positionals,
        1,
        "Usage: de import:pen <file.pen> [--out output.de]",
    )?;
    let out_path = flag_value(flags, "out").unwrap_or_else(|| pen_default_out_path(&pen_path));
    crate::app_control_cli::require_document_extension(Path::new(&out_path))?;
    Ok(Command::ImportPen { pen_path, out_path })
}

pub(super) fn pen_default_out_path(pen_path: &str) -> String {
    let base = pen_path
        .strip_suffix(de_util::document_file::PEN_DEV_SUFFIX)
        .unwrap_or(pen_path);
    de_util::document_file::file_name(base)
}

/// Convert a pen.dev `.pen` into a new `.de`, inlining the image files it
/// keeps in its folder. The source is never written.
pub(super) fn run_import_pen(pen_path: &str, out_path: &str) -> Result<String, CliError> {
    let src = std::fs::read_to_string(pen_path)
        .map_err(|e| CliError::Io(format!("read {pen_path:?}: {e}")))?;
    if de_pen_loader::pen_dev::sniff_pen_dev_version(&src).is_none() {
        return Err(CliError::Document(format!(
            "import {pen_path:?}: not a pen.dev document (schema 2.9 or later); open it directly instead"
        )));
    }
    // Content, not path text: `./design.pen` and `design.pen` are the same
    // file, and any other pen.dev document is off limits too.
    if de_pen_loader::pen_dev::sniff_pen_dev_file(Path::new(out_path)).is_some() {
        return Err(CliError::Document(format!(
            "import {pen_path:?}: {out_path:?} is a pen.dev document; pen.dev files are never overwritten"
        )));
    }
    let assets = de_pen_loader::pen_dev::folder_assets(Path::new(pen_path));
    let loaded = de_pen_loader::payload::load_canonical_with_assets(&src, &assets)
        .map_err(|e| CliError::Document(format!("import {pen_path:?}: {e}")))?;
    let report = loaded.compatibility.pen_dev.unwrap_or_default();
    let document = loaded.loaded.value;
    // Bitmaps land once each in `<out>.assets/`, the JSON indented like an
    // editor save so the result diffs line by line.
    let value = serde_json::to_value(&document)
        .map_err(|e| CliError::Payload(format!("serialize {out_path:?}: {e}")))?;
    de_pen_loader::asset_folder::write_document_value(Path::new(out_path), &value)
        .map_err(|e| crate::cli_error::write_error(out_path, e))?;
    Ok(json!({
        "ok": true,
        "filePath": out_path,
        "sourceVersion": report.source_version,
        "nodeCount": document.children.len(),
        "unsupported": report.unsupported,
        "missingImages": report.missing_images,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("de-cli-pen-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("images")).expect("create temp dir");
        dir
    }

    #[test]
    fn default_out_path_swaps_the_extension() {
        assert_eq!(pen_default_out_path("/tmp/design.pen"), "/tmp/design.de");
        assert_eq!(pen_default_out_path("/tmp/design"), "/tmp/design.de");
    }

    #[test]
    fn imports_a_pen_dev_file_with_its_images() {
        let dir = temp_dir("ok");
        std::fs::write(dir.join("images/a.png"), [0x89, b'P', b'N', b'G']).unwrap();
        let pen = dir.join("design.pen");
        std::fs::write(
            &pen,
            r#"{"version":"2.21","children":[{"type":"frame","id":"f",
                "fill":{"type":"image","url":"images/a.png"},"children":[{"type":"note","id":"n"}]}]}"#,
        )
        .unwrap();
        let out = dir.join("design.de");
        let result =
            run_import_pen(pen.to_str().unwrap(), out.to_str().unwrap()).expect("import succeeds");
        let result: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(result["sourceVersion"], "2.21");
        assert_eq!(result["unsupported"]["note"], 1);
        let written = std::fs::read_to_string(&out).unwrap();
        assert!(written.contains("\"layout\": \"horizontal\""));
        // The image lands in the output's asset folder, named by path.
        let files: Vec<_> = std::fs::read_dir(dir.join("design.assets"))
            .expect("asset folder")
            .map(|entry| entry.expect("entry").path())
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(std::fs::read(&files[0]).unwrap(), [0x89, b'P', b'N', b'G']);
        let name = files[0].file_name().unwrap().to_str().unwrap();
        assert!(written.contains(&format!("\"design.assets/{name}\"")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_to_overwrite_the_source_or_take_other_files() {
        let dir = temp_dir("refuse");
        let pen = dir.join("design.pen");
        std::fs::write(&pen, r#"{"version":"2.21","children":[]}"#).unwrap();
        let path = pen.to_str().unwrap();
        assert!(run_import_pen(path, path).is_err());
        let dotted = format!("{}/./design.pen", dir.display());
        assert!(
            run_import_pen(path, &dotted).is_err(),
            "same file, other spelling"
        );
        assert!(pen.exists());
        assert_eq!(
            std::fs::read_to_string(&pen).unwrap(),
            r#"{"version":"2.21","children":[]}"#
        );
        let op = dir.join("plain.de");
        std::fs::write(&op, r#"{"version":"1.0","children":[]}"#).unwrap();
        let out = dir.join("out.de");
        assert!(run_import_pen(op.to_str().unwrap(), out.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
