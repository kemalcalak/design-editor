//! PPTX → boards → PPTX: the imported deck goes back out through the
//! editor's own `export_deck` writers and reads in again unchanged where
//! the export is structured (slide count, size, text).

use de_editor_core::scene_template_catalog::TemplateScene;
use de_editor_core::EditorState;
use serde_json::Value;

use super::{all_text, boards, fixture};
use crate::import_pptx;

fn deck_state() -> EditorState {
    let import = import_pptx(&fixture::deck(), "Fixture").expect("fixture imports");
    let mut state = EditorState::from_document(import.document);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    state
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("de-pptx-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn imported_deck_exports_back_to_pptx_and_reimports() {
    let state = deck_state();
    let (bytes, summary) =
        de_host_services::export_pptx::build_deck_pptx(&state).expect("pptx export");
    // The hidden third slide is not part of the deck.
    assert_eq!(summary.slides, 2);
    assert!(summary.structured_nodes > 10, "{summary:?}");

    let again = import_pptx(&bytes, "Again").expect("re-import");
    assert_eq!(again.slides, 2);
    let doc = serde_json::to_value(&again.document).expect("serialize");
    let boards = boards(&doc);
    assert_eq!(boards[0]["width"], 1280.0);
    assert_eq!(boards[0]["height"], 720.0);
    let first: String = all_text(boards[0]);
    assert!(first.contains("Quarterly review"), "{first}");
    assert!(first.contains("Revenue up"), "{first}");
    let second: String = all_text(boards[1]);
    for text in ["Plain link", "In a card", "Q1", "12"] {
        assert!(second.contains(text), "{text:?} in {second}");
    }
}

#[test]
fn imported_deck_exports_to_html_and_pdf() {
    let state = deck_state();
    let dir = temp("export");
    let html = dir.join("deck.html");
    let summary = de_host_services::export_html::export_deck_html(&state, &html).expect("html");
    assert_eq!(summary.slides, 2);
    let page = std::fs::read_to_string(&html).expect("html written");
    assert!(page.contains("Quarterly review"));

    let pdf = dir.join("deck.pdf");
    de_host_services::export_pdf::export_deck_pdf(&state, &pdf).expect("pdf");
    let bytes = std::fs::read(&pdf).expect("pdf written");
    assert!(bytes.starts_with(b"%PDF"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn imported_document_is_a_valid_de_document() {
    let import = import_pptx(&fixture::deck(), "Fixture").expect("fixture imports");
    let src = serde_json::to_string(&import.document).expect("serialize");
    let reloaded = jian_ops_schema::load_str(&src).expect("loads as .de").value;
    let value: Value = serde_json::to_value(&reloaded).expect("serialize");
    assert_eq!(boards(&value).len(), 3);
}
