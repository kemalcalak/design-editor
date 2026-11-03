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
    /// Per item: it spanned a box its content was shrink-wrapped to, so it
    /// hugs its own content instead (`shrink_wrapped`).
    pub hugs: Vec<bool>,
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
        let mut hugs = vec![false; items.len()];
        if let Some(natural) = self.shrink_wrapped(cross, content, items, &fills) {
            hugs = std::mem::replace(&mut fills, vec![false; items.len()]);
            rects = natural;
            offsets = offsets_of(&rects);
        }
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
        // The geometry cannot tell the alignments apart when the items are
        // as wide as the box (a heading its text sizes): the CSS intent
        // decides, so the line stays put when a label changes.
        let preferred = match model.align.as_deref() {
            Some("center") => AlignItems::Center,
            Some("flex-end" | "end") => AlignItems::End,
            _ => text_alignment(cross, items).unwrap_or(AlignItems::Start),
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
            hugs,
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
            let (natural, free) = self.auto_natural.get(&frame.base.id)?;
            let can_hug = match cross {
                Axis::Horizontal => free.0,
                Axis::Vertical => free.1,
            };
            if !can_hug {
                return None;
            }
            rects.push(*natural);
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
