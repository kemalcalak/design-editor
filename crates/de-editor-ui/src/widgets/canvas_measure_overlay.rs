//! Hover measurements on the canvas (#399): with a selection, hold Alt /
//! Option and hover another layer to see red distance lines and their
//! values, Figma style. The geometry is `de_editor_core::measure`; this
//! module picks the two boxes off the laid-out scene and paints.
//!
//! Cost discipline: [`MeasureOverlay::build`] returns at its first check
//! while Alt is up, so the feature adds nothing when off. While on, the
//! formatted labels live in a one-entry thread-local memo keyed by the two
//! boxes, so a steady hover reuses the same `Rc` frame after frame and
//! paint allocates nothing.

use std::cell::RefCell;
use std::rc::Rc;

use crate::layout_scene::LayoutScene;
use crate::widgets::canvas_viewport::paint_dashed_line;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};
use de_editor_core::measure::{format_measure_value, measure, MeasureGuide, MeasureLine};
use de_editor_core::Viewport as DocViewport;
use de_editor_core::{DocRect, EditorState, Tool};

/// Figma's measurement red (#F24822).
const RED: Color = Color {
    r: 0.949,
    g: 0.282,
    b: 0.133,
    a: 1.0,
};
const LABEL_FONT: f32 = 11.0;
const LABEL_PAD_X: f32 = 4.0;
const LABEL_H: f32 = 16.0;

/// The two boxes a measurement was built from, as raw bits.
type MeasureKey = [u32; 8];

/// One labelled line with its laid-out value text.
struct LabelledLine {
    line: MeasureLine,
    label: TextLayout,
    label_text: String,
}

/// Distances between the selection and the hovered layer, ready to paint.
pub struct MeasureOverlay {
    key: MeasureKey,
    lines: Vec<LabelledLine>,
    guides: Vec<MeasureGuide>,
}

thread_local! {
    static MEMO: RefCell<Option<Rc<MeasureOverlay>>> = const { RefCell::new(None) };
}

impl MeasureOverlay {
    /// The overlay for this frame, or `None` when Alt is up, the Select
    /// tool is not active, nothing is selected, nothing is hovered, or the
    /// hovered layer is part of the selection.
    pub fn build(state: &EditorState, scene: &LayoutScene) -> Option<Rc<Self>> {
        if !state.editor_ui.canvas_view.measure_modifier {
            return None;
        }
        if state.tool != Tool::Select || state.selection.set.is_empty() {
            return None;
        }
        let hovered = state.editor_ui.canvas_hover_node.as_ref()?;
        if state.selection.contains(hovered) {
            return None;
        }
        let page = scene.active_page()?;
        let target = page.find(hovered.as_str())?.bounds;
        let mut sel: Option<Rect> = None;
        for id in state.selection.set.iter().filter(|id| id.is_real()) {
            let Some(node) = page.find(id.as_str()) else {
                continue;
            };
            sel = Some(match sel {
                Some(acc) => union(acc, node.bounds),
                None => node.bounds,
            });
        }
        let sel = sel?;
        let key = key_of(sel, target);
        MEMO.with(|memo| {
            let mut memo = memo.borrow_mut();
            if let Some(hit) = memo.as_ref().filter(|m| m.key == key) {
                return Some(Rc::clone(hit));
            }
            let built = Rc::new(Self::from_boxes(key, sel, target));
            *memo = Some(Rc::clone(&built));
            Some(built)
        })
    }

    fn from_boxes(key: MeasureKey, sel: Rect, target: Rect) -> Self {
        let m = measure(doc_rect(sel), doc_rect(target));
        let lines = m
            .lines
            .into_iter()
            .map(|line| {
                let label_text = format_measure_value(line.value);
                let label = TextLayout::single_run(
                    &label_text,
                    "system-ui",
                    LABEL_FONT,
                    Color::WHITE.to_jian(),
                    Point2D::new(0.0, 0.0),
                );
                LabelledLine {
                    line,
                    label,
                    label_text,
                }
            })
            .collect();
        Self {
            key,
            lines,
            guides: m.guides,
        }
    }

    /// The value labels, in line order (tests and accessibility).
    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|l| l.label_text.as_str())
    }

    /// Paint the lines, dashed guides and value pills in screen space.
    pub fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect, viewport: &DocViewport) {
        let to_screen = |(x, y): (f64, f64)| {
            Point2D::new(
                rect.origin.x + viewport.pan_x + x as f32 * viewport.zoom,
                rect.origin.y + viewport.pan_y + y as f32 * viewport.zoom,
            )
        };
        for guide in &self.guides {
            paint_dashed_line(cx, to_screen(guide.from), to_screen(guide.to), RED, 1.0);
        }
        for l in &self.lines {
            cx.backend
                .stroke_line(to_screen(l.line.from), to_screen(l.line.to), RED, 1.0);
        }
        for l in &self.lines {
            let a = to_screen(l.line.from);
            let b = to_screen(l.line.to);
            let w = cx.backend.measure_text(&l.label_text, LABEL_FONT) + LABEL_PAD_X * 2.0;
            let (cx_mid, cy_mid) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            // Horizontal distances sit under their line, vertical ones
            // beside it, so the pill never hides the line it labels.
            let pill = if l.line.vertical() {
                Rect::xywh(cx_mid + 4.0, cy_mid - LABEL_H / 2.0, w, LABEL_H)
            } else {
                Rect::xywh(cx_mid - w / 2.0, cy_mid + 4.0, w, LABEL_H)
            };
            cx.backend.fill_round_rect(pill, 3.0, RED);
            let baseline = jian_widgets::centered_text_baseline_y(pill, LABEL_FONT);
            cx.backend.draw_text(
                &l.label,
                Point2D::new(pill.origin.x + LABEL_PAD_X, baseline),
            );
        }
    }
}

fn doc_rect(r: Rect) -> DocRect {
    DocRect {
        x: f64::from(r.origin.x),
        y: f64::from(r.origin.y),
        w: f64::from(r.size.x),
        h: f64::from(r.size.y),
    }
}

fn key_of(a: Rect, b: Rect) -> MeasureKey {
    [
        a.origin.x.to_bits(),
        a.origin.y.to_bits(),
        a.size.x.to_bits(),
        a.size.y.to_bits(),
        b.origin.x.to_bits(),
        b.origin.y.to_bits(),
        b.size.x.to_bits(),
        b.size.y.to_bits(),
    ]
}

fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = (a.origin.x + a.size.x).max(b.origin.x + b.size.x);
    let y1 = (a.origin.y + a.size.y).max(b.origin.y + b.size.y);
    Rect::xywh(x0, y0, x1 - x0, y1 - y0)
}

#[cfg(test)]
#[path = "canvas_measure_overlay_tests.rs"]
mod tests;
