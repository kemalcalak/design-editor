//! Import card titles (#143): wrap or ellipsize, never clip.

use super::*;
use crate::widgets::home_surface::layout::{IMPORT_H, SIDEBAR_W};
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::Widget;
use crate::RenderBackend;
use de_editor_core::{EditorState, Locale};

/// 10 px per character, spaces included: easy widths to reason about.
fn mono(text: &str) -> f32 {
    text.chars().count() as f32 * 10.0
}

#[test]
fn a_title_that_fits_stays_on_one_line() {
    assert_eq!(
        wrap_title("Import a file", 200.0, 2, mono),
        ["Import a file"]
    );
}

#[test]
fn a_long_title_breaks_at_the_last_space_that_fits() {
    let lines = wrap_title("Ekran görüntüsünü yeniden oluştur", 200.0, 2, mono);
    assert_eq!(lines, ["Ekran görüntüsünü", "yeniden oluştur"]);
}

#[test]
fn a_space_right_at_the_edge_is_still_a_break() {
    // "abcd efgh" is 90 px; "abcd" alone fills the 40 px line exactly.
    assert_eq!(wrap_title("abcd efgh", 40.0, 2, mono), ["abcd", "efgh"]);
}

#[test]
fn text_past_the_last_line_ends_in_an_ellipsis() {
    let lines = wrap_title("one two three four five six", 90.0, 2, mono);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "one two");
    assert!(lines[1].ends_with('…'), "{lines:?}");
    assert!(lines.iter().all(|line| mono(line) <= 90.0));
}

#[test]
fn a_one_line_card_ellipsizes_instead_of_wrapping() {
    let lines = wrap_title("Recreate a screenshot", 100.0, 1, mono);
    assert_eq!(lines, ["Recreate …"]);
}

#[test]
fn cjk_breaks_between_characters() {
    let lines = wrap_title("スクリーンショットを再現", 60.0, 2, mono);
    assert_eq!(lines[0], "スクリーンシ");
    assert!(lines.iter().all(|line| mono(line) <= 60.0));
}

#[test]
fn a_character_break_keeps_thai_marks_with_their_base() {
    // "ส ร ้ า ง": the tone mark U+0E49 must not start the second line.
    let lines = wrap_title("สร้างภาพหน้าจอใหม่", 20.0, 2, mono);
    assert!(!lines[1].starts_with('\u{0E49}'), "{lines:?}");
    assert_eq!(lines[0], "ส");
}

fn home_state(locale: Locale) -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.locale = locale;
    state.editor_ui.home.show(false);
    state
}

/// The narrowest window for each import arrangement: one row of four
/// (a 920 px column), two by two (a 720 px column), stacked (the 640 px
/// minimum window), plus the 1440 px frame.
fn narrowest_widths() -> [f32; 4] {
    [
        920.0 + 2.0 * 32.0 + SIDEBAR_W,
        720.0 + 2.0 * 32.0 + SIDEBAR_W,
        640.0,
        1440.0,
    ]
}

#[test]
fn no_import_title_leaves_its_text_box_in_any_locale() {
    for locale in Locale::ALL {
        let state = home_state(locale);
        let home = HomeSurface::for_editor(&state).unwrap();
        let stacked = home.layout(640.0, 900.0);
        assert_eq!(
            stacked.imports[0].size.x, stacked.column.size.x,
            "640 px stacks"
        );
        for w in narrowest_widths() {
            let mut backend = CaptureBackend::default();
            let layout = home.layout(w, 900.0);
            home.paint(
                &mut PaintCx {
                    backend: &mut backend,
                },
                Rect::xywh(0.0, 0.0, w, 900.0),
            );
            let height = layout.imports[0].size.y;
            for card in layout.imports {
                assert_eq!(card.size.y, height, "{locale:?} at {w}: one card height");
                let text_box = import_text_box(card);
                let texts: Vec<(String, Point2D)> = backend
                    .texts
                    .iter()
                    .filter(|(_, at)| {
                        at.x == text_box.origin.x
                            && at.y > card.origin.y
                            && at.y < card.origin.y + card.size.y
                    })
                    .cloned()
                    .collect();
                let (sub, titles) = texts.split_last().expect("a title and a subtitle");
                assert!(!titles.is_empty(), "{locale:?} at {w}: no title painted");
                for (line, at) in titles {
                    let line_w = backend.measure_text_family_styled(
                        line,
                        IMPORT_TITLE_FONT,
                        SANS,
                        TITLE_WEIGHT,
                        false,
                    );
                    assert!(
                        line_w <= text_box.size.x,
                        "{locale:?} at {w}: {line:?} is {line_w} px in a {} px box",
                        text_box.size.x
                    );
                    assert!(
                        at.y - IMPORT_TITLE_FONT >= card.origin.y,
                        "{line:?} above the card"
                    );
                }
                let sub_w = backend.measure_text_family(&sub.0, SUB_FONT, SANS);
                assert!(sub_w <= text_box.size.x, "{locale:?} at {w}: {:?}", sub.0);
                assert!(
                    sub.1.y + 4.0 <= card.origin.y + card.size.y,
                    "{:?} below",
                    sub.0
                );
            }
        }
    }
}

#[test]
fn the_turkish_screenshot_title_wraps_onto_a_taller_row() {
    let state = home_state(Locale::Tr);
    let home = HomeSurface::for_editor(&state).unwrap();
    let layout = home.layout(1440.0, 900.0);
    assert!(layout.imports.iter().all(|c| c.size.y == IMPORT_H_TALL));
    let mut backend = CaptureBackend::default();
    home.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        Rect::xywh(0.0, 0.0, 1440.0, 900.0),
    );
    let x = import_text_box(layout.imports[2]).origin.x;
    let lines: Vec<&str> = backend
        .texts
        .iter()
        .filter(|(_, at)| at.x == x)
        .map(|(text, _)| text.as_str())
        .collect();
    assert_eq!(
        lines.len(),
        3,
        "two title lines and the subtitle: {lines:?}"
    );
    assert!(lines.iter().all(|line| !line.ends_with('…')), "{lines:?}");
}

#[test]
fn english_keeps_the_compact_row_at_full_width() {
    let state = home_state(Locale::EnUs);
    let layout = HomeSurface::for_editor(&state)
        .unwrap()
        .layout(1440.0, 900.0);
    assert!(layout.imports.iter().all(|c| c.size.y == IMPORT_H));
}
