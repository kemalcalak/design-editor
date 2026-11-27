//! Drawing a new shape with a drag (#389): Shift keeps a box square (a
//! circle for an ellipse) and a line on a 45° step, Alt / Option draws
//! from the press point outward, and both combine. Pure doc-px geometry;
//! the host feeds it the press point, the cursor and the modifiers.

use crate::vector_edit::constrain_45;
use crate::Tool;

/// The modifiers held while a shape is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrawModifiers {
    /// Shift: square / circle boxes, 45° lines.
    pub shift: bool,
    /// Alt / Option: the press point is the centre.
    pub alt: bool,
}

/// What a drag draws: a box from corner to corner, or a line from its
/// start to its end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawnShape {
    /// A box in doc px; `w` / `h` are never negative.
    Box { x: f64, y: f64, w: f64, h: f64 },
    /// A line from `(x, y)`; `(dx, dy)` is the end minus the start, the
    /// line node's own `x2` / `y2`.
    Line { x: f64, y: f64, dx: f64, dy: f64 },
}

/// How a tool's drag is shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawKind {
    /// A box Shift keeps square.
    Proportional,
    /// A box Shift leaves free (text boxes, widgets).
    Free,
    /// A line Shift turns in 45° steps.
    Line,
}

impl DrawKind {
    /// The draw kind of a tool that creates a node by dragging.
    pub fn for_tool(tool: Tool) -> Self {
        match tool {
            Tool::Line => DrawKind::Line,
            Tool::Rect | Tool::Ellipse | Tool::Polygon | Tool::Star | Tool::Frame => {
                DrawKind::Proportional
            }
            _ => DrawKind::Free,
        }
    }
}

/// The shape a drag from `start` to `cursor` draws (doc px).
pub fn drawn_shape(
    kind: DrawKind,
    start: (f64, f64),
    cursor: (f64, f64),
    mods: DrawModifiers,
) -> DrawnShape {
    if kind == DrawKind::Line {
        let end = if mods.shift {
            constrain_45(start, cursor)
        } else {
            cursor
        };
        let (dx, dy) = (end.0 - start.0, end.1 - start.1);
        return if mods.alt {
            DrawnShape::Line {
                x: start.0 - dx,
                y: start.1 - dy,
                dx: 2.0 * dx,
                dy: 2.0 * dy,
            }
        } else {
            DrawnShape::Line {
                x: start.0,
                y: start.1,
                dx,
                dy,
            }
        };
    }
    let (mut dx, mut dy) = (cursor.0 - start.0, cursor.1 - start.1);
    if mods.shift && kind == DrawKind::Proportional {
        let side = dx.abs().max(dy.abs());
        dx = side.copysign(dx);
        dy = side.copysign(dy);
    }
    if mods.alt {
        DrawnShape::Box {
            x: start.0 - dx.abs(),
            y: start.1 - dy.abs(),
            w: 2.0 * dx.abs(),
            h: 2.0 * dy.abs(),
        }
    } else {
        DrawnShape::Box {
            x: start.0.min(start.0 + dx),
            y: start.1.min(start.1 + dy),
            w: dx.abs(),
            h: dy.abs(),
        }
    }
}

/// A drawn line on whole document pixels. The caller snaps the drag's
/// start before `drawn_shape`; this rounds the end and, under Shift,
/// keeps a diagonal exactly 45° (sin and cos of a quarter turn differ in
/// the last bit, so rounding alone could leave the two legs a pixel apart).
pub fn snap_line(x: f64, y: f64, dx: f64, dy: f64, shift: bool) -> (f64, f64, f64, f64) {
    let (x0, y0) = (x.round(), y.round());
    let (mut dx, mut dy) = ((x + dx).round() - x0, (y + dy).round() - y0);
    if shift && dx != 0.0 && dy != 0.0 {
        let leg = dx.abs().max(dy.abs());
        dx = leg.copysign(dx);
        dy = leg.copysign(dy);
    }
    (x0, y0, dx, dy)
}

#[cfg(test)]
#[path = "draw_constraints_tests.rs"]
mod tests;
