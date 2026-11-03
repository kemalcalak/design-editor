//! Reading a container's flow from its children: which way they line up,
//! where lines break, and how they sit on the cross axis.

use jian_ops_schema::node::container::AlignItems;

use super::super::{Rect, SnapshotCtx};
use super::{Axis, BoxModel, Item, ItemKind, EPS};

pub(super) struct Cross {
    pub align: Option<AlignItems>,
    /// Per item: does it span the content box's cross axis?
    pub fills: Vec<bool>,
    /// The content decides the container's cross size.
    pub hug: bool,
    /// Every item that does not fill sits at the chosen alignment.
    pub aligned: bool,
}

impl SnapshotCtx<'_> {
    pub(super) fn cross_alignment(
        &self,
        model: &BoxModel,
        axis: Axis,
        content: Rect,
        items: &[Item],
    ) -> Cross {
        let cross = axis.cross();
        let (start, end) = (cross.start(&content), cross.end(&content));
        let offsets: Vec<(f64, f64)> = items
            .iter()
            .map(|i| (cross.start(&i.rect) - start, end - cross.end(&i.rect)))
            .collect();
        let fills: Vec<bool> = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let (before, after) = offsets[index];
                match item.kind {
                    ItemKind::Media => false,
                    // Wrapped text fills the width it wraps in; one-line
                    // text keeps hugging.
                    ItemKind::Text { single_line } => {
                        !single_line && cross == Axis::Horizontal && before.abs() <= EPS
                    }
                    ItemKind::Frame => before.abs() <= EPS && after.abs() <= EPS,
                }
            })
            .collect();
        let placed: Vec<(f64, f64)> = (0..items.len())
            .filter(|&index| !fills[index])
            .map(|index| offsets[index])
            .collect();
        let fits = |align: &AlignItems| {
            placed
                .iter()
                .filter(|(before, after)| match align {
                    AlignItems::Start => before.abs() <= EPS,
                    AlignItems::Center => (before - after).abs() <= 2.0 * EPS,
                    AlignItems::End => after.abs() <= EPS,
                    AlignItems::Stretch => false,
                })
                .count()
        };
        let preferred = match model.align.as_deref() {
            Some("center") => AlignItems::Center,
            Some("flex-end" | "end") => AlignItems::End,
            _ => AlignItems::Start,
        };
        let mut best = preferred.clone();
        for candidate in [AlignItems::Start, AlignItems::Center, AlignItems::End] {
            if fits(&candidate) > fits(&best) {
                best = candidate;
            }
        }
        // One alignment for the whole line: an item that keeps its own cross
        // offset (`self-end`, a top margin) cannot be expressed, so the line
        // stays absolute rather than moving it.
        let aligned = fits(&best) == placed.len();
        let low = items
            .iter()
            .map(|i| cross.start(&i.rect))
            .fold(f64::INFINITY, f64::min);
        let high = items
            .iter()
            .map(|i| cross.end(&i.rect))
            .fold(f64::NEG_INFINITY, f64::max);
        let hug = !fills.iter().any(|fill| *fill)
            && (low - start).abs() <= EPS
            && (end - high).abs() <= EPS;
        Cross {
            align: (best != AlignItems::Start).then_some(best),
            fills,
            hug,
            aligned,
        }
    }
}

/// Group flow items into lines along the container's flow, sorted, or
/// `None` when they cannot be read as lines (a flex column whose items do
/// not stack). Returns each line's axis and item count, in order; `items`
/// is reordered to match.
pub(super) fn lines_of(model: &BoxModel, items: &mut [Item]) -> Option<Vec<(Axis, usize)>> {
    if model.direction == Some(Axis::Vertical) {
        return stacks(items).then(|| vec![(Axis::Vertical, items.len())]);
    }
    if model.direction.is_none() && stacks(items) {
        return Some(vec![(Axis::Vertical, items.len())]);
    }
    // Rows: an item joins the current line while it overlaps the line's
    // vertical band.
    items.sort_by(|a, b| {
        a.rect
            .y
            .total_cmp(&b.rect.y)
            .then(a.rect.x.total_cmp(&b.rect.x))
    });
    let mut lines: Vec<(f64, f64, usize)> = Vec::new();
    for item in items.iter() {
        let (top, bottom) = (item.rect.y, item.rect.y + item.rect.h);
        match lines.last_mut() {
            Some(line) if top < line.1 - EPS => {
                line.1 = line.1.max(bottom);
                line.2 += 1;
            }
            _ => lines.push((top, bottom, 1)),
        }
    }
    if lines.len() > 1 && model.flex && !model.wrap {
        return None;
    }
    let mut start = 0;
    for (_, _, count) in &lines {
        items[start..start + count].sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
        start += count;
    }
    Some(
        lines
            .into_iter()
            .map(|(_, _, count)| (Axis::Horizontal, count))
            .collect(),
    )
}

/// Do the items, in document order, sit one below the other?
fn stacks(items: &mut [Item]) -> bool {
    items.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y));
    items
        .windows(2)
        .all(|pair| pair[1].rect.y >= pair[0].rect.y + pair[0].rect.h - EPS)
}
