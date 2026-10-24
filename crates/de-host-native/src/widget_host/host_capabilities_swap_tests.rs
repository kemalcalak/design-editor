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

/// A document with one text node per Google Fonts family the machine lacks.
fn document_using(families: &[&str]) -> EditorState {
    let children: Vec<String> = families
        .iter()
        .enumerate()
        .map(|(i, family)| {
            format!(
                r#"{{"type":"text","id":"t{i}","name":"t","x":0,"y":0,"width":10,"height":10,
                  "content":"hi","fontFamily":"{family}"}}"#
            )
        })
        .collect();
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(&format!(
        r#"{{"version":"0.8.0","children":[{}]}}"#,
        children.join(",")
    ))
    .expect("document");
    EditorState::from_document(doc)
}

/// A host with every capability on, as desktop advertises at startup
/// (`de-host-desktop/src/app_state.rs`).
fn desktop_like_host() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    // A struct literal, so a flag added to `HostCapabilities` must be
    // switched on here too.
    HostCapabilities {
        font_import: true,
        font_download: true,
        batch_frame_export: true,
        deck_html_export: true,
        slide_thumbnails: true,
        scene_template_generate: true,
        scene_template_save_current: true,
        style_import_file_picker: true,
    }
    .apply_to(&mut host.editor_state_mut().editor_ui);
    host
}

/// Whether a horizontal scan at `y` (panel-relative) hits `want`.
fn modal_offers(host: &WidgetHostNative, want: MissingFontsHit, y: f32) -> bool {
    let Some(panel) = MissingFontsPanel::for_editor(host.editor_state()) else {
        return false;
    };
    let rect = panel.rect(1200.0, 800.0);
    let viewport = de_editor_ui::Rect::xywh(0.0, 0.0, 1200.0, 800.0);
    let y = if y < 0.0 { rect.size.y + y } else { y };
    (0..rect.size.x as i32).any(|dx| {
        let point = Point2D::new(rect.origin.x + dx as f32, rect.origin.y + y);
        panel.hit_test(rect, viewport, point) == want
    })
}

/// y = 90 crosses the first family row's button band, as in
/// `missing_fonts_dispatch`'s tests.
fn download_button_offered(host: &WidgetHostNative) -> bool {
    modal_offers(host, MissingFontsHit::Download(0), 90.0)
}

/// 30 px above the panel's bottom edge crosses the footer actions.
fn download_all_offered(host: &WidgetHostNative) -> bool {
    modal_offers(host, MissingFontsHit::DownloadAll, -30.0)
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
    let mut state = document_using(&["Geologica", "Mona Sans"]);
    // Open's own bookkeeping carries font enumeration across; the
    // capability itself must come from the host seam.
    state.editor_ui.system_fonts_loaded = true;

    assert!(host.replace_editor_state(state));
    host.arm_missing_fonts_detection();

    let ui = &host.editor_state().editor_ui;
    let prompt = ui
        .missing_fonts_prompt
        .as_ref()
        .expect("both families are missing");
    assert!(ui.missing_fonts_modal_open);
    assert_eq!(prompt.entries.len(), 2);
    assert!(prompt.entries.iter().all(|entry| can_download(ui, entry)));
    assert!(
        download_button_offered(&host),
        "the modal row must paint a Download button after Open"
    );
    assert!(
        download_all_offered(&host),
        "with two downloadable rows the footer offers Download all"
    );
}

#[test]
fn a_host_without_downloads_does_not_gain_them_from_the_incoming_state() {
    let mut host = WidgetHostNative::new();
    let mut state = document_using(&["Geologica"]);
    state.editor_ui.font_download_supported = true;

    assert!(host.replace_editor_state(state));

    assert!(!host.editor_state().editor_ui.font_download_supported);
}
