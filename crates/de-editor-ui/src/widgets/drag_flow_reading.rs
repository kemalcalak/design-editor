//! Drop index for containers laid out in reading order (#379): a wrapping
//! row or a grid. Siblings are ordered left to right, then top to bottom,
//! so the dragged box lands before the first sibling it reads ahead of —
//! one on a later row, or one to its right on the same row.

use crate::Rect;
use de_editor_core::editor_ui_state::CanvasOverlayLine;

/// Insert index among `siblings` (document order, the dragged node left
/// out) for a box dragged to `dragged`, plus a vertical insertion bar.
pub(super) fn reading_order_insert(
    siblings: &[Rect],
    dragged: Rect,
) -> (usize, Option<CanvasOverlayLine>) {
    let x = dragged.origin.x + dragged.size.x / 2.0;
    let y = dragged.origin.y + dragged.size.y / 2.0;
    let index = siblings
        .iter()
        .position(|s| {
            let bottom = s.origin.y + s.size.y;
            if y < s.origin.y {
                return true;
            }
            y <= bottom && x < s.origin.x + s.size.x / 2.0
        })
        .unwrap_or(siblings.len());
    (index, insertion_bar(siblings, index))
}

/// A bar at the left edge of the sibling the drop goes before, or at the
/// right edge of the last one.
fn insertion_bar(siblings: &[Rect], index: usize) -> Option<CanvasOverlayLine> {
    let (x, anchor) = match siblings.get(index) {
        Some(next) => (next.origin.x - 2.0, *next),
        None => {
            let last = *siblings.last()?;
            (last.origin.x + last.size.x + 2.0, last)
        }
    };
    Some(CanvasOverlayLine::new(
        x as f64,
        anchor.origin.y as f64,
        x as f64,
        (anchor.origin.y + anchor.size.y) as f64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells() -> Vec<Rect> {
        // Two rows of two 100×50 cells, 10 px apart.
        vec![
            Rect::xywh(0.0, 0.0, 100.0, 50.0),
            Rect::xywh(110.0, 0.0, 100.0, 50.0),
            Rect::xywh(0.0, 60.0, 100.0, 50.0),
            Rect::xywh(110.0, 60.0, 100.0, 50.0),
        ]
    }

    #[test]
    fn drop_reads_left_to_right_then_down() {
        let at = |x: f32, y: f32| reading_order_insert(&cells(), Rect::xywh(x, y, 20.0, 20.0)).0;
        assert_eq!(at(-20.0, 10.0), 0);
        assert_eq!(at(60.0, 10.0), 1);
        assert_eq!(at(200.0, 10.0), 2);
        assert_eq!(at(60.0, 70.0), 3);
        assert_eq!(at(200.0, 70.0), 4);
    }

    #[test]
    fn bar_sits_before_the_next_cell_or_after_the_last() {
        let (_, bar) = reading_order_insert(&cells(), Rect::xywh(60.0, 70.0, 20.0, 20.0));
        let bar = bar.unwrap();
        assert_eq!((bar.x1, bar.y1, bar.y2), (108.0, 60.0, 110.0));
        let (_, end) = reading_order_insert(&cells(), Rect::xywh(300.0, 70.0, 20.0, 20.0));
        assert_eq!(end.unwrap().x1, 212.0);
        assert!(reading_order_insert(&[], Rect::xywh(0.0, 0.0, 1.0, 1.0))
            .1
            .is_none());
    }
}
