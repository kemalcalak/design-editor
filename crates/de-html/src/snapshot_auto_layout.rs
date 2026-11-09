//! Auto-layout reconstruction for browser snapshots (#292).
//!
//! The absolute snapshot import pins every box where the browser put it,
//! which is exact but dead: change a label and nothing around it moves. This
//! pass rebuilds each captured element as a `horizontal` / `vertical` frame
//! from the geometry the browser already resolved, so nothing re-runs the
//! cascade. The capture's `layout` box (flex direction, wrapping, growth,
//! padding, border widths) says what the author asked for; the child rects
//! say what it came to, and every decision below is checked against them:
//!
//! - **axis** — a flex container's direction, else the children's own
//!   arrangement (stacked → vertical, one row → horizontal, several rows →
//!   a vertical stack of `row` frames, which is also how wrapped flex rows
//!   and grids come in);
//! - **gap** — the smallest space between neighbours. Tailwind's
//!   `space-x-*` / `space-y-*` are margins, not `gap`, so the CSS value only
//!   breaks the tie of two items pushed apart; a larger space becomes a fixed
//!   `spacer` frame, and the single one that soaks up all the free space
//!   (`ml-auto`, at least `FREE_SPACE` past the gap) a filling spacer. A
//!   vector icon is spaced by its element box, its inset artwork riding in a
//!   frame of that box;
//! - **padding** — CSS padding plus border widths, shrunk on any side the
//!   children reach into, plus the space a line leaves at its ends when that
//!   space is not free space (an offset first child, a filling item);
//! - **alignment** — whichever of start / center / end the children share,
//!   the CSS `align-items` or the text's `text-align` breaking a tie;
//! - **sizing** — `fill_container` for a flex item that grew past its content
//!   (a content-sized `flex-1` item hugs) or a child that spans its parent's
//!   cross axis, `fit_content` for a box whose content decides its size (one
//!   that left no free space, whatever its `justify-content`, and a block
//!   shrink-wrapped around its widest child), a number for everything else.
//!   A `truncate` run the browser cut fills its box and ends in "…" there.
//!
//! A container whose children overlap along the axis (a negative-margin
//! avatar stack) keeps `layout: none`; in a stack of lines only the
//! overlapping line does, as a fixed-size layer that keeps its place in the
//! stack (an avatar pulled up over a banner), and so does a flex column's
//! band of overlapping items. A box a CSS transform moved is laid out by
//! its untransformed box (the capture's `flowRect`) and drawn where the
//! transform put it, inside a slot of that box. `position: absolute |
//! fixed` children stay absolutely placed inside any frame.

use std::collections::BTreeMap;

use jian_ops_schema::node::base::{NumberOrExpression, PenNodeBase};
use jian_ops_schema::node::container::{
    AlignItems, ContainerProps, JustifyContent, LayoutMode, Padding,
};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};
use serde_json::{Map, Value};

use super::snapshot_stack::PaintBucket;
use super::{Rect, SnapshotCtx};

#[path = "snapshot_auto_layout_flow.rs"]
mod flow;
#[path = "snapshot_auto_layout_items.rs"]
mod items;
#[path = "snapshot_auto_layout_lines.rs"]
mod lines;
use flow::lines_of;
use items::{cut_at, fitted_inset, hug_of, set_size, union_of};

/// Within this many CSS px two edges count as the same edge: the capture
/// rounds every rect to hundredths, and sub-pixel layout differs by less.
const EPS: f64 = 0.75;

/// Space between two flex items wider than their gap by at least this much
/// reads as pushed apart (`ml-auto`) rather than spaced (`space-x-*`).
const FREE_SPACE: f64 = 24.0;

/// One mapped child with the geometry its layout is decided from.
pub(super) struct Item {
    pub node: PenNode,
    pub bucket: PaintBucket,
    /// The box the child occupies in its parent's flow. For text this is the
    /// line box (glyph box grown to the line-height), which is what the
    /// editor lays the text node out as.
    pub rect: Rect,
    /// `position: absolute | fixed`: placed, not laid out.
    pub overlay: bool,
    /// `flex-grow` above zero (meaningful in a flex parent only).
    pub grow: bool,
    pub kind: ItemKind,
    /// One line of text the browser cut short with "…": it ran past its
    /// parent's content box, which clips it (`truncate`). It fills that
    /// box and keeps cutting there after an edit. Set from the run's
    /// `text-overflow` first; `cut_at` keeps it only where the run did
    /// overflow.
    pub cut: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemKind {
    Frame,
    /// A text run; `single_line` hugs both axes.
    Text {
        single_line: bool,
    },
    /// Images, vectors and icons: always their captured size.
    Media,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    fn start(self, rect: &Rect) -> f64 {
        match self {
            Axis::Horizontal => rect.x,
            Axis::Vertical => rect.y,
        }
    }
    fn size(self, rect: &Rect) -> f64 {
        match self {
            Axis::Horizontal => rect.w,
            Axis::Vertical => rect.h,
        }
    }
    fn end(self, rect: &Rect) -> f64 {
        self.start(rect) + self.size(rect)
    }
    fn cross(self) -> Axis {
        match self {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        }
    }
    fn mode(self) -> LayoutMode {
        match self {
            Axis::Horizontal => LayoutMode::Horizontal,
            Axis::Vertical => LayoutMode::Vertical,
        }
    }
}

/// How one element came out: its layout fields, its children in canonical
/// order, and whether its content decides its size on each axis.
pub(super) struct Laid {
    pub layout: LayoutMode,
    pub gap: Option<f64>,
    pub padding: Option<Padding>,
    pub justify: Option<JustifyContent>,
    pub align: Option<AlignItems>,
    pub children: Vec<PenNode>,
    /// `(width, height)`: the frame may be `fit_content` on that axis.
    pub hug: (bool, bool),
    /// What the frame would take if it hugged its content.
    pub natural: Option<Natural>,
}

/// The box a frame's content would take if the frame hugged it, read by a
/// parent deciding whether the frame was shrink-wrapped
/// (`cross_alignment`'s `shrink_wrapped`).
#[derive(Clone, Copy)]
pub(super) struct Natural {
    pub rect: Rect,
    /// `(width, height)`: hugging keeps the layout on that axis (no child
    /// fills it, and along the line no free space was shared out).
    pub can_hug: (bool, bool),
}

impl Laid {
    /// Write the layout fields onto `container`; hands back the children.
    pub fn apply(self, container: &mut ContainerProps) -> Vec<PenNode> {
        container.layout = Some(self.layout);
        container.gap = self.gap.map(NumberOrExpression::Number);
        container.padding = self.padding;
        container.justify_content = self.justify;
        container.align_items = self.align;
        self.children
    }
}

/// The capture's `layout` box for one element.
pub(super) struct BoxModel {
    pub flex: bool,
    pub direction: Option<Axis>,
    pub wrap: bool,
    pub justify: Option<String>,
    pub align: Option<String>,
    /// top, right, bottom, left: padding plus border widths.
    pub inset: [f64; 4],
    /// CSS `row-gap`, `column-gap`.
    pub gap: [f64; 2],
}

impl BoxModel {
    pub fn read(object: &Map<String, Value>, styles: &BTreeMap<String, String>) -> Self {
        let layout = object.get("layout").and_then(Value::as_object);
        let text = |key: &str| layout.and_then(|l| l.get(key)).and_then(Value::as_str);
        let sides = |key: &str| -> [f64; 4] {
            let mut out = [0.0; 4];
            if let Some(values) = layout.and_then(|l| l.get(key)).and_then(Value::as_array) {
                for (slot, value) in out.iter_mut().zip(values) {
                    *slot = value.as_f64().filter(|v| v.is_finite()).unwrap_or(0.0);
                }
            }
            out
        };
        let display = styles.get("display").map(|d| d.trim().to_ascii_lowercase());
        let flex = matches!(display.as_deref(), Some("flex" | "inline-flex"));
        let direction = flex.then(|| match text("direction") {
            Some(d) if d.starts_with("column") => Axis::Vertical,
            _ => Axis::Horizontal,
        });
        let padding = sides("padding");
        let border = sides("border");
        let mut inset = [0.0; 4];
        for side in 0..4 {
            inset[side] = padding[side] + border[side];
        }
        Self {
            flex,
            direction,
            wrap: flex && text("wrap").is_some_and(|w| w != "nowrap"),
            justify: text("justify").map(str::to_string),
            align: text("align").map(str::to_string),
            inset,
            gap: {
                let gap = sides("gap");
                [gap[0], gap[1]]
            },
        }
    }
}

impl SnapshotCtx<'_> {
    /// Map one element's captured children and lay them out.
    pub(super) fn auto_children(
        &mut self,
        object: &Map<String, Value>,
        styles: &BTreeMap<String, String>,
        rect: Rect,
        flex_or_grid: bool,
    ) -> Laid {
        let children = object
            .get("children")
            .and_then(Value::as_array)
            .map(|children| lines::merge_wrapped_lines(children, rect.w))
            .unwrap_or_default();
        let model = BoxModel::read(object, styles);
        let mut items = Vec::with_capacity(children.len());
        for child in children.iter().map(|child| child.as_ref()) {
            let bucket = super::snapshot_stack::paint_bucket(child, flex_or_grid);
            let node = self
                .map_child(child, rect)
                .and_then(|node| self.unless_escaping(child, node, bucket));
            if let Some(node) = node {
                if let Some(item) = self.item(child, node, bucket) {
                    items.push(item);
                }
            }
        }
        self.auto_layout(&model, rect, items)
    }

    /// Lay `items` out inside `rect`. Overlays keep their placement; the
    /// rest becomes flow, or the element falls back to `layout: none` when
    /// its children cannot be expressed as one.
    pub(super) fn auto_layout(&mut self, model: &BoxModel, rect: Rect, items: Vec<Item>) -> Laid {
        let (overlays, mut flow): (Vec<Item>, Vec<Item>) =
            items.into_iter().partition(|item| item.overlay);
        if flow.is_empty() {
            return absolute(overlays, Vec::new());
        }
        let content = Rect {
            x: rect.x + model.inset[3],
            w: (rect.w - model.inset[1] - model.inset[3]).max(0.0),
            ..rect
        };
        cut_at(&mut flow, content);
        let inset = fitted_inset(model.inset, rect, &flow);
        let content = Rect {
            x: rect.x + inset[3],
            y: rect.y + inset[0],
            w: (rect.w - inset[1] - inset[3]).max(0.0),
            h: (rect.h - inset[0] - inset[2]).max(0.0),
        };
        let lines = match lines_of(model, &mut flow) {
            Some(lines) => lines,
            None => return absolute(overlays, flow),
        };
        // The stack of rows is not the flex line: its spacing is the rows'
        // own geometry, never the items' `justify-content` / `align-items`
        // (`align-content` is not captured). A column's bands are its own
        // line, so a flex column keeps them.
        let column = model.direction == Some(Axis::Vertical);
        let stack = BoxModel {
            flex: column && model.flex,
            direction: Some(Axis::Vertical),
            wrap: false,
            justify: model.justify.clone().filter(|_| column),
            align: model.align.clone().filter(|_| column),
            inset: model.inset,
            gap: model.gap,
        };
        let (axis, flow_items, model) = if lines.len() == 1 {
            let axis = model.direction.unwrap_or(if flow.len() == 1 {
                Axis::Vertical
            } else {
                lines[0].0
            });
            (axis, flow, model)
        } else {
            // Several rows: each becomes a horizontal `row` frame spanning
            // the content width, stacked vertically (a column's band of
            // overlapping items a layer).
            let mut rows = Vec::new();
            let mut rest = flow.into_iter();
            for (line_axis, count) in &lines {
                let row: Vec<Item> = rest.by_ref().take(*count).collect();
                let layer = *line_axis == Axis::Vertical && *count > 1;
                rows.push(self.row_frame(model, rect, content, row, layer));
            }
            (Axis::Vertical, rows, &stack)
        };
        let flow_items = match axis {
            Axis::Vertical => self.band_strays(model, rect, content, flow_items),
            Axis::Horizontal => flow_items,
        };
        match self.arrange(model, axis, content, flow_items) {
            Ok(mut laid) => {
                let extra = match laid.padding {
                    Some(Padding::LtrB(values)) => values,
                    _ => [0.0; 4],
                };
                // The content box grown by the padding the frame ends up
                // with (`inset` and `extra` are top, right, bottom, left).
                let [top, right, bottom, left] =
                    std::array::from_fn(|side| inset[side] + extra[side]);
                for natural in laid.natural.iter_mut() {
                    let content = natural.rect;
                    natural.rect = Rect {
                        x: content.x - left,
                        y: content.y - top,
                        w: content.w + left + right,
                        h: content.h + top + bottom,
                    };
                }
                laid.padding = padding_of(inset, laid.padding.take());
                laid.children = with_overlays(overlays, laid.children);
                laid
            }
            Err(flow) => absolute(overlays, flow),
        }
    }

    /// A stack item that keeps a cross offset of its own (an avatar's
    /// `ml-4` beside full-width siblings) cannot share the stack's
    /// alignment: it rides in a fixed layer the width of the content box,
    /// at its own offset, as an overlapping line does, and the rest of the
    /// stack stays in flow.
    fn band_strays(
        &mut self,
        model: &BoxModel,
        rect: Rect,
        content: Rect,
        items: Vec<Item>,
    ) -> Vec<Item> {
        let cross = self.cross_alignment(model, Axis::Vertical, content, &items);
        if cross.aligned {
            return items;
        }
        items
            .into_iter()
            .zip(cross.strays)
            .map(|(item, stray)| match stray {
                true => self.row_frame(model, rect, content, vec![item], true),
                false => item,
            })
            .collect()
    }

    /// Wrap one line of a multi-line container (`rect`) in a horizontal
    /// frame that spans the container's content width; `layer` keeps the
    /// line's items placed instead (a column's overlapping band, a stray).
    fn row_frame(
        &mut self,
        model: &BoxModel,
        rect: Rect,
        content: Rect,
        row: Vec<Item>,
        layer: bool,
    ) -> Item {
        if row.len() == 1 && !layer {
            return row.into_iter().next().expect("one item");
        }
        let top = row.iter().map(|i| i.rect.y).fold(f64::INFINITY, f64::min);
        let bottom = row
            .iter()
            .map(|i| i.rect.y + i.rect.h)
            .fold(f64::NEG_INFINITY, f64::max);
        let line = Rect {
            x: content.x,
            y: top,
            w: content.w,
            h: (bottom - top).max(0.0),
        };
        let id = self.auxiliary_id("row");
        let row_model = BoxModel {
            flex: model.flex,
            direction: Some(Axis::Horizontal),
            wrap: false,
            justify: model.justify.clone(),
            align: model.align.clone(),
            inset: [0.0; 4],
            gap: model.gap,
        };
        // Where the row sits in the element: read only if the element itself
        // falls back to `layout: none` (flow clears it, `into_flow`).
        let (dx, dy) = (line.x - rect.x, line.y - rect.y);
        let arranged = if layer {
            Err(row)
        } else {
            self.arrange(&row_model, Axis::Horizontal, line, row)
        };
        let (mut frame, height) = match arranged {
            Ok(laid) => (frame_node(id, "row", laid), Fit::Hug),
            // Overlapping items (an avatar pulled up over a banner by a
            // negative margin): a `layout: none` layer the size of the line
            // keeps them where the page put them, and its fixed height keeps
            // the lines after it in place. Absolute children size nothing,
            // so a hugging layer would collapse to 0 and let them ride up.
            Err(mut row) => {
                for item in &mut row {
                    items::shift_back(&mut item.node, dx, dy);
                }
                (frame_node(id, "row", absolute(Vec::new(), row)), Fit::Fixed)
            }
        };
        (frame.base.x, frame.base.y) = (Some(dx), Some(dy));
        let mut node = PenNode::Frame(frame);
        set_size(&mut node, Axis::Horizontal, Fit::Fill, line.w);
        set_size(&mut node, Axis::Vertical, height, line.h);
        self.auto_hug
            .insert(node_id(&node), (false, height == Fit::Hug));
        Item {
            node,
            bucket: PaintBucket::Flow,
            rect: line,
            overlay: false,
            grow: false,
            kind: ItemKind::Frame,
            cut: false,
        }
    }

    /// Lay one line of `items` along `axis` inside `content`. `Err` hands the
    /// items back when they overlap along the axis.
    fn arrange(
        &mut self,
        model: &BoxModel,
        axis: Axis,
        content: Rect,
        mut items: Vec<Item>,
    ) -> Result<Laid, Vec<Item>> {
        items.sort_by(|a, b| axis.start(&a.rect).total_cmp(&axis.start(&b.rect)));
        let gaps: Vec<f64> = items
            .windows(2)
            .map(|pair| axis.start(&pair[1].rect) - axis.end(&pair[0].rect))
            .collect();
        if gaps.iter().any(|gap| *gap < -EPS) {
            return Err(items);
        }
        let leading = axis.start(&items[0].rect) - axis.start(&content);
        let trailing = axis.end(&content) - axis.end(&items[items.len() - 1].rect);
        if leading < -EPS {
            return Err(items);
        }
        // A growing flex item fills only when growing made it larger than its
        // content; one that came out content-sized (a `flex-1` tab in a
        // `w-fit` list: no free space to share) hugs, as it did in the page.
        // `(main, cross)`: does each item's content decide its size?
        let hugs: Vec<(bool, bool)> = items
            .iter()
            .map(|item| {
                let (hug_w, hug_h) = hug_of(&self.auto_hug, &item.node, item.kind);
                match axis {
                    Axis::Horizontal => (hug_w, hug_h),
                    Axis::Vertical => (hug_h, hug_w),
                }
            })
            .collect();
        let fills_main: Vec<bool> = items
            .iter()
            .zip(&hugs)
            .map(|(item, (hug_main, _))| {
                (model.flex && item.grow && !hug_main) || (item.cut && axis == Axis::Horizontal)
            })
            .collect();
        let any_fill = fills_main.iter().any(|fill| *fill);
        let css_gap = match axis {
            Axis::Horizontal => model.gap[1],
            Axis::Vertical => model.gap[0],
        };
        // Items flush with both ends and spaced by exactly the CSS gap left
        // no free space to distribute: the box is as wide as its content
        // (`w-fit`), so its `justify-content` decided nothing; the box hugs
        // and grows with its content, as the page would.
        let packed = leading <= EPS
            && trailing.abs() <= EPS
            && gaps.iter().all(|gap| (gap - css_gap).abs() <= EPS);
        let space_between = !any_fill
            && !packed
            && model.justify.as_deref() == Some("space-between")
            && items.len() > 1;
        let mut base_gap = gaps.iter().copied().fold(f64::INFINITY, f64::min).max(0.0);
        // Two items, flush with both ends, far wider apart than the CSS gap:
        // the space between them is the free space (`ml-auto`, `mr-auto`),
        // not a gap, and has to stay free when either side changes.
        if gaps.len() == 1
            && model.flex
            && leading <= EPS
            && trailing.abs() <= EPS
            && gaps[0] - css_gap >= FREE_SPACE
        {
            base_gap = css_gap;
        }
        let mut extras: Vec<f64> = gaps.iter().map(|gap| gap - base_gap).collect();
        let wide: Vec<usize> = (0..extras.len()).filter(|&i| extras[i] > EPS).collect();
        // Only space well past the gap is free space: a smaller step is a
        // margin (`mr-2` before a label, `ml-1` before a chevron) and stays
        // a fixed spacer.
        let soaks_free_space = !any_fill
            && !space_between
            && wide.len() == 1
            && extras[wide[0]] >= FREE_SPACE
            && leading <= EPS
            && trailing.abs() <= EPS;
        // A fixed spacer sits between two gaps, so it takes its extra space
        // less one gap; a step narrower than that (`mr-2` next to `ml-1`)
        // has no such spacer, and every space becomes one instead.
        if !space_between && !soaks_free_space && wide.iter().any(|&i| extras[i] < base_gap - EPS) {
            base_gap = 0.0;
            extras.clone_from(&gaps);
        }
        let mut lead_pad = 0.0;
        let mut trail_pad = 0.0;
        let justify = if any_fill {
            // A filling item takes the free space, so whatever space is left
            // at either end is structural (a margin, a zero-size sibling's
            // gap): it becomes padding.
            lead_pad = leading.max(0.0);
            trail_pad = trailing.max(0.0);
            JustifyContent::Start
        } else if space_between {
            lead_pad = leading.max(0.0);
            JustifyContent::SpaceBetween
        } else if leading <= EPS || soaks_free_space {
            JustifyContent::Start
        } else if trailing.abs() <= EPS {
            JustifyContent::End
        } else if (leading - trailing).abs() <= 2.0 * EPS {
            JustifyContent::Center
        } else {
            lead_pad = leading;
            JustifyContent::Start
        };
        let cross = self.cross_alignment(model, axis, content, &items);
        if !cross.aligned {
            return Err(items);
        }
        let hug_main = !space_between
            && !any_fill
            && !soaks_free_space
            && leading <= EPS
            && trailing.abs() <= EPS;
        // `padding` is added once the element's own inset is known
        // (`auto_layout`).
        let cross_can_hug = !cross.fills.contains(&true);
        let natural = Natural {
            rect: union_of(&items),
            can_hug: match axis {
                Axis::Horizontal => (hug_main, cross_can_hug),
                Axis::Vertical => (cross_can_hug, hug_main),
            },
        };
        let mut children = Vec::with_capacity(items.len() * 2);
        let count = items.len();
        for (index, item) in items.into_iter().enumerate() {
            let fill_cross = cross.fills[index];
            let mut node = item.node;
            let (hug_main_axis, hug_cross_axis) = hugs[index];
            let main_fit = if fills_main[index] {
                Fit::Fill
            } else if hug_main_axis {
                Fit::Hug
            } else {
                Fit::Fixed
            };
            let cross_fit = if fill_cross {
                Fit::Fill
            } else if hug_cross_axis || cross.unstretched[index] {
                Fit::Hug
            } else {
                Fit::Fixed
            };
            set_size(&mut node, axis, main_fit, axis.size(&item.rect));
            set_size(
                &mut node,
                axis.cross(),
                cross_fit,
                axis.cross().size(&item.rect),
            );
            if item.cut {
                items::truncate(&mut node);
            }
            items::into_flow(&mut node);
            children.push(node);
            if !space_between && index + 1 < count && extras[index] > EPS {
                let (fit, size) = if soaks_free_space {
                    (Fit::Fill, extras[index])
                } else {
                    (Fit::Fixed, extras[index] - base_gap)
                };
                children.push(self.spacer(axis, fit, size));
            }
        }
        let mut padding = [0.0; 4];
        match axis {
            Axis::Horizontal => (padding[3], padding[1]) = (lead_pad, trail_pad),
            Axis::Vertical => (padding[0], padding[2]) = (lead_pad, trail_pad),
        }
        let hug_cross = cross.hug;
        let hug = match axis {
            Axis::Horizontal => (hug_main, hug_cross),
            Axis::Vertical => (hug_cross, hug_main),
        };
        Ok(Laid {
            layout: axis.mode(),
            // Under `space-between` the gap is still the minimum spacing; with
            // a filling item it is the base spacing, wider gaps keeping their
            // fixed spacers.
            gap: (count > 1 && base_gap > 0.0).then_some(round(base_gap)),
            padding: Some(Padding::LtrB(padding)),
            justify: (justify != JustifyContent::Start).then_some(justify),
            align: cross.align,
            children,
            hug,
            natural: Some(natural),
        })
    }

    fn spacer(&mut self, axis: Axis, fit: Fit, size: f64) -> PenNode {
        let id = self.auxiliary_id("spacer");
        let mut node = PenNode::Frame(frame_node(
            id,
            "spacer",
            Laid {
                layout: LayoutMode::None,
                gap: None,
                padding: None,
                justify: None,
                align: None,
                children: Vec::new(),
                hug: (false, false),
                natural: None,
            },
        ));
        set_size(&mut node, axis, fit, round(size));
        set_size(&mut node, axis.cross(), Fit::Fixed, 0.0);
        node
    }

    fn auxiliary_id(&mut self, kind: &str) -> String {
        // Counted against the output budget like every captured node, so
        // the frames this pass adds cannot carry an import past it.
        self.node_count += 1;
        self.auxiliary_ids += 1;
        format!("snapshot_{kind}_{}", self.auxiliary_ids)
    }
}

/// Sizing mode chosen for one axis of one child.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Fit {
    Fixed,
    Hug,
    Fill,
}

impl Fit {
    pub fn sizing(self, size: f64) -> SizingBehavior {
        match self {
            Fit::Fixed => SizingBehavior::Number(round(size)),
            Fit::Hug => SizingBehavior::Keyword(SizingKeyword::FitContent),
            Fit::Fill => SizingBehavior::Keyword(SizingKeyword::FillContainer),
        }
    }
}

/// Keep the element absolutely laid out: every child stays where the
/// browser put it, in canonical paint order.
fn absolute(overlays: Vec<Item>, flow: Vec<Item>) -> Laid {
    let entries = flow
        .into_iter()
        .chain(overlays)
        .map(|item| (item.bucket, item.node))
        .collect();
    Laid {
        layout: LayoutMode::None,
        gap: None,
        padding: None,
        justify: None,
        align: None,
        children: super::snapshot_stack::to_front_to_back(entries),
        hug: (false, false),
        natural: None,
    }
}

/// Overlays paint above the flow (front-to-back among themselves), except
/// those with a negative `z-index`, which paint behind it.
fn with_overlays(overlays: Vec<Item>, flow: Vec<PenNode>) -> Vec<PenNode> {
    let (behind, above): (Vec<Item>, Vec<Item>) = overlays
        .into_iter()
        .partition(|item| matches!(item.bucket, PaintBucket::NegativeZ(_)));
    let order = |list: Vec<Item>| {
        super::snapshot_stack::to_front_to_back(
            list.into_iter()
                .map(|item| (item.bucket, item.node))
                .collect(),
        )
    };
    let mut children = order(above);
    children.extend(flow);
    children.extend(order(behind));
    children
}

/// The element's padding (CSS padding plus border widths) with any space the
/// arrangement folded into the leading or trailing edge.
fn padding_of(inset: [f64; 4], extra: Option<Padding>) -> Option<Padding> {
    let extra = match extra {
        Some(Padding::LtrB(values)) => values,
        _ => [0.0; 4],
    };
    crate::mapper::padding_from_sides(std::array::from_fn(|side| round(inset[side] + extra[side])))
}

/// A plain frame carrying `laid`'s layout, for rows and spacers.
fn frame_node(id: String, name: &str, laid: Laid) -> FrameNode {
    let base = PenNodeBase {
        id,
        name: Some(name.to_string()),
        ..PenNodeBase::default()
    };
    let mut container = ContainerProps::default();
    let children = laid.apply(&mut container);
    match super::frame_with(base, container, children) {
        PenNode::Frame(frame) => frame,
        _ => unreachable!("frame_with builds a frame"),
    }
}

fn node_id(node: &PenNode) -> String {
    match node {
        PenNode::Frame(frame) => frame.base.id.clone(),
        _ => String::new(),
    }
}

/// One captured style value, read without building the whole style map.
pub(super) fn style<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get("styles")?.get(key)?.as_str()
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
#[path = "snapshot_auto_layout_tests.rs"]
mod tests;
