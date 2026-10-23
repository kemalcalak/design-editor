//! Save-side guard for the pen.dev import: refuse to write the canonical
//! schema over a pen.dev file. The conversion, the image reader and the
//! detection rule live in `de_pen_loader::pen_dev`.

use std::path::Path;

use super::DocIoError;

/// Fail when `path` holds a pen.dev document. Every `doc_io` writer (the
/// streaming save and both clean-copy paths) calls this before staging, so
/// no desktop or MCP save can turn an import source into a canonical file.
pub(super) fn refuse_overwrite(path: &Path) -> Result<(), DocIoError> {
    match de_pen_loader::pen_dev::sniff_pen_dev_file(path) {
        Some(_) => Err(DocIoError::PenDevSourceReadOnly {
            path: path.display().to_string(),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "de-pen-dev-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(dir.join("images")).expect("create temp dir");
        dir
    }

    #[test]
    fn opening_a_pen_dev_file_inlines_its_sibling_images_and_never_binds_it() {
        let dir = temp_dir("open");
        std::fs::write(dir.join("images/a.png"), [0x89, b'P', b'N', b'G']).unwrap();
        let path = dir.join("design.pen");
        std::fs::write(
            &path,
            r##"{"version":"2.21","children":[{"type":"frame","id":"f","width":10,"height":10,
                "fill":[{"type":"image","url":"images/a.png"},{"type":"image","url":"images/b.png"}],
                "children":[{"type":"note","id":"n"}]}]}"##,
        )
        .unwrap();

        let loaded =
            super::super::load_editor_state_with_report(&path, de_editor_core::Locale::EnUs)
                .expect("pen.dev file opens");
        let report = loaded
            .report
            .pen_dev_import
            .clone()
            .expect("pen.dev report");
        assert_eq!(report.missing_images, vec!["images/b.png"]);
        assert_eq!(report.unsupported.get("note"), Some(&1));
        assert!(!loaded.report.needs_schema_upgrade());

        let err = super::super::save_to_path(&loaded.state, &path).expect_err("source is kept");
        assert!(matches!(err, DocIoError::PenDevSourceReadOnly { .. }));
        let copy = dir.join("design.op");
        super::super::save_to_path(&loaded.state, &copy).expect("save to a new file");
        let saved = std::fs::read_to_string(&copy).unwrap();
        assert!(saved.contains("data:image/png;base64,iVBORw=="));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_clean_copy_cannot_land_on_a_pen_dev_file() {
        let dir = temp_dir("clean-copy");
        let source = dir.join("doc.op");
        std::fs::write(&source, r#"{"version":"1.0","children":[]}"#).unwrap();
        let pen = dir.join("design.pen");
        let original = r#"{"version":"2.21","children":[]}"#;
        std::fs::write(&pen, original).unwrap();
        let err = super::super::copy_clean_document_with_editor_meta_to_path(
            &source,
            &pen,
            de_pen_loader::EditorMeta::default(),
        )
        .expect_err("pen.dev target is refused");
        assert!(matches!(err, DocIoError::PenDevSourceReadOnly { .. }));
        assert_eq!(std::fs::read_to_string(&pen).unwrap(), original);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn canonical_files_can_still_be_overwritten() {
        let dir = temp_dir("canonical");
        let path = dir.join("doc.op");
        std::fs::write(&path, r#"{"version":"1.0","children":[]}"#).unwrap();
        assert!(refuse_overwrite(&path).is_ok());
        assert!(refuse_overwrite(&dir.join("missing.op")).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn images_outside_the_document_folder_are_not_read() {
        let dir = temp_dir("escape");
        let outside = dir.with_extension("secret.png");
        std::fs::write(&outside, b"secret").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, dir.join("images/link.png")).unwrap();
        let read = de_pen_loader::pen_dev::folder_assets(&dir.join("design.pen"));
        #[cfg(unix)]
        assert_eq!(read("images/link.png"), None);
        std::fs::write(dir.join("images/ok.png"), b"ok").unwrap();
        assert_eq!(read("images/ok.png"), Some(b"ok".to_vec()));
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(dir);
    }
}
