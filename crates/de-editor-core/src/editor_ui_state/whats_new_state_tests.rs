use super::*;
use crate::editor_ui_state::EditorUiState;

/// A release that ships notes, read from the asset so no test names the
/// workspace version (`check-version-sync.sh`).
fn noted_release() -> String {
    release_notes::release_notes()[0].version.clone()
}

fn paged(versions: &[&str]) -> EditorUiState {
    EditorUiState {
        whats_new: Some(WhatsNewState {
            versions: versions.iter().map(|v| v.to_string()).collect(),
            page: 0,
            running: versions[0].to_string(),
            hover: None,
        }),
        ..Default::default()
    }
}

#[test]
fn a_fresh_install_shows_nothing_and_records_the_version() {
    let running = noted_release();
    let mut ui = EditorUiState::default();
    ui.open_whats_new_on_startup(false, &running);
    assert!(!ui.whats_new_visible());
    assert_eq!(ui.last_seen_version.as_deref(), Some(running.as_str()));
}

#[test]
fn an_upgrade_from_a_build_without_the_window_shows_this_release() {
    let running = noted_release();
    let mut ui = EditorUiState::default();
    ui.open_whats_new_on_startup(true, &running);
    let state = ui.whats_new.as_ref().unwrap();
    assert_eq!(state.versions, std::slice::from_ref(&running));
    assert_eq!(state.buttons(), [WhatsNewButton::GotIt]);
    // Not seen until it is closed.
    assert_eq!(ui.last_seen_version, None);
}

#[test]
fn an_upgrade_from_a_seen_release_shows_the_newer_notes() {
    let running = noted_release();
    let mut ui = EditorUiState {
        last_seen_version: Some("0.0.1".into()),
        ..Default::default()
    };
    ui.open_whats_new_on_startup(true, &running);
    let state = ui.whats_new.as_ref().unwrap();
    assert_eq!(state.versions[0], running);
    assert_eq!(state.page, 0);
}

#[test]
fn the_same_version_again_shows_nothing() {
    let running = noted_release();
    let mut ui = EditorUiState {
        last_seen_version: Some(running.clone()),
        ..Default::default()
    };
    ui.open_whats_new_on_startup(true, &running);
    assert!(!ui.whats_new_visible());
}

#[test]
fn a_release_without_notes_is_marked_seen_silently() {
    let mut ui = EditorUiState {
        last_seen_version: Some("999.0.0".into()),
        ..Default::default()
    };
    ui.open_whats_new_on_startup(true, "999.0.1");
    assert!(!ui.whats_new_visible());
    assert_eq!(ui.last_seen_version.as_deref(), Some("999.0.1"));
}

#[test]
fn a_downgrade_keeps_the_newer_seen_version() {
    let mut ui = EditorUiState {
        last_seen_version: Some("999.0.0".into()),
        ..Default::default()
    };
    ui.open_whats_new_on_startup(true, "1.0.0");
    assert!(!ui.whats_new_visible());
    assert_eq!(ui.last_seen_version.as_deref(), Some("999.0.0"));
}

#[test]
fn paging_walks_from_newest_to_oldest_and_stops_at_the_ends() {
    let mut ui = paged(&["3.0.0", "2.0.0", "1.0.0"]);
    let page = |ui: &EditorUiState| ui.whats_new.as_ref().unwrap().version().to_string();
    assert_eq!(page(&ui), "3.0.0");
    ui.press_whats_new_button(WhatsNewButton::Previous);
    assert_eq!(page(&ui), "3.0.0");
    ui.press_whats_new_button(WhatsNewButton::Next);
    ui.press_whats_new_button(WhatsNewButton::Next);
    assert_eq!(page(&ui), "1.0.0");
    assert!(!ui.whats_new.as_ref().unwrap().enabled(WhatsNewButton::Next));
    ui.press_whats_new_button(WhatsNewButton::Next);
    assert_eq!(page(&ui), "1.0.0");
    ui.press_whats_new_button(WhatsNewButton::Previous);
    assert_eq!(page(&ui), "2.0.0");
}

#[test]
fn got_it_closes_every_page_and_records_the_running_version() {
    let mut ui = paged(&["3.0.0", "2.0.0"]);
    ui.press_whats_new_button(WhatsNewButton::Next);
    ui.press_whats_new_button(WhatsNewButton::GotIt);
    assert!(!ui.whats_new_visible());
    assert_eq!(ui.last_seen_version.as_deref(), Some("3.0.0"));
}

#[test]
fn escape_closes_like_got_it() {
    let mut ui = paged(&["2.0.0"]);
    assert!(ui.escape_whats_new());
    assert_eq!(ui.last_seen_version.as_deref(), Some("2.0.0"));
    assert!(!ui.escape_whats_new());
}

#[test]
fn the_window_owns_the_pointer() {
    let ui = paged(&["2.0.0"]);
    assert!(ui.pointer_modal_open());
}

#[test]
fn an_update_offer_held_back_by_the_window_shows_after_it_closes() {
    use crate::editor_ui_state::{UpdateModalState, UpdatePhase};
    let mut ui = paged(&["2.0.0"]);
    let mut offer = UpdateModalState::new("3.0.0", UpdatePhase::Offer);
    offer.visible = false;
    ui.update_modal = Some(offer);
    ui.press_whats_new_button(WhatsNewButton::GotIt);
    assert!(ui.update_modal_visible());
}

#[test]
fn a_pre_release_build_leaves_everything_alone() {
    let mut ui = EditorUiState {
        last_seen_version: Some("1.0.0".into()),
        ..Default::default()
    };
    ui.open_whats_new_on_startup(true, "2.0.0-beta.1");
    assert!(!ui.whats_new_visible());
    assert_eq!(ui.last_seen_version.as_deref(), Some("1.0.0"));
}
