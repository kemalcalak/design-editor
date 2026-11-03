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
//!   (`ml-auto`) a filling spacer;
//! - **padding** — CSS padding plus border widths, shrunk on any side the
//!   children reach into, plus the space a line leaves at its ends when that
//!   space is not free space (an offset first child, a filling item);
//! - **alignment** — whichever of start / center / end the children share;
//! - **sizing** — `fill_container` for a flex item that grew past its content
//!   (a content-sized `flex-1` item hugs) or a child that spans its parent's
//!   cross axis, `fit_content` for a box whose content decides its size, a
//!   number for everything else.
//!
//! A container whose children overlap along the axis (a negative-margin
//! avatar stack) keeps `layout: none`, and `position: absolute | fixed`
//! children stay absolutely placed inside any frame.

use std::collections::BTreeMap;

use jian_ops_schema::node::base::{NumberOrExpression, PenNodeBase};
use jian_ops_schema::node::container::{
    AlignItems, ContainerProps, JustifyContent, LayoutMode, Padding,
};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};
use serde_json::{Map, Value};

use super::snapshot_stack::PaintBucket;
use super::{parse_px, Rect, SnapshotCtx};

#[path = "snapshot_auto_layout_flow.rs"]
mod flow;
#[path = "snapshot_auto_layout_items.rs"]
mod items;
#[path = "snapshot_auto_layout_lines.rs"]
mod lines;
use flow::lines_of;
use items::{hug_of, set_size};

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
            if let Some(node) = self.map_child(child, rect) {
                if let Some(item) = self.item(child, node, bucket) {
                    items.push(item);
                }
            }
        }
        self.auto_layout(&model, rect, items)
    }

    /// The geometry and role of one mapped child.
    fn item(&self, child: &Value, node: PenNode, bucket: PaintBucket) -> Option<Item> {
        let object = child.as_object()?;
        let rect = Rect::from_node(object)?;
        let overlay = matches!(
            style(object, "position").map(str::trim),
            Some("absolute" | "fixed")
        );
        let grow = object
            .get("layout")
            .and_then(|layout| layout.get("grow"))
            .and_then(Value::as_f64)
            .is_some_and(|grow| grow > 0.0);
        let (kind, rect) = match &node {
            PenNode::Frame(_) => (ItemKind::Frame, rect),
            PenNode::Text(_) => {
                let lines = object.get("lines").and_then(Value::as_u64).unwrap_or(1);
                let nowrap = style(object, "white-space") == Some("nowrap");
                let single_line = lines <= 1 || nowrap;
                let rect = line_box(object, rect, self.opts.base_font_size);
                (ItemKind::Text { single_line }, rect)
            }
            // A vector path is sized to its artwork, not the element box.
            PenNode::Path(_) => (
                ItemKind::Media,
                Rect::from_field(object, "vectorRect")
                    .filter(|r| r.w > 0.0 && r.h > 0.0)
                    .unwrap_or(rect),
            ),
            _ => (ItemKind::Media, rect),
        };
        Some(Item {
            node,
            bucket,
            rect,
            overlay,
            grow,
            kind,
        })
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
        // (`align-content` is not captured).
        let stack = BoxModel {
            flex: false,
            direction: Some(Axis::Vertical),
            wrap: false,
            justify: None,
            align: None,
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
            // the content width, stacked vertically.
            let mut rows = Vec::new();
            let mut rest = flow.into_iter();
            for (_, count) in &lines {
                let row: Vec<Item> = rest.by_ref().take(*count).collect();
                rows.push(self.row_frame(model, content, row));
            }
            (Axis::Vertical, rows, &stack)
        };
        match self.arrange(model, axis, content, flow_items) {
            Ok(mut laid) => {
                laid.padding = padding_of(inset, laid.padding.take());
                laid.children = with_overlays(overlays, laid.children);
                laid
            }
            Err(flow) => absolute(overlays, flow),
        }
    }

    /// Wrap one line of a multi-line container in a horizontal frame that
    /// spans the container's content width.
    fn row_frame(&mut self, model: &BoxModel, content: Rect, row: Vec<Item>) -> Item {
        if row.len() == 1 {
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
        let frame = match self.arrange(&row_model, Axis::Horizontal, line, row) {
            Ok(laid) => frame_node(id, "row", laid),
            Err(row) => frame_node(id, "row", absolute(Vec::new(), row)),
        };
        let mut node = PenNode::Frame(frame);
        set_size(&mut node, Axis::Horizontal, Fit::Fill, line.w);
        set_size(&mut node, Axis::Vertical, Fit::Hug, line.h);
        self.auto_hug.insert(node_id(&node), (false, true));
        Item {
            node,
            bucket: PaintBucket::Flow,
            rect: line,
            overlay: false,
            grow: false,
            kind: ItemKind::Frame,
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
            .map(|(item, (hug_main, _))| model.flex && item.grow && !hug_main)
            .collect();
        let any_fill = fills_main.iter().any(|fill| *fill);
        let space_between =
            !any_fill && model.justify.as_deref() == Some("space-between") && items.len() > 1;
        let mut base_gap = gaps.iter().copied().fold(f64::INFINITY, f64::min).max(0.0);
        // Two items, flush with both ends, far wider apart than the CSS gap:
        // the space between them is the free space (`ml-auto`, `mr-auto`),
        // not a gap, and has to stay free when either side changes.
        let css_gap = match axis {
            Axis::Horizontal => model.gap[1],
            Axis::Vertical => model.gap[0],
        };
        if gaps.len() == 1
            && model.flex
            && leading <= EPS
            && trailing.abs() <= EPS
            && gaps[0] - css_gap >= FREE_SPACE
        {
            base_gap = css_gap;
        }
        let extras: Vec<f64> = gaps.iter().map(|gap| gap - base_gap).collect();
        let wide: Vec<usize> = (0..extras.len()).filter(|&i| extras[i] > EPS).collect();
        let soaks_free_space = !any_fill
            && !space_between
            && wide.len() == 1
            && leading <= EPS
            && trailing.abs() <= EPS;
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
            } else if hug_cross_axis {
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
            items::into_flow(&mut node);
            children.push(node);
            if !space_between && index + 1 < count && extras[index] > EPS {
                let fit = if soaks_free_space {
                    Fit::Fill
                } else {
                    Fit::Fixed
                };
                children.push(self.spacer(axis, fit, extras[index]));
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

/// The element's padding and border, shrunk on any side its flow children
/// reach into. Content that overflows its padding box (a 20 px label in a
/// 32 px button with 8 px padding and a border) still sits inside the box
/// in the browser; keeping the full padding there makes the editor grow
/// the box around it, and that growth travels up every hugging ancestor.
fn fitted_inset(inset: [f64; 4], rect: Rect, flow: &[Item]) -> [f64; 4] {
    let low_x = flow.iter().map(|i| i.rect.x).fold(f64::INFINITY, f64::min);
    let low_y = flow.iter().map(|i| i.rect.y).fold(f64::INFINITY, f64::min);
    let high_x = flow
        .iter()
        .map(|i| i.rect.x + i.rect.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let high_y = flow
        .iter()
        .map(|i| i.rect.y + i.rect.h)
        .fold(f64::NEG_INFINITY, f64::max);
    let reach = [
        low_y - rect.y,
        rect.x + rect.w - high_x,
        rect.y + rect.h - high_y,
        low_x - rect.x,
    ];
    let mut fitted = inset;
    for side in 0..4 {
        if reach[side] < inset[side] - EPS {
            fitted[side] = reach[side].max(0.0);
        }
    }
    fitted
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

/// Text line box: the glyph box the capture measured, grown to the run's
/// line-height (the height the editor gives the text node), centred on it.
pub(super) fn line_box(object: &Map<String, Value>, rect: Rect, default_font: f64) -> Rect {
    let font_size = style(object, "font-size")
        .and_then(parse_px)
        .unwrap_or(default_font);
    let line_height = style(object, "line-height")
        .and_then(parse_px)
        .unwrap_or(font_size * 1.2);
    let lines = object
        .get("lines")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as f64;
    let height = (line_height * lines).max(rect.h);
    Rect {
        y: rect.y - (height - rect.h) / 2.0,
        h: height,
        ..rect
    }
}

/// One captured style value, read without building the whole style map.
fn style<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get("styles")?.get(key)?.as_str()
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
#[path = "snapshot_auto_layout_tests.rs"]
mod tests;
