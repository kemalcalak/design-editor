use super::*;
use crate::theme::Theme;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use de_editor_core::missing_fonts::{MissingFontEntry, MissingFontsPrompt};
use de_editor_core::EditorState;
use std::sync::Arc;

#[derive(Default)]
struct CaptureBackend {
    text: Vec<(String, f32)>,
    clips: Vec<Rect>,
}

impl RenderBackend for CaptureBackend {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &TextLayout, _: Point2D) {
        self.text.extend(
            layout
                .runs()
                .iter()
                .map(|run| (run.content.clone(), run.font_size)),
        );
    }
    fn clip_rect(&mut self, rect: Rect) {
        self.clips.push(rect);
    }
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, _: Point2D, _: f32, _: Color, _: f32) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _: Point2D) {}
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
}

fn content_rect(width: f32) -> Rect {
    Rect::xywh(16.0, 20.0, width - 32.0, 900.0)
}

fn state_with_missing(families: &[&str]) -> EditorState {
    let mut state = EditorState::new();
    state.editor_ui.missing_fonts_prompt = Some(MissingFontsPrompt {
        entries: families
            .iter()
            .map(|family| MissingFontEntry {
                family: (*family).to_owned(),
                run_count: 2,
                mismatch_note: None,
                resolved: false,
            })
            .collect(),
    });
    state
}

fn painted_text(state: &EditorState, width: f32) -> Vec<(String, f32)> {
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_fonts_tab(
        &mut cx,
        &Theme::dark(),
        &state.editor_ui,
        content_rect(width),
    );
    backend.text
}

#[test]
fn tab_renders_missing_rows_and_empty_copy() {
    let state = state_with_missing(&["Katibeh", "Inter Tight"]);
    let text = painted_text(&state, 492.0);
    assert!(text.iter().any(|(value, _)| value == "Katibeh"));
    assert!(text.iter().any(|(value, _)| value == "Inter Tight"));

    let empty = EditorState::new();
    let text = painted_text(&empty, 492.0);
    assert!(text
        .iter()
        .any(|(value, _)| { value == translate(&empty.editor_ui, "missingFonts.noneMissing") }));
}

#[test]
fn desktop_font_geometry_keeps_legacy_density() {
    let mut state = state_with_missing(&["Katibeh"]);
    state.editor_ui.imported_font_families = Arc::new(vec!["Imported".into()]);
    let content = content_rect(492.0);
    let missing = missing_row_rect(content, 0);
    assert_eq!(missing.size.y, 72.0);
    assert_eq!(
        settings_row_button_rect(missing, &state.editor_ui).size.y,
        28.0
    );
    assert_eq!(imported_row_rect(content, &state.editor_ui, 0).size.y, 44.0);
    assert_eq!(
        imported_remove_rect(content, &state.editor_ui, 0).size.y,
        28.0
    );
    assert_eq!(content_height(&state.editor_ui), 252.0);
}

#[test]
fn downloaded_families_get_their_own_section_and_remove_button() {
    let mut state = state_with_missing(&["Katibeh"]);
    state.editor_ui.imported_font_families =
        Arc::new(vec!["Montserrat".into(), "My Import".into()]);
    state.editor_ui.downloaded_font_families = Arc::new(vec!["Montserrat".into()]);
    let ui = &state.editor_ui;

    assert_eq!(user_imported_families(ui), vec!["My Import"]);
    let text = painted_text(&state, 492.0);
    assert!(text
        .iter()
        .any(|(value, _)| value == translate(ui, "missingFonts.downloadedSection")));

    let content = content_rect(492.0);
    let panel = Rect::xywh(0.0, 0.0, 492.0, 1000.0);
    let centre = |rect: Rect| {
        Point2D::new(
            rect.origin.x + rect.size.x / 2.0,
            rect.origin.y + rect.size.y / 2.0,
        )
    };
    assert_eq!(
        hit_test(
            panel,
            content,
            ui,
            centre(downloaded_remove_rect(content, ui, 0)),
            0.0
        ),
        FontsHit::RemoveDownloadedFont(0)
    );
    assert_eq!(
        hit_test(
            panel,
            content,
            ui,
            centre(imported_remove_rect(content, ui, 0)),
            0.0
        ),
        FontsHit::RemoveImportedFont(0)
    );
}

#[test]
fn settings_rows_offer_download_when_the_host_can_download() {
    let mut state = state_with_missing(&["Montserrat"]);
    let content = content_rect(492.0);
    let panel = Rect::xywh(0.0, 0.0, 492.0, 1000.0);
    let row = missing_row_rect(content, 0);
    let hits = |ui: &EditorUiState| {
        (0..row.size.x as i32)
            .map(|dx| Point2D::new(row.origin.x + dx as f32, row.origin.y + 36.0))
            .any(|point| hit_test(panel, content, ui, point, 0.0) == FontsHit::Download(0))
    };
    assert!(!hits(&state.editor_ui));
    state.editor_ui.font_download_supported = true;
    assert!(hits(&state.editor_ui));
}
