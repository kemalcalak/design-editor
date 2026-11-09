//! Reading a container's flow from its children: which way they line up,
//! where lines break, and how they sit on the cross axis.

use jian_ops_schema::node::container::AlignItems;
use jian_ops_schema::node::text::TextAlign;
use jian_ops_schema::node::PenNode;

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
    /// Per item: it does not fill and keeps a cross offset of its own (a
    /// left margin in a stack), off the chosen alignment.
    pub strays: Vec<bool>,
    /// Per item: it only spanned a box shrink-wrapped around the widest
    /// item, so it hugs its own content instead (`shrink_wrapped`).
    pub unstretched: Vec<bool>,
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
        let offsets_of = |rects: &[Rect]| -> Vec<(f64, f64)> {
            rects
                .iter()
                .map(|r| (cross.start(r) - start, end - cross.end(r)))
                .collect()
        };
        let mut rects: Vec<Rect> = items.iter().map(|item| item.rect).collect();
        let mut offsets = offsets_of(&rects);
        let mut fills: Vec<bool> = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let (before, after) = offsets[index];
                match item.kind {
                    ItemKind::Media => false,
                    // Wrapped text fills the width it wraps in, and so does
                    // a cut line; one-line text keeps hugging.
                    ItemKind::Text { single_line } => {
                        (!single_line || item.cut)
                            && cross == Axis::Horizontal
                            && before.abs() <= EPS
                    }
                    ItemKind::Frame => before.abs() <= EPS && after.abs() <= EPS,
                }
            })
            .collect();
        let mut unstretched = vec![false; items.len()];
        if let Some(natural) = self.shrink_wrapped(cross, content, items, &fills) {
            // The items that filled hug instead, and none fills.
            unstretched = fills;
            fills = vec![false; items.len()];
            rects = natural;
            offsets = offsets_of(&rects);
        }
        let placed: Vec<(f64, f64)> = (0..items.len())
            .filter(|&index| !fills[index])
            .map(|index| offsets[index])
            .collect();
        let sits = |align: &AlignItems, (before, after): &(f64, f64)| match align {
            AlignItems::Start => before.abs() <= EPS,
            AlignItems::Center => (before - after).abs() <= 2.0 * EPS,
            AlignItems::End => after.abs() <= EPS,
            AlignItems::Stretch => false,
        };
        let fits =
            |align: &AlignItems| placed.iter().filter(|offsets| sits(align, offsets)).count();
        // The geometry cannot tell the alignments apart when the items are
        // as wide as the box (a heading its text sizes): the CSS intent
        // decides, so the line stays put when a label changes.
        let preferred = match model.align.as_deref() {
            Some("center") => AlignItems::Center,
            Some("flex-end" | "end") => AlignItems::End,
            None | Some("normal" | "stretch") => {
                text_alignment(cross, items).unwrap_or(AlignItems::Start)
            }
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
        let strays = (0..items.len())
            .map(|index| !fills[index] && !sits(&best, &offsets[index]))
            .collect();
        let low = rects
            .iter()
            .map(|r| cross.start(r))
            .fold(f64::INFINITY, f64::min);
        let high = rects
            .iter()
            .map(|r| cross.end(r))
            .fold(f64::NEG_INFINITY, f64::max);
        let hug = !fills.iter().any(|fill| *fill)
            && (low - start).abs() <= EPS
            && (end - high).abs() <= EPS;
        Cross {
            align: (best != AlignItems::Start).then_some(best),
            fills,
            hug,
            aligned,
            strays,
            unstretched,
        }
    }

    /// A box as wide as its widest item's content (a block shrink-wrapped
    /// by its flex parent, `w-fit`) only looks spanned by its items: each
    /// was stretched to the widest. Returns each item's content box when
    /// that is so, every spanning item being a frame without a visible box
    /// of its own whose content can hug; those hug and the box hugs them,
    /// so a longer label grows the box instead of overflowing it.
    fn shrink_wrapped(
        &self,
        cross: Axis,
        content: Rect,
        items: &[Item],
        fills: &[bool],
    ) -> Option<Vec<Rect>> {
        if !fills.contains(&true) {
            return None;
        }
        let mut rects = Vec::with_capacity(items.len());
        for (item, fill) in items.iter().zip(fills) {
            if !fill {
                rects.push(item.rect);
                continue;
            }
            let PenNode::Frame(frame) = &item.node else {
                return None;
            };
            let painted = |list: Option<usize>| list.is_some_and(|len| len > 0);
            if painted(frame.container.fill.as_ref().map(Vec::len))
                || painted(frame.container.effects.as_ref().map(Vec::len))
                || frame.container.stroke.is_some()
            {
                return None;
            }
            let natural = self.auto_natural.get(&frame.base.id)?;
            let can_hug = match cross {
                Axis::Horizontal => natural.can_hug.0,
                Axis::Vertical => natural.can_hug.1,
            };
            if !can_hug {
                return None;
            }
            rects.push(natural.rect);
        }
        let widest = rects.iter().map(|r| cross.size(r)).fold(0.0, f64::max);
        ((widest - cross.size(&content)).abs() <= EPS).then_some(rects)
    }
}

/// The `text-align` every text run in a stack shares, as the alignment it
/// asks of the stack (a block's `text-align: center` centres its lines).
fn text_alignment(cross: Axis, items: &[Item]) -> Option<AlignItems> {
    if cross != Axis::Horizontal {
        return None;
    }
    let mut aligns = items.iter().filter_map(|item| match &item.node {
        PenNode::Text(text) => Some(match text.text_align {
            Some(TextAlign::Center) => AlignItems::Center,
            Some(TextAlign::Right) => AlignItems::End,
            _ => AlignItems::Start,
        }),
        _ => None,
    });
    let first = aligns.next()?;
    aligns.all(|align| align == first).then_some(first)
}

/// Group flow items into lines along the container's flow, sorted, or
/// `None` when they cannot be read as lines (a flex row that does not
/// wrap yet holds several, a flex column all of whose items overlap).
/// Returns each line's axis and item count, in order; `items` is reordered
/// to match. A line of a column is `Axis::Vertical`: items there share a
/// band only by overlapping (an avatar pulled up over its cover by a
/// negative margin), so such a band is a layer, never a row.
pub(super) fn lines_of(model: &BoxModel, items: &mut [Item]) -> Option<Vec<(Axis, usize)>> {
    if model.direction == Some(Axis::Vertical) {
        if stacks(items) {
            return Some(vec![(Axis::Vertical, items.len())]);
        }
        let bands = bands(items);
        return (bands.len() > 1).then(|| {
            bands
                .into_iter()
                .map(|count| (Axis::Vertical, count))
                .collect()
        });
    }
    if model.direction.is_none() && stacks(items) {
        return Some(vec![(Axis::Vertical, items.len())]);
    }
    let lines = bands(items);
    if lines.len() > 1 && model.flex && !model.wrap {
        return None;
    }
    let mut start = 0;
    for count in &lines {
        items[start..start + count].sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
        start += count;
    }
    Some(
        lines
            .into_iter()
            .map(|count| (Axis::Horizontal, count))
            .collect(),
    )
}

/// Sort the items top to bottom and count them into bands: an item joins
/// the current band while it overlaps the band's vertical extent.
fn bands(items: &mut [Item]) -> Vec<usize> {
    items.sort_by(|a, b| {
        a.rect
            .y
            .total_cmp(&b.rect.y)
            .then(a.rect.x.total_cmp(&b.rect.x))
    });
    let mut bands: Vec<(f64, usize)> = Vec::new();
    for item in items.iter() {
        let (top, bottom) = (item.rect.y, item.rect.y + item.rect.h);
        match bands.last_mut() {
            Some(band) if top < band.0 - EPS => {
                band.0 = band.0.max(bottom);
                band.1 += 1;
            }
            _ => bands.push((bottom, 1)),
        }
    }
    bands.into_iter().map(|(_, count)| count).collect()
}

/// Do the items, in document order, sit one below the other?
fn stacks(items: &mut [Item]) -> bool {
    items.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y));
    items
        .windows(2)
        .all(|pair| pair[1].rect.y >= pair[0].rect.y + pair[0].rect.h - EPS)
}
