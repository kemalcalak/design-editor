//! Host capability flags survive whole-state swaps (#161).
//!
//! The desktop host advertises what it can do once at startup; Open / New /
//! a template then replace the whole `EditorState`, whose flags default to
//! off. These tests pin the seam so the missing-font Download action (and
//! the other host-backed controls) are still offered afterwards.

use de_editor_core::editor_ui_state::HostCapabilities;
use de_editor_core::missing_fonts::can_download;
use de_editor_core::EditorState;
use de_editor_ui::widgets::{MissingFontsHit, MissingFontsPanel};
use de_editor_ui::Point2D;

use super::WidgetHostNative;

/// A document whose only text uses a Google Fonts family the machine lacks.
fn document_using(family: &str) -> EditorState {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(&format!(
        r#"{{"version":"0.8.0","children":[
            {{"type":"text","id":"t1","name":"t","x":0,"y":0,"width":10,"height":10,
              "content":"hi","fontFamily":"{family}"}}]}}"#
    ))
    .expect("document");
    EditorState::from_document(doc)
}

/// A host the way desktop starts it: every capability on.
fn desktop_like_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.font_download_supported = true;
    ui.font_import_supported = true;
    ui.batch_frame_export_supported = true;
    ui.deck_html_export_supported = true;
    ui.slide_thumbnails_supported = true;
    ui.scene_template_generate_supported = true;
    ui.scene_template_center.save_current_supported = true;
    ui.style_import_file_picker_supported = true;
    host
}

fn download_button_offered(host: &WidgetHostNative) -> bool {
    let Some(panel) = MissingFontsPanel::for_editor(host.editor_state()) else {
        return false;
    };
    let rect = panel.rect(1200.0, 800.0);
    let viewport = de_editor_ui::Rect::xywh(0.0, 0.0, 1200.0, 800.0);
    (0..rect.size.x as i32).any(|dx| {
        let point = Point2D::new(rect.origin.x + dx as f32, rect.origin.y + 90.0);
        panel.hit_test(rect, viewport, point) == MissingFontsHit::Download(0)
    })
}

#[test]
fn replacing_the_editor_state_keeps_every_host_capability() {
    let mut host = desktop_like_host();
    let before = HostCapabilities::of(&host.editor_state().editor_ui);

    assert!(host.replace_editor_state(EditorState::starter()));

    assert_eq!(HostCapabilities::of(&host.editor_state().editor_ui), before);
}

#[test]
fn opened_document_still_offers_download_for_a_google_family() {
    let mut host = desktop_like_host();
    let mut state = document_using("Geologica");
    // Open's own bookkeeping carries font enumeration across; the
    // capability itself must come from the host seam.
    state.editor_ui.system_fonts_loaded = true;

    assert!(host.replace_editor_state(state));
    host.arm_missing_fonts_detection();

    let ui = &host.editor_state().editor_ui;
    let prompt = ui
        .missing_fonts_prompt
        .as_ref()
        .expect("Geologica is missing");
    assert!(ui.missing_fonts_modal_open);
    assert!(can_download(ui, &prompt.entries[0]));
    assert!(
        download_button_offered(&host),
        "the modal row must paint a Download button after Open"
    );
}

#[test]
fn a_host_without_downloads_does_not_gain_them_from_the_incoming_state() {
    let mut host = WidgetHostNative::new();
    let mut state = document_using("Geologica");
    state.editor_ui.font_download_supported = true;

    assert!(host.replace_editor_state(state));

    assert!(!host.editor_state().editor_ui.font_download_supported);
}
