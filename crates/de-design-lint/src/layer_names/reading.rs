//! Reading order: the order a person reads the children of a free-form
//! (unlaid-out) container in, top to bottom in rows and left to right
//! within a row. Laid-out containers read in tree order (their layout
//! places the children in that order).

use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;

use crate::node_util::{children, node_x, node_y, numeric_height, numeric_width, rotation};

use super::rules::{font_size, layout};

/// Two boxes whose vertical centres are this close (or within half the
/// shorter one's height) sit on one row.
const ROW_TOLERANCE_PX: f64 = 4.0;
/// Line height of a text without an authored height, in font sizes.
const TEXT_LINE_HEIGHT: f64 = 1.2;
/// Advance of one character of a text without an authored width, in font
/// sizes.
const TEXT_CHAR_ADVANCE: f64 = 0.5;

/// An axis-aligned box in some container's coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Bounds {
    fn centre_y(&self) -> f64 {
        self.y + self.h / 2.0
    }

    pub(crate) fn offset(self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }

    fn centre(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// The box around this one turned by `degrees` about `pivot`.
    fn rotated_about(self, degrees: f64, pivot: (f64, f64)) -> Self {
        if degrees.rem_euclid(360.0) == 0.0 {
            return self;
        }
        let (sin, cos) = degrees.to_radians().sin_cos();
        let turn = |x: f64, y: f64| {
            let (dx, dy) = (x - pivot.0, y - pivot.1);
            (pivot.0 + dx * cos - dy * sin, pivot.1 + dx * sin + dy * cos)
        };
        let corners = [
            turn(self.x, self.y),
            turn(self.x + self.w, self.y),
            turn(self.x, self.y + self.h),
            turn(self.x + self.w, self.y + self.h),
        ];
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (x, y) in corners {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        Self {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        }
    }

    /// The box around this one turned by `degrees` about its centre.
    #[cfg(test)]
    fn rotated(self, degrees: f64) -> Self {
        self.rotated_about(degrees, self.centre())
    }

    fn union(self, other: Self) -> Self {
        let (x, y) = (self.x.min(other.x), self.y.min(other.y));
        Self {
            x,
            y,
            w: (self.x + self.w).max(other.x + other.w) - x,
            h: (self.y + self.h).max(other.y + other.h) - y,
        }
    }
}

/// Whether `node` places its children by their own `x` / `y`, and every
/// child has a `y` to read.
pub(crate) fn is_free_form(node: &PenNode) -> bool {
    let kids = children(node);
    matches!(layout(node), None | Some(LayoutMode::None))
        && !kids.is_empty()
        && kids.iter().all(|kid| node_y(kid).is_some())
}

fn text_size(node: &PenNode) -> Option<(f64, f64)> {
    let PenNode::Text(text) = node else {
        return None;
    };
    let number = |sizing: &Option<SizingBehavior>| match sizing {
        Some(SizingBehavior::Number(v)) if v.is_finite() && *v > 0.0 => Some(*v),
        _ => None,
    };
    let size = font_size(node);
    let chars = super::rules::text_content(node).map_or(0, |t| t.chars().count()) as f64;
    Some((
        number(&text.width).unwrap_or(chars * size * TEXT_CHAR_ADVANCE),
        number(&text.height).unwrap_or(size * TEXT_LINE_HEIGHT),
    ))
}

/// `node`'s box in its parent's coordinates before its rotation: its
/// authored size, a text's estimated one, or the box around a sizeless
/// container's children (the painter's aggregate bounds).
fn unrotated_bounds(node: &PenNode) -> Bounds {
    let (x, y) = (node_x(node).unwrap_or(0.0), node_y(node).unwrap_or(0.0));
    if let (Some(w), Some(h)) = (numeric_width(node), numeric_height(node)) {
        return Bounds { x, y, w, h };
    }
    if let Some((w, h)) = text_size(node) {
        return Bounds { x, y, w, h };
    }
    let content = children(node)
        .iter()
        .filter(|kid| node_y(kid).is_some())
        .map(bounds_in_parent)
        .reduce(Bounds::union)
        .map(|b| b.offset(x, y));
    match content {
        Some(content) => Bounds {
            w: numeric_width(node).unwrap_or(content.w),
            h: numeric_height(node).unwrap_or(content.h),
            ..content
        },
        None => Bounds {
            x,
            y,
            w: numeric_width(node).unwrap_or(0.0),
            h: numeric_height(node).unwrap_or(0.0),
        },
    }
}

/// `node`'s box in its parent's coordinates, turned by its rotation about
/// the centre of its box, as the painter turns it (`rotation` is in
/// degrees; jian rotates about the node's centre).
pub(crate) fn bounds_in_parent(node: &PenNode) -> Bounds {
    let own = unrotated_bounds(node);
    own.rotated_about(rotation(node), own.centre())
}

/// A box in `node`'s own coordinates (one of its free-form descendants)
/// placed in `node`'s parent's coordinates: moved by `node`'s position and
/// turned with `node` about its centre.
pub(crate) fn to_parent(node: &PenNode, inner: Bounds) -> Bounds {
    let own = unrotated_bounds(node);
    inner
        .offset(node_x(node).unwrap_or(0.0), node_y(node).unwrap_or(0.0))
        .rotated_about(rotation(node), own.centre())
}

/// Group `items` into reading rows, top to bottom: a box joins the row
/// whose first box's centre is within `ROW_TOLERANCE_PX` or half the
/// shorter height of its own.
fn rows<T>(mut items: Vec<(Bounds, T)>) -> Vec<Vec<(Bounds, T)>> {
    items.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
    let mut rows: Vec<Vec<(Bounds, T)>> = Vec::new();
    for item in items {
        let joins = rows
            .last()
            .and_then(|row| row.first())
            .is_some_and(|first| {
                let tolerance = ROW_TOLERANCE_PX.max(first.0.h.min(item.0.h) / 2.0);
                (first.0.centre_y() - item.0.centre_y()).abs() <= tolerance
            });
        match rows.last_mut() {
            Some(row) if joins => row.push(item),
            _ => rows.push(vec![item]),
        }
    }
    rows
}

/// Sort `items` by their boxes into reading order: rows top to bottom,
/// left to right within a row. Equal boxes keep their order.
pub(crate) fn sort_reading<T>(items: &mut Vec<(Bounds, T)>) {
    for mut row in rows(std::mem::take(items)) {
        row.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        items.extend(row);
    }
}

/// Whether `boxes` sit on one reading row.
pub(crate) fn single_row(boxes: &[Bounds]) -> bool {
    rows(boxes.iter().map(|b| (*b, ())).collect()).len() == 1
}

/// `node`'s children in reading order: by their boxes in a free-form
/// container, in tree order otherwise.
pub(crate) fn ordered_children(node: &PenNode) -> Vec<&PenNode> {
    if !is_free_form(node) {
        return children(node).iter().collect();
    }
    let mut kids: Vec<(Bounds, &PenNode)> = children(node)
        .iter()
        .map(|kid| (bounds_in_parent(kid), kid))
        .collect();
    sort_reading(&mut kids);
    kids.into_iter().map(|(_, kid)| kid).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
        Bounds { x, y, w, h }
    }

    #[test]
    fn a_row_reads_left_to_right_despite_small_offsets() {
        let mut items = vec![
            (b(200.0, 3.0, 80.0, 20.0), "right"),
            (b(0.0, 0.0, 80.0, 24.0), "left"),
            (b(0.0, 60.0, 80.0, 20.0), "below"),
        ];
        sort_reading(&mut items);
        let order: Vec<_> = items.iter().map(|(_, n)| *n).collect();
        assert_eq!(order, ["left", "right", "below"]);
    }

    #[test]
    fn rotation_widens_the_box_about_its_centre() {
        let turned = b(0.0, 0.0, 100.0, 20.0).rotated(90.0);
        assert!((turned.w - 20.0).abs() < 1e-9 && (turned.h - 100.0).abs() < 1e-9);
        assert!((turned.y + 40.0).abs() < 1e-9, "{turned:?}");
    }
}
