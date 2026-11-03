//! Row geometry of deep layer trees (#306): an imported web page nests 15+
//! frames, and every row must still show its name.

use super::layer_panel::{LayerPanel, LayerPanelHit, LAYER_PANEL_WIDTH};
use super::layer_panel_metrics::{
    collapse_target, glyph_rect_in, layer_indent, layer_label_x, LayerPanelMetrics,
    DEEP_INDENT_STEP, FULL_INDENT_LEVELS, INDENT_STEP, MIN_LABEL_CHARS,
};
use super::layer_panel_paint::{layer_action_gutter_left, ROW_FONT};
use super::{PaintCx, Widget};
use crate::{Color, Point2D, Rect, RenderBackend, TextLayout};
use de_editor_core::{EditorState, NodeId};

const M: LayerPanelMetrics = LayerPanelMetrics::DESKTOP;
/// Matches the paint heuristic (13 px × 0.56), so "12 characters" means the
/// same width here as in the indent cap.
const CHAR_W: f32 = 7.28;
const DEPTH: usize = 15;
/// Row width inside the default 240 px left card.
const ROW_W: f32 = LAYER_PANEL_WIDTH - 12.0;

#[derive(Default)]
struct Recorder {
    translation: Point2D,
    saved: Vec<Point2D>,
    texts: Vec<(String, Point2D)>,
    strokes: Vec<Point2D>,
}

impl RenderBackend for Recorder {
    fn begin_frame(&mut self) {}
    fn end_frame(&mut self) {}
    fn fill_rect(&mut self, _: Rect, _: Color) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn draw_text(&mut self, layout: &TextLayout, origin: Point2D) {
        for run in layout.runs() {
            self.texts.push((
                run.content.clone(),
                Point2D::new(origin.x + self.translation.x, origin.y),
            ));
        }
    }
    fn clip_rect(&mut self, _: Rect) {}
    fn stroke_line(&mut self, _: Point2D, _: Point2D, _: Color, _: f32) {}
    fn fill_round_rect(&mut self, _: Rect, _: f32, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: f32, _: Color, _: f32) {}
    fn stroke_svg_path(&mut self, _: &str, top_left: Point2D, _: f32, _: Color, _: f32) {
        self.strokes.push(top_left + self.translation);
    }
    fn save(&mut self) {
        self.saved.push(self.translation);
    }
    fn restore(&mut self) {
        self.translation = self.saved.pop().expect("balanced save/restore");
    }
    fn translate(&mut self, offset: Point2D) {
        self.translation += offset;
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn dpi_scale(&self) -> f32 {
        1.0
    }
    fn measure_text_family(&mut self, text: &str, _: f32, _: &str) -> f32 {
        text.chars().count() as f32 * CHAR_W
    }
}

/// `levels` nested frames, each named `div-level-NN-wrapper`.
fn nested_state(levels: usize) -> EditorState {
    let mut src = String::from(r#"{"version":"1.0.0","children":["#);
    for i in 0..levels {
        src.push_str(&format!(
            r#"{{"type":"frame","id":"d{i}","name":"div-level-{i:02}-wrapper","width":100,"height":100,"children":["#
        ));
    }
    src.push_str(&"]}".repeat(levels));
    src.push_str("]}");
    let doc = jian_ops_schema::load_str(&src)
        .expect("fixture parses")
        .value;
    EditorState::from_document(doc)
}

fn rect() -> Rect {
    Rect::xywh(0.0, 0.0, LAYER_PANEL_WIDTH, 700.0)
}

fn paint(panel: &LayerPanel) -> Recorder {
    let mut backend = Recorder::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        rect(),
    );
    backend
}

fn row_of(panel: &LayerPanel, id: &str) -> (usize, Rect) {
    let index = panel
        .items
        .iter()
        .position(|item| item.node_id == NodeId::new(id))
        .expect("row is listed");
    let r = panel.regions(rect());
    (index, panel.layer_row_rect(rect(), &r, index))
}

/// The label painted on `row` (its baseline lies inside the row).
fn label_in_row(backend: &Recorder, row: Rect) -> &(String, Point2D) {
    backend
        .texts
        .iter()
        .find(|(_, origin)| origin.y > row.origin.y && origin.y < row.origin.y + row.size.y)
        .unwrap_or_else(|| panic!("a label paints in {row:?}"))
}

fn visible_chars(label: &str) -> usize {
    label.trim_end_matches('…').chars().count()
}

#[test]
fn indent_is_full_then_compressed_and_never_decreases() {
    for depth in 0..=FULL_INDENT_LEVELS {
        assert_eq!(
            layer_indent(depth, ROW_W, M),
            M.row_pad_x + depth as f32 * INDENT_STEP
        );
    }
    assert_eq!(
        layer_indent(FULL_INDENT_LEVELS + 2, ROW_W, M),
        M.row_pad_x + FULL_INDENT_LEVELS as f32 * INDENT_STEP + 2.0 * DEEP_INDENT_STEP
    );
    let mut last = 0.0;
    for depth in 0..60 {
        let indent = layer_indent(depth, ROW_W, M);
        assert!(indent >= last, "indent shrank at depth {depth}");
        last = indent;
    }
}

#[test]
fn indent_cap_keeps_the_label_budget_at_any_depth() {
    for depth in [DEPTH, 30, 200] {
        let row = Rect::xywh(6.0, 0.0, ROW_W, 24.0);
        let label_x = layer_label_x(row, layer_indent(depth, ROW_W, M), M);
        let budget = row.origin.x + row.size.x - 8.0 - label_x;
        assert!(
            budget + 0.01 >= MIN_LABEL_CHARS * ROW_FONT * 0.56,
            "depth {depth}: {budget} px is under the minimum label budget"
        );
    }
    // A card too narrow for the cap still indents the first level.
    assert_eq!(layer_indent(1, 60.0, M), M.row_pad_x + INDENT_STEP);
}

#[test]
fn every_row_of_a_fifteen_level_tree_shows_twelve_characters() {
    let panel = LayerPanel::from_editor(&nested_state(DEPTH));
    assert_eq!(panel.items.last().map(|item| item.depth), Some(DEPTH - 1));
    let backend = paint(&panel);
    let r = panel.regions(rect());
    for (index, item) in panel.items.iter().enumerate() {
        let (label, _) = label_in_row(&backend, panel.layer_row_rect(rect(), &r, index));
        assert!(item.label.starts_with(label.trim_end_matches('…')));
        assert!(
            visible_chars(label) >= 12,
            "depth {} painted {label:?}",
            item.depth
        );
    }
}

#[test]
fn hovered_deep_row_keeps_its_name_and_its_origin() {
    let deepest = format!("d{}", DEPTH - 1);
    let mut state = nested_state(DEPTH);
    let rest = paint(&LayerPanel::from_editor(&state));
    state.editor_ui.hovered_layer_id = Some(NodeId::new(&deepest));
    let panel = LayerPanel::from_editor(&state);
    let hovered = paint(&panel);

    let (_, row) = row_of(&panel, &deepest);
    let (rest_label, rest_origin) = label_in_row(&rest, row);
    let (hover_label, hover_origin) = label_in_row(&hovered, row);
    assert_eq!(rest_origin, hover_origin, "hover must not move the label");
    assert!(visible_chars(rest_label) >= 12);
    assert!(
        visible_chars(hover_label) >= 5,
        "hover icons left only {hover_label:?}"
    );
    let label_right = hover_origin.x + hover_label.chars().count() as f32 * CHAR_W;
    assert!(
        label_right <= layer_action_gutter_left(row) + 0.01,
        "hovered label runs under the eye/lock gutter"
    );
}

#[test]
fn deep_chevron_paints_where_it_hits() {
    let panel = LayerPanel::from_editor(&nested_state(DEPTH));
    let backend = paint(&panel);
    let id = format!("d{}", DEPTH - 2);
    let (index, row) = row_of(&panel, &id);
    let item = &panel.items[index];
    let indent = layer_indent(item.depth, row.size.x, M);
    let chevron = glyph_rect_in(collapse_target(row, indent, 0.0, M), M.glyph_size);
    assert!(
        backend.strokes.contains(&chevron.origin),
        "chevron paints at {:?}",
        chevron.origin
    );
    let centre = chevron.origin + Point2D::new(M.glyph_size / 2.0, M.glyph_size / 2.0);
    assert_eq!(
        panel.hit_test(rect(), centre),
        Some(LayerPanelHit::ToggleCollapsed(NodeId::new(&id)))
    );
}

#[test]
fn truncated_hovered_name_gets_a_tooltip_anchor() {
    let deepest = format!("d{}", DEPTH - 1);
    let mut state = nested_state(DEPTH);
    state.editor_ui.hovered_layer_id = Some(NodeId::new(&deepest));
    let panel = LayerPanel::from_editor(&state);
    let mut backend = Recorder::default();
    let (anchor, label) = panel
        .truncated_name_anchor(&mut backend, rect())
        .expect("deep hovered name is cut");
    assert_eq!(label, "div-level-14-wrapper");
    let painted = paint(&panel);
    let (_, row) = row_of(&panel, &deepest);
    let (_, origin) = label_in_row(&painted, row);
    assert_eq!(anchor.origin.x, origin.x, "anchor starts at the label");

    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"logo","name":"Logo","width":10,"height":10}]}"#,
    )
    .expect("fixture parses")
    .value;
    let mut state = EditorState::from_document(doc);
    state.editor_ui.hovered_layer_id = Some(NodeId::new("logo"));
    let shallow = LayerPanel::from_editor(&state);
    assert_eq!(
        shallow.truncated_name_anchor(&mut backend, rect()),
        None,
        "a name that fits gets no tooltip"
    );
}
