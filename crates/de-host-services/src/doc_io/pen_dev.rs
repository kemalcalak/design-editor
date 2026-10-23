//! Filesystem side of the pen.dev import: reading a document's sibling
//! image files, and refusing to write the canonical schema over a pen.dev
//! file. The conversion itself is `de_pen_loader::pen_dev`.

use std::io::Read as _;
use std::path::Path;

use super::DocIoError;

/// How much of an existing save target is read to recognise a pen.dev file.
const SNIFF_BYTES: u64 = 512;

/// Reader for a document's sibling image files; see
/// `de_pen_loader::pen_dev::folder_assets`.
pub(super) fn document_assets(document: &Path) -> impl Fn(&str) -> Option<Vec<u8>> {
    de_pen_loader::pen_dev::folder_assets(document)
}

/// Fail when `path` holds a pen.dev document. Every save funnels through
/// here, so no transport (desktop, MCP `filePath`, CLI) can turn an import
/// source into a canonical file in place.
pub(super) fn refuse_overwrite(path: &Path) -> Result<(), DocIoError> {
    let Ok(file) = std::fs::File::open(path) else {
        return Ok(());
    };
    let mut head = Vec::new();
    if file.take(SNIFF_BYTES).read_to_end(&mut head).is_err() {
        return Ok(());
    }
    let head = match std::str::from_utf8(&head) {
        Ok(head) => head,
        // The read may stop inside a multi-byte character.
        Err(error) => std::str::from_utf8(&head[..error.valid_up_to()]).unwrap_or_default(),
    };
    if de_pen_loader::pen_dev::sniff_pen_dev_version(head).is_some() {
        return Err(DocIoError::PenDevSourceReadOnly {
            path: path.display().to_string(),
        });
    }
    Ok(())
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
        let read = document_assets(&dir.join("design.pen"));
        #[cfg(unix)]
        assert_eq!(read("images/link.png"), None);
        std::fs::write(dir.join("images/ok.png"), b"ok").unwrap();
        assert_eq!(read("images/ok.png"), Some(b"ok".to_vec()));
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(dir);
    }
}
