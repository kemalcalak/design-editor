//! File ▸ Import ▸ PowerPoint in the browser (#246): pick a `.pptx`, read
//! its bytes, convert it with the same `de-pptx` importer the desktop
//! open path uses, and install it as a new unsaved deck.

use de_editor_core::EditorState;

use crate::file_actions;
use crate::repaint_ctx::RepaintContext;

use super::import_generation::{begin_document_replacement, document_replacement_is_current};
use super::{console_error, js_bytes, open_file_picker, read_file, InnerRc, ReadMode};

pub(super) fn open_pptx_document<C: RepaintContext + 'static>(inner: &InnerRc<C>) {
    let inner = inner.clone();
    open_file_picker(
        de_util::document_file::PPTX_SUFFIX,
        Box::new(move |file| read_pptx_file(&inner, file)),
    );
}

/// Read a picked or dropped deck and install it; File ▸ Open and drops
/// route a `.pptx` here (`read_open_document_file`).
pub(super) fn read_pptx_file<C: RepaintContext + 'static>(inner: &InnerRc<C>, file: web_sys::File) {
    let name = file.name();
    let generation = begin_document_replacement(inner);
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
            // Parse outside any `inner` borrow: a deck with media takes a
            // moment.
            match de_pptx::import_pptx(&bytes, file_actions::file_stem(&name)) {
                Ok(import) => install(&inner, import),
                Err(error) => console_error(&format!("[import-pptx] {name}: {error}")),
            }
        }),
    );
}

fn install<C: RepaintContext + 'static>(inner: &InnerRc<C>, import: de_pptx::PptxImport) {
    let mut b = inner.borrow_mut();
    let mut state = EditorState::from_document(import.document);
    // An import, not a file this tab can write back to.
    state.editor_ui.file_name_display = None;
    b.host_mut().install_unsaved_ingested_state(state);
    // Installing keeps the live chrome, scenario included; the deck tag
    // belongs to the new document, so it goes on after.
    b.host_mut().editor_state_mut().tag_as_deck();
    if !import.warnings.is_empty() {
        b.host_mut().editor_state_mut().editor_ui.show_toast(
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
