use super::*;
use crate::widgets::whats_new_flow;
use de_editor_core::release_notes::release_notes;
use de_i18n::Locale;

/// A release that ships notes, read from the asset so no test names the
/// workspace version (`check-version-sync.sh`).
fn noted_release() -> String {
    release_notes()[0].version.clone()
}

/// The window over `pages` copies of the embedded release (paging only
/// needs a count; every page resolves to real notes).
fn state_with(pages: usize) -> EditorState {
    let version = noted_release();
    let mut state = EditorState::default();
    state.editor_ui.whats_new = Some(WhatsNewState {
        versions: vec![version.clone(); pages],
        page: 0,
        running: version,
        hover: None,
    });
    state
}

fn centre(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

fn button(state: &EditorState, which: WhatsNewButton) -> Option<Rect> {
    let modal = WhatsNewModal::for_editor(state).unwrap();
    modal
        .button_rects(modal.rect(1200.0, 800.0))
        .into_iter()
        .find(|(b, _)| *b == which)
        .map(|(_, rect)| rect)
}

#[test]
fn closed_window_is_not_painted() {
    let mut state = state_with(1);
    assert!(WhatsNewModal::for_editor(&state).is_some());
    state.editor_ui.whats_new = None;
    assert!(WhatsNewModal::for_editor(&state).is_none());
}

#[test]
fn one_release_shows_only_got_it() {
    let state = state_with(1);
    assert!(button(&state, WhatsNewButton::Previous).is_none());
    assert!(button(&state, WhatsNewButton::Next).is_none());
    assert!(button(&state, WhatsNewButton::GotIt).is_some());
}

#[test]
fn paging_buttons_sit_left_of_got_it_inside_the_card() {
    let state = state_with(3);
    let modal = WhatsNewModal::for_editor(&state).unwrap();
    let card = modal.rect(1200.0, 800.0);
    let rects = modal.button_rects(card);
    assert_eq!(rects.len(), 3);
    let (prev, next, got) = (rects[0].1, rects[1].1, rects[2].1);
    assert!(prev.origin.x + prev.size.x < next.origin.x);
    // Room for the "1 / 3" counter between the pair and "Got it".
    assert!(next.origin.x + next.size.x + 40.0 < got.origin.x);
    assert!(got.origin.x + got.size.x <= card.origin.x + card.size.x - PAD + 0.01);
}

#[test]
fn every_locale_fits_its_buttons_in_the_card() {
    for locale in Locale::ALL {
        let mut state = state_with(3);
        state.editor_ui.locale = locale;
        let modal = WhatsNewModal::for_editor(&state).unwrap();
        let card = modal.rect(1200.0, 800.0);
        let rects = modal.button_rects(card);
        let next = rects[1].1;
        let got = rects[2].1;
        assert!(
            next.origin.x + next.size.x + 40.0 < got.origin.x,
            "{} paging overlaps Got it",
            locale.code()
        );
    }
}

#[test]
fn every_locale_wraps_inside_the_card_and_fits_the_viewport() {
    for locale in Locale::ALL {
        let mut state = state_with(1);
        state.editor_ui.locale = locale;
        let modal = WhatsNewModal::for_editor(&state).unwrap();
        let card = modal.rect(1200.0, 800.0);
        assert!(
            card.size.y < 800.0 - 32.0,
            "{} card too tall",
            locale.code()
        );
        let inner = card.size.x - PAD * 2.0;
        let lines = modal.lines(inner);
        for line in lines.items.iter().flatten().chain(&lines.intro) {
            assert!(
                estimate_width(line, BODY_SIZE) <= inner,
                "{}: {line}",
                locale.code()
            );
        }
    }
}

#[test]
fn an_unspaced_run_breaks_between_characters_but_not_before_marks() {
    let thai = "หลังการอัปเดตแต่ละครั้งหน้าต่างนี้จะแสดงสิ่งที่เปลี่ยนไปเป็นภาษาของคุณ";
    let lines = wrap(thai, 120.0, BODY_SIZE);
    assert!(lines.len() > 1);
    assert_eq!(lines.concat(), thai);
    for pair in lines.windows(2) {
        let first = pair[1].chars().next().unwrap() as u32;
        let last = pair[0].chars().last().unwrap() as u32;
        let mark = matches!(first, 0x0E31 | 0x0E33..=0x0E3A | 0x0E47..=0x0E4E);
        assert!(!mark, "line starts with a mark: {}", pair[1]);
        assert!(
            !(0x0E40..=0x0E44).contains(&last),
            "line ends on a leading vowel: {}",
            pair[0]
        );
    }
}

#[test]
fn cjk_breaks_between_characters_instead_of_at_the_last_space() {
    let text = "Claude で作ったものを含む HTML のスライドを開くと、各スライドが編集できるボードになります。";
    let lines = wrap(text, 300.0, BODY_SIZE);
    // A space-first break would end the first line at "HTML" with room left.
    assert!(
        estimate_width(&lines[0], BODY_SIZE) > 300.0 - BODY_SIZE * 1.5,
        "{lines:?}"
    );
}

#[test]
fn the_card_keeps_one_height_while_paging_and_fits_a_short_window() {
    let mut state = state_with(2);
    let before = WhatsNewModal::for_editor(&state)
        .unwrap()
        .rect(1200.0, 800.0);
    state.editor_ui.whats_new.as_mut().unwrap().page = 1;
    let after = WhatsNewModal::for_editor(&state)
        .unwrap()
        .rect(1200.0, 800.0);
    assert_eq!(before, after);

    let modal = WhatsNewModal::for_editor(&state).unwrap();
    let card = modal.rect(1200.0, 240.0);
    assert!(card.origin.y + card.size.y <= 240.0 - 16.0 + 0.01);
    let got = button(&state, WhatsNewButton::GotIt).unwrap();
    assert!(got.size.y > 0.0);
    let short_got = modal
        .button_rects(card)
        .into_iter()
        .find(|(b, _)| *b == WhatsNewButton::GotIt)
        .unwrap()
        .1;
    assert!(short_got.origin.y >= card.origin.y);
}

#[test]
fn a_release_without_notes_is_not_painted() {
    let mut state = state_with(1);
    state.editor_ui.whats_new.as_mut().unwrap().versions = vec!["999.0.0".into()];
    assert!(WhatsNewModal::for_editor(&state).is_none());
}

#[test]
fn press_pages_closes_and_swallows_the_scrim() {
    let mut state = state_with(2);
    assert_eq!(
        whats_new_flow::press(&mut state, Point2D::new(2.0, 2.0), 1200.0, 800.0),
        Some(false)
    );
    let next = button(&state, WhatsNewButton::Next).unwrap();
    whats_new_flow::press(&mut state, centre(next), 1200.0, 800.0);
    assert_eq!(state.editor_ui.whats_new.as_ref().unwrap().page, 1);

    let got = button(&state, WhatsNewButton::GotIt).unwrap();
    assert_eq!(
        whats_new_flow::press(&mut state, centre(got), 1200.0, 800.0),
        Some(true)
    );
    assert!(state.editor_ui.whats_new.is_none());
    assert_eq!(state.editor_ui.last_seen_version, Some(noted_release()));
    assert_eq!(
        whats_new_flow::press(&mut state, centre(got), 1200.0, 800.0),
        None
    );
}

#[test]
fn hover_tracks_the_button_under_the_pointer() {
    let mut state = state_with(1);
    let got = button(&state, WhatsNewButton::GotIt).unwrap();
    assert_eq!(
        whats_new_flow::hover(&mut state, centre(got), 1200.0, 800.0),
        Some(true)
    );
    assert_eq!(
        state.editor_ui.whats_new.as_ref().unwrap().hover,
        Some(WhatsNewButton::GotIt)
    );
    assert_eq!(
        whats_new_flow::hover(&mut state, centre(got), 1200.0, 800.0),
        Some(false)
    );
    assert_eq!(
        whats_new_flow::hover(&mut state, Point2D::new(1.0, 1.0), 1200.0, 800.0),
        Some(true)
    );
}
