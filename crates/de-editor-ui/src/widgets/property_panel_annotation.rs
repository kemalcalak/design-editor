//! Annotation section (#12): the kind (Note / Context / Prompt), the
//! content and, for a prompt, the model. Visible when the selection is a
//! note / context / prompt node (`VisibleSections.annotation`). Paint and
//! both layout walkers step through the same rows, like the Widget section.

use jian_ops_schema::node::{AnnotationKind, PenNode};

use crate::theme::Theme;
use crate::widgets::property_panel::{NodeSnapshot, PropertyPanelAction};
use crate::widgets::property_panel_inputs::{
    paint_input_with_prefix_focused_state, paint_section_divider, paint_section_label,
    INPUT_HEIGHT, PAD_X, SECTION_GAP, SECTION_HEADER_HEIGHT,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::property_panel_segments::{paint_segment_row, segment_rects};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::PropertyFocus;

/// One row's vertical advance (input + gap).
const ROW_ADVANCE: f32 = INPUT_HEIGHT + 6.0;
const SEGMENT_FONT: f32 = 12.0;

/// The selected annotation's editable fields.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationSummary {
    pub kind: AnnotationKind,
    pub content: String,
    pub model: String,
}

/// `Some` for a note / context / prompt node.
pub fn annotation_summary_of(node: &PenNode) -> Option<AnnotationSummary> {
    let (kind, body) = node.annotation()?;
    Some(AnnotationSummary {
        kind,
        content: body.plain_content(),
        model: body.model.clone().unwrap_or_default(),
    })
}

/// The text rows `kind` shows, in order.
fn text_rows(kind: AnnotationKind) -> &'static [PropertyFocus] {
    match kind {
        AnnotationKind::Prompt => &[
            PropertyFocus::AnnotationContent,
            PropertyFocus::AnnotationModel,
        ],
        _ => &[PropertyFocus::AnnotationContent],
    }
}

/// Section height without the trailing gap (callers add `SECTION_GAP`).
pub fn annotation_section_height(kind: AnnotationKind) -> f32 {
    SECTION_HEADER_HEIGHT + ROW_ADVANCE * (1 + text_rows(kind).len()) as f32 + 12.0
}

fn segment_row_rect(x0: f32, top: f32, usable_w: f32) -> Rect {
    Rect {
        origin: Point2D::new(x0 + PAD_X, top + SECTION_HEADER_HEIGHT),
        size: Point2D::new(usable_w, INPUT_HEIGHT),
    }
}

/// The kind cells, one action each.
pub fn push_annotation_action_rects(
    out: &mut Vec<(PropertyPanelAction, Rect)>,
    x0: f32,
    top: f32,
    usable_w: f32,
) {
    let cells = segment_rects(
        segment_row_rect(x0, top, usable_w),
        AnnotationKind::ALL.len(),
    );
    for (kind, rect) in AnnotationKind::ALL.into_iter().zip(cells) {
        out.push((PropertyPanelAction::SetAnnotationKind(kind), rect));
    }
}

/// The content (and model) input rects.
pub fn push_annotation_input_rects(
    rects: &mut Vec<(PropertyFocus, Rect)>,
    kind: AnnotationKind,
    x0: f32,
    top: f32,
    usable_w: f32,
) {
    let mut y = top + SECTION_HEADER_HEIGHT + ROW_ADVANCE;
    for focus in text_rows(kind) {
        rects.push((
            *focus,
            Rect {
                origin: Point2D::new(x0 + PAD_X, y),
                size: Point2D::new(usable_w, INPUT_HEIGHT),
            },
        ));
        y += ROW_ADVANCE;
    }
}

/// Paint the section. Returns the y below it, gap included.
#[allow(clippy::too_many_arguments)]
pub fn paint_annotation_section(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    snapshot: &NodeSnapshot,
    edit: &EditContext<'_>,
    locale: de_editor_core::Locale,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let Some(summary) = snapshot.annotation.as_ref() else {
        return y;
    };
    let usable_w = width - PAD_X * 2.0;
    let top = y;
    paint_section_label(
        cx,
        theme,
        label(locale, kind_key(summary.kind)),
        x,
        y,
        width,
    );
    let labels: Vec<&str> = AnnotationKind::ALL
        .iter()
        .map(|kind| label(locale, kind_key(*kind)))
        .collect();
    let active = AnnotationKind::ALL
        .iter()
        .position(|kind| *kind == summary.kind);
    paint_segment_row(
        cx,
        theme,
        segment_row_rect(x, top, usable_w),
        &labels,
        active,
        SEGMENT_FONT,
    );
    let mut y = top + SECTION_HEADER_HEIGHT + ROW_ADVANCE;
    for focus in text_rows(summary.kind) {
        let (prefix, fallback) = match focus {
            PropertyFocus::AnnotationModel => {
                (label(locale, "annotation.model"), summary.model.as_str())
            }
            _ => (
                label(locale, "annotation.content"),
                summary.content.as_str(),
            ),
        };
        let rect = Rect {
            origin: Point2D::new(x + PAD_X, y),
            size: Point2D::new(usable_w, INPUT_HEIGHT),
        };
        paint_input_with_prefix_focused_state(
            cx,
            theme,
            rect,
            prefix,
            edit.value_for(*focus, fallback),
            edit.focus == Some(*focus),
            edit.caret_at(*focus),
            edit.select_all_at(*focus),
            edit.input_at(*focus),
            edit.now_ms,
        );
        y += ROW_ADVANCE;
    }
    y += 12.0;
    paint_section_divider(cx, theme, x, y, width);
    y + SECTION_GAP
}

fn kind_key(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Note => "annotation.note",
        AnnotationKind::Context => "annotation.context",
        AnnotationKind::Prompt => "annotation.prompt",
    }
}

fn label(locale: de_editor_core::Locale, key: &'static str) -> &'static str {
    crate::i18n::translate(locale, key)
}

#[cfg(test)]
#[path = "property_panel_annotation_tests.rs"]
mod tests;
