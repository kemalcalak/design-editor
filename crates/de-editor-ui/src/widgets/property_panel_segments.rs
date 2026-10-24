//! A labelled segmented row (jian `ToggleGroup`) whose cells match the
//! even split the action walkers hit-test. Shared by the stroke
//! alignment row, the shader preset row and the image-fill mode row.

use crate::theme::Theme;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};

/// Family jian's `ToggleGroup` draws its labels in; mirrored so the
/// ellipsis below measures in the face that is painted.
const TOGGLE_GROUP_FAMILY: &str = "Inter";
/// Horizontal breathing room kept inside each cell.
const CELL_INSET: f32 = 6.0;

/// `count` equal cells across `rect`, left to right.
pub(crate) fn segment_rects(rect: Rect, count: usize) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let cell_w = rect.size.x / count as f32;
    (0..count)
        .map(|i| Rect {
            origin: Point2D::new(rect.origin.x + i as f32 * cell_w, rect.origin.y),
            size: Point2D::new(cell_w, rect.size.y),
        })
        .collect()
}

/// Paint the row. `active` is the highlighted cell, `None` for a value
/// no cell represents (a custom shader, say). Labels that would spill
/// into the neighbouring cell are ellipsized, since the component
/// itself does not clip.
pub(crate) fn paint_segment_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    labels: &[&str],
    active: Option<usize>,
    font_size: f32,
) {
    if labels.is_empty() {
        return;
    }
    let max_w = (rect.size.x / labels.len() as f32 - CELL_INSET * 2.0).max(0.0);
    // Only an overflowing label allocates; the common case borrows.
    let fitted: Vec<std::borrow::Cow<'_, str>> = labels
        .iter()
        .map(|label| {
            let width =
                text_metrics::measure_in_family(cx.backend, label, font_size, TOGGLE_GROUP_FAMILY);
            if width <= max_w {
                std::borrow::Cow::Borrowed(*label)
            } else {
                std::borrow::Cow::Owned(text_metrics::fit_in_family(
                    cx.backend,
                    label,
                    max_w,
                    font_size,
                    TOGGLE_GROUP_FAMILY,
                ))
            }
        })
        .collect();
    let options: Vec<&str> = fitted.iter().map(|label| label.as_ref()).collect();
    jian_widgets::components::toggle_group::ToggleGroup {
        options: &options,
        icons: None,
        // Out of range = no cell highlighted.
        active: active.unwrap_or(usize::MAX),
        hover: None,
        font_size,
    }
    .paint(
        cx.backend,
        rect,
        &crate::widgets::button::tokens_from_theme(theme),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_rects_split_the_row_evenly() {
        let rects = segment_rects(Rect::xywh(10.0, 20.0, 90.0, 26.0), 3);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].origin.x, 10.0);
        assert_eq!(rects[1].origin.x, 40.0);
        assert_eq!(rects[2].size.x, 30.0);
        assert!(segment_rects(Rect::xywh(0.0, 0.0, 10.0, 10.0), 0).is_empty());
    }
}
