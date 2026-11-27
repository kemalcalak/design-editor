use super::*;
use crate::RenderBackend;

#[derive(Default)]
struct FontMetricCapture {
    runs: Vec<(String, Point2D)>,
}

impl RenderBackend for FontMetricCapture {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &TextLayout, origin: Point2D) {
        let text = layout.runs().first().unwrap().content.clone();
        self.runs.push((text, origin));
    }
    fn clip_rect(&mut self, _: Rect) {}
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

    // Deliberately unlike the monospace metric. The regression test must fail
    // if code-preview positioning falls back to the default UI font again.
    fn measure_text(&mut self, text: &str, _: f32) -> f32 {
        text.chars().count() as f32
    }

    fn measure_text_family_styled(
        &mut self,
        text: &str,
        _: f32,
        family: &str,
        weight: u16,
        italic: bool,
    ) -> f32 {
        assert_eq!(family, CODE_FONT_FAMILY);
        assert_eq!(weight, 400);
        assert!(!italic);
        text.chars().count() as f32 * 8.0
    }
}

#[test]
fn highlighted_token_origins_follow_the_painted_monospace_advance() {
    let mut backend = FontMetricCapture::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint_highlighted_code_line(
        &mut cx,
        &Theme::light(),
        "<span style={{position:'absolute',left:0,width:380}}>",
        100.0,
        20.0,
    );

    assert!(
        backend.runs.len() > 5,
        "fixture should produce multiple tokens"
    );
    let mut expected_x = 100.0;
    for (text, origin) in backend.runs {
        assert_eq!(
            origin.x, expected_x,
            "token {text:?} overlaps its predecessor"
        );
        expected_x += text.chars().count() as f32 * 8.0;
    }
}

#[test]
fn host_handoff_notice_reserves_one_row_above_the_source() {
    let mut state = CodegenState::default();
    assert_eq!(code_area_y(&state, 20.0, 24.0), 20.0);
    state.host_notice = Some("Queued for Codex".into());
    assert_eq!(code_area_y(&state, 20.0, 24.0), 44.0);
    state.degraded = true;
    assert_eq!(code_area_y(&state, 20.0, 24.0), 68.0);
}

/// The action chips are sized from their labels as painted (#451): in every
/// locale each label sits whole inside the chip the backend-less hit-test
/// resolves when the row has room, and ellipsized inside it when the row
/// falls back to equal chips.
#[test]
fn action_chips_hold_their_labels_in_every_locale() {
    use crate::widgets::chrome_family_paint_tests::assert_drawn_as_measured;
    use crate::widgets::label_fit_test_support::assert_inside;
    use de_i18n::Locale;

    let state = CodegenState::default();
    for locale in Locale::ALL {
        let strings = CodePanelStrings::new(locale);
        let labels = [
            strings.copy(),
            strings.save(),
            strings.bundle(),
            strings.regen(),
        ];
        for w in [272.0, 1400.0] {
            let layout = CompleteLayout {
                x: 0.0,
                y: 0.0,
                w,
                progress_row_h: 24.0,
                panel_bottom: None,
            };
            let what = format!("code actions {locale:?} at {w}");
            let backend = assert_drawn_as_measured(&what, &[CODE_FONT_FAMILY], |cx| {
                paint_complete_body_in_panel(cx, &Theme::dark(), &state, strings, layout, None);
                Rect {
                    origin: Point2D::new(0.0, 0.0),
                    size: Point2D::new(w, 2000.0),
                }
            });
            let row_y = complete_action_row_y_in_panel(&state, layout);
            let rects = action_chip_rects(layout.x, row_y, w, strings);
            for (label, chip) in labels.iter().zip(rects) {
                let label_box = Rect {
                    origin: Point2D::new(chip.origin.x + ACTION_CHIP_LEAD, chip.origin.y),
                    size: Point2D::new(
                        (chip.size.x - ACTION_CHIP_LEAD - ACTION_CHIP_PAD_X).max(0.0),
                        chip.size.y,
                    ),
                };
                let run = backend.runs.iter().find(|run| {
                    (run.text == *label || run.text.ends_with('…'))
                        && (run.origin.x - label_box.origin.x).abs() < 0.01
                });
                if w > 1000.0 {
                    let run = run.unwrap_or_else(|| panic!("{what}: {label:?} not painted"));
                    assert_eq!(
                        run.text, *label,
                        "{what}: a wide row fits every label whole"
                    );
                }
                // A chip too narrow for any of its label shows its icon alone.
                if let Some(run) = run {
                    assert_inside(run, label_box, 0.0, &what);
                }
            }
        }
    }
}
