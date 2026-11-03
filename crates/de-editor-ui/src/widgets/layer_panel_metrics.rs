//! LayerPanel metrics and shared action geometry.
//!
//! Paint, hit testing, scrolling, reveal and drag/drop all consume the same
//! metrics so rows cannot visually drift away from their hit targets.

use crate::{Point2D, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayerPanelMetrics {
    pub section_header_height: f32,
    pub page_row_height: f32,
    pub layer_row_height: f32,
    pub row_pad_x: f32,
    pub section_gap: f32,
    pub header_font: f32,
    pub row_font: f32,
    pub glyph_size: f32,
    pub trailing_glyph_size: f32,
    pub action_target: f32,
    pub pages_max_rows: usize,
}

impl LayerPanelMetrics {
    pub const DESKTOP: Self = Self {
        section_header_height: 28.0,
        page_row_height: 32.0,
        layer_row_height: 28.0,
        row_pad_x: 12.0,
        section_gap: 8.0,
        header_font: 11.0,
        row_font: 13.0,
        glyph_size: 14.0,
        trailing_glyph_size: 12.0,
        action_target: 22.0,
        pages_max_rows: 6,
    };
}

fn square_centered_on(glyph: Rect, side: f32) -> Rect {
    Rect {
        origin: Point2D::new(
            glyph.origin.x - (side - glyph.size.x) / 2.0,
            glyph.origin.y - (side - glyph.size.y) / 2.0,
        ),
        size: Point2D::new(side, side),
    }
}

pub(crate) fn glyph_rect_in(target: Rect, size: f32) -> Rect {
    Rect {
        origin: Point2D::new(
            target.origin.x + (target.size.x - size) / 2.0,
            target.origin.y + (target.size.y - size) / 2.0,
        ),
        size: Point2D::new(size, size),
    }
}

pub(crate) fn add_page_target(rect: Rect, header_y: f32, m: LayerPanelMetrics) -> Rect {
    let glyph = Rect {
        origin: Point2D::new(
            rect.origin.x + rect.size.x - m.row_pad_x - 12.0,
            header_y + (m.section_header_height - m.glyph_size) / 2.0,
        ),
        size: Point2D::new(m.glyph_size, m.glyph_size),
    };
    square_centered_on(glyph, m.action_target)
}

pub(crate) fn delete_page_target(rect: Rect, row_y: f32, m: LayerPanelMetrics) -> Rect {
    let glyph = Rect {
        origin: Point2D::new(
            rect.origin.x + rect.size.x - m.row_pad_x - m.glyph_size,
            row_y + (m.page_row_height - m.glyph_size) / 2.0,
        ),
        size: Point2D::new(m.glyph_size, m.glyph_size),
    };
    square_centered_on(glyph, m.action_target)
}

pub(crate) fn layer_action_targets(row: Rect, m: LayerPanelMetrics) -> (Rect, Rect) {
    let trailing_right = row.origin.x + row.size.x - 8.0;
    let lock_x = trailing_right - 14.0;
    let eye_x = lock_x - 22.0;
    let y = row.origin.y + 7.0;
    let eye_glyph = Rect {
        origin: Point2D::new(eye_x, y),
        size: Point2D::new(m.trailing_glyph_size, m.trailing_glyph_size),
    };
    let lock_glyph = Rect {
        origin: Point2D::new(lock_x, y),
        size: Point2D::new(m.trailing_glyph_size, m.trailing_glyph_size),
    };
    (
        square_centered_on(eye_glyph, m.action_target),
        square_centered_on(lock_glyph, m.action_target),
    )
}

pub(crate) fn collapse_target(
    row: Rect,
    indent: f32,
    horizontal_offset: f32,
    m: LayerPanelMetrics,
) -> Rect {
    let glyph = Rect {
        origin: Point2D::new(
            row.origin.x + indent - horizontal_offset,
            row.origin.y + 6.0,
        ),
        size: Point2D::new(m.glyph_size, m.glyph_size),
    };
    square_centered_on(glyph, m.action_target)
}

pub(crate) fn layer_node_icon_x(row: Rect, indent: f32) -> f32 {
    row.origin.x + indent + LAYER_ICON_OFFSET
}

/// Where a layer row's label starts: after the chevron column and icon.
pub(crate) fn layer_label_x(row: Rect, indent: f32, m: LayerPanelMetrics) -> f32 {
    layer_node_icon_x(row, indent) + m.glyph_size + 6.0
}

/// Indent step of the shallow levels, where the hierarchy is read most.
pub(crate) const INDENT_STEP: f32 = 12.0;
/// Levels that get the full [`INDENT_STEP`] before deeper ones compress.
pub(crate) const FULL_INDENT_LEVELS: usize = 3;
/// Indent step past [`FULL_INDENT_LEVELS`]. An imported web page nests 15+
/// `div`s; a full step per level would leave those rows no name (#306).
pub(crate) const DEEP_INDENT_STEP: f32 = 4.0;
/// Average characters of label budget a row keeps at rest at any depth, as
/// far as the row width allows: 12 of the name plus the ellipsis. Deeper
/// levels stop indenting once this budget is reached.
pub(crate) const MIN_LABEL_CHARS: f32 = 13.0;
/// Chevron column: from the indent to the node icon.
const LAYER_ICON_OFFSET: f32 = 18.0;
/// Node icon plus its gap: from the icon to the label.
pub(crate) const LAYER_LABEL_OFFSET: f32 = 20.0;
/// Gap between the label's budget and the row's trailing edge at rest.
pub(crate) const ROW_TRAILING_PAD: f32 = 8.0;

/// Leading indent of a layer row at `depth` inside a `row_w`-wide row:
/// the row padding plus a depth term that compresses past
/// [`FULL_INDENT_LEVELS`] and is capped so the label keeps
/// [`MIN_LABEL_CHARS`]. Paint, hit testing, the drag ghost and the
/// horizontal content width all read it, so they cannot drift apart.
pub(crate) fn layer_indent(depth: usize, row_w: f32, m: LayerPanelMetrics) -> f32 {
    let full = depth.min(FULL_INDENT_LEVELS) as f32 * INDENT_STEP;
    let deep = depth.saturating_sub(FULL_INDENT_LEVELS) as f32 * DEEP_INDENT_STEP;
    let min_label_w = MIN_LABEL_CHARS * m.row_font * super::layer_panel_paint::TEXT_WIDTH_FACTOR;
    let cap = row_w
        - m.row_pad_x
        - LAYER_ICON_OFFSET
        - LAYER_LABEL_OFFSET
        - ROW_TRAILING_PAD
        - min_label_w;
    // However narrow the card, the first level still steps in.
    let cap = cap.max(INDENT_STEP);
    m.row_pad_x + (full + deep).min(cap)
}
