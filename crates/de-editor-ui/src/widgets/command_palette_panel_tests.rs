use de_editor_core::command_palette::{
    command_palette_move, command_palette_text, PaletteField, PaletteHover, PaletteMode,
    PaletteOutcome, ROW_HEIGHT,
};
use de_editor_core::{EditorState, NodeId};
use serde_json::json;

use super::command_palette_flow as flow;
use super::command_palette_panel::{CommandPaletteHit, CommandPalettePanel};
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::PaintCx;
use crate::Point2D;

const VW: f32 = 1200.0;
const VH: f32 = 800.0;

fn state(mode: PaletteMode) -> EditorState {
    let doc = json!({"version": "1.0.0", "pages": [
        {"id": "p1", "name": "Home", "children": [
            {"type": "text", "id": "a", "name": "Title", "content": "Sign in"}
        ]},
        {"id": "p2", "name": "Checkout", "children": [
            {"type": "text", "id": "b", "name": "Pay", "content": "Sign in to pay"}
        ]}
    ]});
    let mut state = EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    );
    state.toggle_command_palette(mode, 0);
    state
}

fn type_text(state: &mut EditorState, text: &str) {
    for c in text.chars() {
        command_palette_text(state, c, 0);
    }
}

fn row_centre(state: &EditorState, index: usize) -> Point2D {
    let panel = CommandPalettePanel::for_editor(state).expect("open");
    let rect = panel.rect(VW, VH);
    let rows = panel.rows_rect(rect);
    Point2D::new(
        rect.origin.x + 40.0,
        rows.origin.y + index as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0,
    )
}

#[test]
fn hit_test_covers_query_rows_replace_and_scrim() {
    let mut state = state(PaletteMode::FindLayers);
    type_text(&mut state, "sign");
    let panel = CommandPalettePanel::for_editor(&state).expect("open");
    let rect = panel.rect(VW, VH);
    let query = panel.query_rect(rect);
    assert_eq!(
        panel.hit_test(
            rect,
            Point2D::new(query.origin.x + 80.0, query.origin.y + 20.0)
        ),
        CommandPaletteHit::Query
    );
    let replace = panel
        .replace_rect(rect)
        .expect("find mode has a replace row");
    assert_eq!(
        panel.hit_test(
            rect,
            Point2D::new(replace.origin.x + 80.0, replace.origin.y + 20.0)
        ),
        CommandPaletteHit::Replace
    );
    let button = panel.replace_all_rect(rect).unwrap();
    assert_eq!(
        panel.hit_test(
            rect,
            Point2D::new(button.origin.x + 4.0, button.origin.y + 4.0)
        ),
        CommandPaletteHit::ReplaceAll
    );
    assert_eq!(
        panel.hit_test(rect, row_centre(&state, 1)),
        CommandPaletteHit::Row(1)
    );
    assert_eq!(
        panel.hit_test(rect, Point2D::new(5.0, VH - 5.0)),
        CommandPaletteHit::Outside
    );
    // Commands mode has no replace row.
    let commands = self::state(PaletteMode::Commands);
    let panel = CommandPalettePanel::for_editor(&commands).unwrap();
    assert!(panel.replace_rect(panel.rect(VW, VH)).is_none());
}

#[test]
fn a_row_click_selects_and_frames_its_match_on_another_page() {
    let mut state = state(PaletteMode::FindLayers);
    type_text(&mut state, "pay");
    let point = row_centre(&state, 0);
    let outcome = flow::press(&mut state, VW, VH, point, 0);
    assert_eq!(outcome, Some(PaletteOutcome::Frame(vec![NodeId::new("b")])));
    assert_eq!(state.ui.active_page_index, 1);
    assert!(!state.command_palette_open());
    assert_eq!(flow::press(&mut state, VW, VH, point, 0), None);
}

#[test]
fn the_scrim_closes_and_fields_take_focus() {
    let mut state = state(PaletteMode::FindLayers);
    let panel = CommandPalettePanel::for_editor(&state).unwrap();
    let replace = panel.replace_rect(panel.rect(VW, VH)).unwrap();
    let point = Point2D::new(replace.origin.x + 60.0, replace.origin.y + 10.0);
    assert_eq!(
        flow::press(&mut state, VW, VH, point, 0),
        Some(PaletteOutcome::Nothing)
    );
    assert_eq!(
        state.editor_ui.command_palette.as_ref().unwrap().field,
        PaletteField::Replace
    );
    assert_eq!(
        flow::press(&mut state, VW, VH, Point2D::new(4.0, VH - 4.0), 0),
        Some(PaletteOutcome::Nothing)
    );
    assert!(!state.command_palette_open());
}

#[test]
fn hover_tints_and_wheel_scrolls_the_rows() {
    let mut state = state(PaletteMode::Commands);
    let point = row_centre(&state, 2);
    assert!(flow::hover(&mut state, VW, VH, point));
    assert_eq!(
        state.editor_ui.command_palette.as_ref().unwrap().hover,
        Some(PaletteHover::Row(2))
    );
    assert!(!flow::hover(&mut state, VW, VH, point), "unchanged");
    assert_eq!(
        flow::scroll(&mut state, point.x, point.y, -60.0, VW, VH),
        Some(true)
    );
    assert!(
        state
            .editor_ui
            .command_palette
            .as_ref()
            .unwrap()
            .scroll
            .offset
            > 0.0
    );
    assert_eq!(flow::scroll(&mut state, 2.0, 2.0, -60.0, VW, VH), None);
}

#[test]
fn paint_shows_chords_highlight_and_summary() {
    let mut state = state(PaletteMode::Commands);
    command_palette_move(&mut state, 1);
    let panel = CommandPalettePanel::for_editor(&state).unwrap();
    let rect = panel.rect(VW, VH);
    let mut backend = CaptureBackend::default();
    panel.paint_at(
        &mut PaintCx {
            backend: &mut backend,
        },
        rect,
        0,
    );
    let texts: Vec<&str> = backend.texts.iter().map(|(t, _)| t.as_str()).collect();
    assert!(texts.contains(&"Undo"), "{texts:?}");
    let undo_chord = de_editor_core::command_palette::PaletteCommand::Undo.chord();
    assert!(texts.contains(&undo_chord));
    assert!(texts.iter().any(|t| t.contains("esc")), "footer hint");
    // The highlighted row (index 1) carries the accent wash.
    assert!(!backend.round_fills.is_empty());

    let mut find = self::state(PaletteMode::FindLayers);
    type_text(&mut find, "sign");
    let panel = CommandPalettePanel::for_editor(&find).unwrap();
    let mut backend = CaptureBackend::default();
    panel.paint_at(
        &mut PaintCx {
            backend: &mut backend,
        },
        panel.rect(VW, VH),
        0,
    );
    let texts: Vec<&str> = backend.texts.iter().map(|(t, _)| t.as_str()).collect();
    assert!(texts.contains(&"2 matches · on 2 pages"), "{texts:?}");
    assert!(texts.contains(&"Checkout"), "a match shows its page");
}
