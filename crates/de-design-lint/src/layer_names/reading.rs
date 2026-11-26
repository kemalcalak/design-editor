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

    /// The box around this one turned by `degrees` about its centre.
    fn rotated(self, degrees: f64) -> Self {
        if degrees.rem_euclid(360.0) == 0.0 {
            return self;
        }
        let (sin, cos) = degrees.to_radians().sin_cos();
        let (w, h) = (
            self.w * cos.abs() + self.h * sin.abs(),
            self.w * sin.abs() + self.h * cos.abs(),
        );
        Self {
            x: self.x + (self.w - w) / 2.0,
            y: self.y + (self.h - h) / 2.0,
            w,
            h,
        }
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

/// `node`'s box in its parent's coordinates, turned by its rotation: its
/// authored size, a text's estimated one, or the box around a free-form
/// container's children.
pub(crate) fn bounds_in_parent(node: &PenNode) -> Bounds {
    let (w, h) = match (numeric_width(node), numeric_height(node)) {
        (Some(w), Some(h)) => (w, h),
        _ => text_size(node).unwrap_or_else(|| {
            let content = children(node)
                .iter()
                .filter(|kid| node_y(kid).is_some())
                .map(|kid| bounds_in_parent(kid))
                .reduce(Bounds::union);
            let content = content.map_or((0.0, 0.0), |b| (b.x + b.w, b.y + b.h));
            (
                numeric_width(node).unwrap_or(content.0),
                numeric_height(node).unwrap_or(content.1),
            )
        }),
    };
    Bounds {
        x: node_x(node).unwrap_or(0.0),
        y: node_y(node).unwrap_or(0.0),
        w,
        h,
    }
    .rotated(rotation(node))
}

/// Sort `items` by their boxes into reading order: rows top to bottom
/// (a box joins the row whose first box's centre is within
/// `ROW_TOLERANCE_PX` or half the shorter height of its own), then left to
/// right within a row. Equal boxes keep their order.
pub(crate) fn sort_reading<T>(items: &mut Vec<(Bounds, T)>) {
    items.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
    let mut rows: Vec<Vec<(Bounds, T)>> = Vec::new();
    for item in items.drain(..) {
        let joins = rows
            .last()
            .and_then(|row| row.first())
            .is_some_and(|first| {
                let tolerance = ROW_TOLERANCE_PX.max(first.0.h.min(item.0.h) / 2.0);
                (first.0.centre_y() - item.0.centre_y()).abs() <= tolerance
            });
        if joins {
            rows.last_mut().expect("row").push(item);
        } else {
            rows.push(vec![item]);
        }
    }
    for mut row in rows {
        row.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        items.extend(row);
    }
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
