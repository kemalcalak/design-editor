//! Layout + `from_editor` snapshot tests for the AI chat panel.

#[allow(unused_imports)]
use super::super::tests_paint::{assert_close, color_close, rect_close};
use super::super::*;

#[test]
fn layout_reports_fixed_size() {
    let s = EditorState::new();
    let p = AIChatPlaceholder::from_editor(&s);
    let cx = LayoutCx {
        available_width: 9999.0,
        dpi: 1.0,
    };
    let lb = p.layout(&cx);
    assert_eq!(lb.rect.size.x, AI_CHAT_WIDTH);
    assert_eq!(lb.rect.size.y, AI_CHAT_HEIGHT);
}

#[test]
fn examples_grid_has_four_cards() {
    assert_eq!(example_cards(de_editor_core::Locale::EnUs).len(), 4);
}

#[test]
fn prompt_rows_fill_the_input_with_their_own_words() {
    let en = example_cards(de_editor_core::Locale::EnUs);
    assert_eq!(en[0].title, "Pricing page for a calm note-taking app");
    for card in &en {
        assert_eq!(card.title, card.prompt);
    }
    let tr = example_cards(de_editor_core::Locale::Tr);
    assert_ne!(tr[0].title, en[0].title, "prompts are localised");
}

#[test]
fn from_editor_tracks_selection_count_for_toolbar() {
    let mut s = EditorState::new();
    s.selection.set = vec![
        de_editor_core::NodeId::new("n1"),
        de_editor_core::NodeId::new("n2"),
    ];
    let panel = AIChatPlaceholder::from_editor(&s);

    assert_eq!(panel.selected_count, 2);
}

#[test]
fn from_editor_uses_the_empty_state_title() {
    let s = EditorState::new();
    let panel = AIChatPlaceholder::from_editor(&s);

    assert_eq!(
        panel.label_start_with_ai,
        de_i18n::translate(s.editor_ui.effective_locale(), "chat.empty.title")
    );
}
