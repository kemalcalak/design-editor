use super::fuzzy::fuzzy_score;
use super::*;
use crate::find_layers::tests::multi_page_state;
use de_i18n::Locale;

fn type_text(state: &mut EditorState, text: &str) {
    for c in text.chars() {
        command_palette_text(state, c, 0);
    }
}

fn labels(state: &EditorState) -> Vec<String> {
    state
        .editor_ui
        .command_palette
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.label.to_string())
        .collect()
}

#[test]
fn fuzzy_ranks_exact_prefix_word_substring_subsequence() {
    let exact = fuzzy_score("save", "Save").unwrap();
    let prefix = fuzzy_score("save", "Save As…").unwrap();
    let word = fuzzy_score("as", "Save As…").unwrap();
    let substring = fuzzy_score("ave", "Save").unwrap();
    let subsequence = fuzzy_score("dpl", "Duplicate").unwrap();
    assert!(exact > prefix && prefix > word && word > substring && substring > subsequence);
    assert!(fuzzy_score("zzz", "Duplicate").is_none());
    assert_eq!(fuzzy_score("", "anything"), Some(0));
    // A shorter label wins within a tier.
    assert!(fuzzy_score("gr", "Group").unwrap() > fuzzy_score("gr", "Group Selection").unwrap());
}

#[test]
fn every_command_label_is_translated() {
    for command in PaletteCommand::ALL {
        let key = command.spec().label_key;
        for locale in Locale::ALL {
            assert_ne!(
                de_i18n::translate(locale, key),
                key,
                "{key} missing in {locale:?}"
            );
        }
    }
}

#[test]
fn empty_query_lists_every_command_with_its_chord() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::Commands, 0);
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert_eq!(palette.rows.len(), PaletteCommand::ALL.len());
    let undo = palette
        .rows
        .iter()
        .find(|row| row.item == PaletteItem::Command(PaletteCommand::Undo))
        .unwrap();
    assert_eq!(undo.trailing, PaletteCommand::Undo.chord());
    assert!(!undo.trailing.is_empty());
}

#[test]
fn query_ranks_commands_pages_and_components() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::Commands, 0);
    type_text(&mut state, "dupl");
    assert_eq!(labels(&state)[0], "Duplicate");
    command_palette_select_all(&mut state);
    type_text(&mut state, "settings");
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert_eq!(palette.rows[0].item, PaletteItem::Page(1), "page by name");
    command_palette_select_all(&mut state);
    type_text(&mut state, "button");
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert!(palette
        .rows
        .iter()
        .any(|row| row.item == PaletteItem::Component(NodeId::new("btn-master"))));
    // English keywords find a command in any locale.
    state.editor_ui.locale = Locale::Tr;
    command_palette_select_all(&mut state);
    type_text(&mut state, "redo");
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert_eq!(
        palette.rows[0].item,
        PaletteItem::Command(PaletteCommand::Redo)
    );
}

#[test]
fn arrows_wrap_and_enter_runs_the_highlighted_command() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::Commands, 0);
    assert_eq!(command_palette_move(&mut state, -1), Some(true));
    let last = PaletteCommand::ALL.len() - 1;
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert_eq!(palette.highlighted, last);
    assert!(palette.scroll.offset > 0.0, "the highlight stays in view");
    command_palette_move(&mut state, 1);
    assert_eq!(
        state
            .editor_ui
            .command_palette
            .as_ref()
            .unwrap()
            .highlighted,
        0
    );
    type_text(&mut state, "group");
    let outcome = command_palette_activate(&mut state, false);
    assert_eq!(
        outcome,
        Some(PaletteOutcome::Command(PaletteCommand::Group))
    );
    assert!(!state.command_palette_open());
    assert_eq!(command_palette_activate(&mut state, false), None);
}

#[test]
fn same_chord_closes_other_chord_switches_mode() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::Commands, 0);
    type_text(&mut state, "abc");
    state.toggle_command_palette(PaletteMode::FindLayers, 0);
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert_eq!(palette.mode, PaletteMode::FindLayers);
    assert_eq!(palette.input.text(), "");
    state.toggle_command_palette(PaletteMode::FindLayers, 0);
    assert!(!state.command_palette_open());
}

#[test]
fn find_enter_selects_a_match_on_its_page() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::FindLayers, 0);
    type_text(&mut state, "again");
    let outcome = command_palette_activate(&mut state, false);
    assert_eq!(
        outcome,
        Some(PaletteOutcome::Frame(vec![NodeId::new("signin2")]))
    );
    assert_eq!(state.ui.active_page_index, 1);
    assert_eq!(state.selection.anchor.as_str(), "signin2");
}

#[test]
fn find_shift_enter_selects_all_matches_on_the_highlighted_page() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::FindLayers, 0);
    type_text(&mut state, "type:text");
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert!(palette.summary.contains('3'), "{}", palette.summary);
    assert!(
        palette.summary.contains('2'),
        "on 2 pages: {}",
        palette.summary
    );
    // Highlight the first match on page 2.
    command_palette_move(&mut state, 1);
    let outcome = command_palette_activate(&mut state, true).unwrap();
    let PaletteOutcome::Frame(ids) = outcome else {
        panic!("expected a frame, got {outcome:?}");
    };
    assert_eq!(ids.len(), 2);
    assert_eq!(state.ui.active_page_index, 1);
    assert_eq!(state.selection.set.len(), 2);
}

#[test]
fn find_replace_field_rewrites_text_and_stays_open() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::FindLayers, 0);
    type_text(&mut state, "sign in");
    assert_eq!(command_palette_tab(&mut state, 0), Some(true));
    type_text(&mut state, "Log in");
    let outcome = command_palette_activate(&mut state, false);
    assert_eq!(outcome, Some(PaletteOutcome::Replaced(2)));
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert_eq!(palette.input.text(), "sign in", "the query is untouched");
    assert!(palette.summary.contains('2'));
    assert!(state.undo(), "one undo step");
}

#[test]
fn find_layers_command_switches_mode_in_place() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::Commands, 0);
    type_text(&mut state, "find layers");
    assert_eq!(
        command_palette_activate(&mut state, false),
        Some(PaletteOutcome::Nothing)
    );
    let palette = state.editor_ui.command_palette.as_ref().unwrap();
    assert_eq!(palette.mode, PaletteMode::FindLayers);
}

#[test]
fn the_palette_input_owns_the_caret() {
    let mut state = multi_page_state();
    state.toggle_command_palette(PaletteMode::FindLayers, 0);
    type_text(&mut state, "x");
    assert_eq!(
        state.active_text_input().map(|input| input.text()),
        Some("x")
    );
    command_palette_tab(&mut state, 0);
    type_text(&mut state, "y");
    assert_eq!(
        state.active_text_input().map(|input| input.text()),
        Some("y")
    );
    assert_eq!(command_palette_escape(&mut state), Some(true));
    assert!(state.active_text_input().is_none() || !state.command_palette_open());
}
