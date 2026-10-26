//! File ▸ Import ▸ PowerPoint in the browser (#246): pick a `.pptx`, read
//! its bytes, convert it with the same `de-pptx` importer the desktop
//! open path uses, and install it as a new unsaved deck.

use de_editor_core::scene_template_catalog::TemplateScene;
use de_editor_core::EditorState;

use crate::file_actions;
use crate::repaint_ctx::RepaintContext;

use super::import_generation::{begin_document_replacement, document_replacement_is_current};
use super::{console_error, js_bytes, open_file_picker, read_file, InnerRc, ReadMode};

pub(super) fn open_pptx_document<C: RepaintContext + 'static>(inner: &InnerRc<C>) {
    let inner = inner.clone();
    open_file_picker(
        de_util::document_file::PPTX_SUFFIX,
        Box::new(move |file| {
            let name = file.name();
            let generation = begin_document_replacement(&inner);
            let inner = inner.clone();
            read_file(
                file,
                ReadMode::Bytes,
                Box::new(move |value| {
                    if !document_replacement_is_current(&inner, generation) {
                        return;
                    }
                    let Some(bytes) = js_bytes(&value) else {
                        console_error(&format!(
                            "[import-pptx] {name}: file read produced no bytes"
                        ));
                        return;
                    };
                    // Parse outside any `inner` borrow: a deck with media
                    // takes a moment.
                    let stem = file_actions::file_stem(&name);
                    match de_pptx::import_pptx(&bytes, stem) {
                        Ok(import) => install(&inner, import),
                        Err(error) => console_error(&format!("[import-pptx] {name}: {error}")),
                    }
                }),
            );
        }),
    );
}

fn install<C: RepaintContext + 'static>(inner: &InnerRc<C>, import: de_pptx::PptxImport) {
    let mut b = inner.borrow_mut();
    let mut state = EditorState::from_document(import.document);
    // An import, not a file this tab can write back to.
    state.editor_ui.file_name_display = None;
    b.host_mut().install_unsaved_ingested_state(state);
    let ui = &mut b.host_mut().editor_state_mut().editor_ui;
    // Installing keeps the live chrome; the deck tag is the document's.
    ui.scenario = Some(TemplateScene::Slides);
    if !import.warnings.is_empty() {
        ui.show_toast(
            "dialog.pptxImportToast",
            vec![("details".to_owned(), import.warnings.join(", "))],
            de_editor_core::editor_toast::EditorToastLevel::Warn,
            crate::listener::now_ms_perf(),
        );
    }
    let (w, h) = b.viewport_size();
    b.host_mut().fit_content_to_viewport(w, h);
    let _ = b.repaint();
}
