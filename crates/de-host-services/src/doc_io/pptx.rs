//! Opening a PowerPoint deck (#246): the `.pptx` converts into a new
//! untitled deck document, one board per slide, the way a pen.dev `.pen`
//! opens. The source is never written; the first save asks for a `.de`.

use super::load::{finish_loaded_state, localized_with_name};
use super::{DocIoError, DocumentLoadReport, LoadedEditorState};
use de_editor_core::EditorState;

pub(super) fn load_pptx(
    path: &std::path::Path,
    locale: de_editor_core::Locale,
) -> Result<LoadedEditorState, DocIoError> {
    let bytes = std::fs::read(path).map_err(|e| DocIoError::Io(e.to_string()))?;
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let import = de_pptx::import_pptx(&bytes, &name).map_err(|error| {
        DocIoError::NotPptxDeck(
            localized_with_name(locale, "dialog.loadErrorNotPptx", path)
                .replace("{{detail}}", &error.to_string()),
        )
    })?;
    let mut state = EditorState::from_document(import.document);
    state.tag_as_deck();
    Ok(LoadedEditorState {
        state: finish_loaded_state(state, None),
        report: DocumentLoadReport {
            pptx_import: Some(import.warnings),
            ..DocumentLoadReport::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::scene_template_catalog::TemplateScene;

    #[test]
    fn a_deck_opens_as_an_untitled_slides_document() {
        // A deck written by the editor's own exporter, opened back.
        let source = jian_ops_schema::load_str(
            r##"{"version":"1.0.0","children":[
                {"type":"frame","id":"a","x":0,"y":0,"width":960,"height":540,"layout":"none",
                 "children":[{"type":"text","id":"t","x":40,"y":40,"width":400,"height":60,
                              "content":"Hello deck","fontSize":32}]},
                {"type":"frame","id":"b","x":1100,"y":0,"width":960,"height":540,"layout":"none"}
            ]}"##,
        )
        .expect("fixture")
        .value;
        let (bytes, _) = crate::export_pptx::build_deck_pptx(&EditorState::from_document(source))
            .expect("export");
        let dir = std::env::temp_dir().join(format!("de-doc-io-pptx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("Deck.pptx");
        std::fs::write(&path, bytes).expect("write");

        let loaded =
            crate::doc_io::load_editor_state_with_report(&path, de_editor_core::Locale::EnUs)
                .expect("opens");
        assert!(loaded.report.is_import());
        assert_eq!(loaded.state.editor_ui.scenario, Some(TemplateScene::Slides));
        let boards = de_editor_core::preview_slideshow::active_page_boards(&loaded.state);
        assert_eq!(boards.len(), 2);

        // A save aimed at the deck (an MCP `filePath`) is refused.
        let before = std::fs::read(&path).expect("deck bytes");
        let err = crate::doc_io::save_to_path(&loaded.state, &path).expect_err("source is kept");
        assert!(
            matches!(err, DocIoError::PptxSourceReadOnly { .. }),
            "{err}"
        );
        assert_eq!(std::fs::read(&path).expect("deck bytes"), before);

        std::fs::write(&path, b"not a zip").expect("write");
        let Err(error) = crate::doc_io::load_editor_state(&path, de_editor_core::Locale::EnUs)
        else {
            panic!("garbage must not open");
        };
        assert!(matches!(error, DocIoError::NotPptxDeck(_)));
        assert!(error.to_string().contains("Deck.pptx"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
