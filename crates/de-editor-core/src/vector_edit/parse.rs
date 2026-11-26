//! SVG path data (`d`) → editable [`Subpath`]s.
//!
//! Every command of the SVG grammar is read, absolute and relative:
//! `M L H V C S Q T A Z`. Quadratics are degree-elevated to cubics
//! (exact), arcs go through [`super::arc_to_cubics`] (error bounded),
//! so the result is a pure cubic / line anchor list per subpath.
//!
//! The tokenizer follows the grammar rather than splitting on
//! separators: numbers may run together (`1-2`, `.5.5`), and an arc's
//! two flags may be packed without separators (`a1 1 0 011 1`).
//!
//! An explicit closing segment that lands back on the start point is
//! folded into the closing `Z` (its incoming handle moves onto the first
//! anchor), so `anchors → d → anchors` round-trips anchor for anchor.

use super::{arc::arc_to_cubics, corner, Pt, Subpath};
use jian_ops_schema::node::{PenPathAnchor, PenPathHandle};

/// Distance under which a closing segment's end counts as the start.
const CLOSE_EPS: f64 = 1e-6;
/// Handle deltas shorter than this are stored as "no handle".
const HANDLE_EPS: f64 = 1e-9;

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn skip_separators(&mut self) {
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_whitespace() || self.bytes[self.pos] == b',')
        {
            self.pos += 1;
        }
    }

    /// Next command letter, if the next token is one.
    fn command(&mut self) -> Option<u8> {
        self.skip_separators();
        let c = *self.bytes.get(self.pos)?;
        if c.is_ascii_alphabetic() && c != b'e' && c != b'E' {
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }

    /// True when a number (not a command) follows.
    fn number_follows(&mut self) -> bool {
        self.skip_separators();
        matches!(self.bytes.get(self.pos), Some(c) if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.'))
    }

    fn number(&mut self) -> Option<f64> {
        self.skip_separators();
        let start = self.pos;
        let b = self.bytes;
        let mut i = self.pos;
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            i += 1;
        }
        let mut digits = false;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits = true;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
                digits = true;
            }
        }
        if !digits {
            return None;
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
                j += 1;
            }
            if j < b.len() && b[j].is_ascii_digit() {
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
            }
        }
        self.pos = i;
        std::str::from_utf8(&b[start..i]).ok()?.parse().ok()
    }

    /// An arc flag: a single `0` / `1`, separators optional.
    fn flag(&mut self) -> Option<bool> {
        self.skip_separators();
        let c = *self.bytes.get(self.pos)?;
        self.pos += 1;
        match c {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }
}

fn handle(from: Pt, to: Pt) -> Option<PenPathHandle> {
    let (x, y) = (to.0 - from.0, to.1 - from.1);
    (x.abs() > HANDLE_EPS || y.abs() > HANDLE_EPS).then_some(PenPathHandle { x, y })
}

/// Builder for one subpath while commands stream in.
struct Builder {
    out: Vec<Subpath>,
    current: Option<Subpath>,
}

impl Builder {
    fn start(&mut self, p: Pt) {
        self.finish();
        self.current = Some(Subpath {
            anchors: vec![corner(p)],
            closed: false,
        });
    }

    fn line(&mut self, p: Pt) {
        if let Some(sub) = self.current.as_mut() {
            sub.anchors.push(corner(p));
        }
    }

    fn cubic(&mut self, c1: Pt, c2: Pt, p: Pt) {
        let Some(sub) = self.current.as_mut() else {
            return;
        };
        if let Some(prev) = sub.anchors.last_mut() {
            prev.handle_out = handle((prev.x, prev.y), c1);
        }
        sub.anchors.push(PenPathAnchor {
            x: p.0,
            y: p.1,
            handle_in: handle(p, c2),
            handle_out: None,
            point_type: None,
        });
    }

    fn close(&mut self) {
        let Some(sub) = self.current.as_mut() else {
            return;
        };
        sub.closed = true;
        let n = sub.anchors.len();
        if n > 1 {
            let (first, last) = (&sub.anchors[0], &sub.anchors[n - 1]);
            if (first.x - last.x).abs() < CLOSE_EPS && (first.y - last.y).abs() < CLOSE_EPS {
                let handle_in = sub.anchors[n - 1].handle_in.clone();
                sub.anchors.pop();
                sub.anchors[0].handle_in = handle_in;
            }
        }
    }

    fn finish(&mut self) {
        if let Some(sub) = self.current.take() {
            if !sub.anchors.is_empty() {
                self.out.push(sub);
            }
        }
    }
}

/// Parse `d` into subpaths (native coordinates of the string). `None`
/// when the data is malformed before any drawable segment was read or
/// has no subpath at all; trailing garbage after valid data is ignored,
/// matching how SVG renderers draw a path up to its first error.
pub fn parse_path_data(d: &str) -> Option<Vec<Subpath>> {
    let mut cur = Cursor {
        bytes: d.as_bytes(),
        pos: 0,
    };
    let mut b = Builder {
        out: Vec::new(),
        current: None,
    };
    let mut cmd = 0u8;
    let mut pen: Pt = (0.0, 0.0);
    let mut start: Pt = (0.0, 0.0);
    // Previous control points for S / T reflection.
    let mut last_cubic: Option<Pt> = None;
    let mut last_quad: Option<Pt> = None;
    loop {
        if let Some(c) = cur.command() {
            cmd = c;
        } else if !cur.number_follows() || cmd == 0 {
            // End of data, or numbers with no command to repeat.
            break;
        }
        let rel = cmd.is_ascii_lowercase();
        let abs = |p: Pt, pen: Pt| if rel { (pen.0 + p.0, pen.1 + p.1) } else { p };
        let ok = match cmd.to_ascii_uppercase() {
            b'Z' => {
                b.close();
                pen = start;
                last_cubic = None;
                last_quad = None;
                // A command after Z without M restarts at the start point.
                b.finish();
                b.current = None;
                cmd = 0;
                true
            }
            b'M' => (|| {
                let p = abs((cur.number()?, cur.number()?), pen);
                b.start(p);
                pen = p;
                start = p;
                cmd = if rel { b'l' } else { b'L' };
                Some(())
            })()
            .is_some(),
            b'L' | b'H' | b'V' | b'C' | b'S' | b'Q' | b'T' | b'A' => {
                if b.current.is_none() {
                    b.start(pen);
                    start = pen;
                }
                draw(
                    &mut cur,
                    &mut b,
                    cmd,
                    &mut pen,
                    &mut last_cubic,
                    &mut last_quad,
                )
                .is_some()
            }
            _ => false,
        };
        if !ok {
            break;
        }
    }
    b.finish();
    (!b.out.is_empty()).then_some(b.out)
}

/// One drawing command's arguments → anchors. Updates the pen and the
/// reflection memory.
fn draw(
    cur: &mut Cursor<'_>,
    b: &mut Builder,
    cmd: u8,
    pen: &mut Pt,
    last_cubic: &mut Option<Pt>,
    last_quad: &mut Option<Pt>,
) -> Option<()> {
    let rel = cmd.is_ascii_lowercase();
    let p0 = *pen;
    let abs = |p: Pt| if rel { (p0.0 + p.0, p0.1 + p.1) } else { p };
    let mut next_cubic = None;
    let mut next_quad = None;
    let end = match cmd.to_ascii_uppercase() {
        b'L' => {
            let p = abs((cur.number()?, cur.number()?));
            b.line(p);
            p
        }
        b'H' => {
            let x = cur.number()?;
            let p = (if rel { p0.0 + x } else { x }, p0.1);
            b.line(p);
            p
        }
        b'V' => {
            let y = cur.number()?;
            let p = (p0.0, if rel { p0.1 + y } else { y });
            b.line(p);
            p
        }
        b'C' => {
            let c1 = abs((cur.number()?, cur.number()?));
            let c2 = abs((cur.number()?, cur.number()?));
            let p = abs((cur.number()?, cur.number()?));
            b.cubic(c1, c2, p);
            next_cubic = Some(c2);
            p
        }
        b'S' => {
            let c1 = last_cubic.map_or(p0, |c| (2.0 * p0.0 - c.0, 2.0 * p0.1 - c.1));
            let c2 = abs((cur.number()?, cur.number()?));
            let p = abs((cur.number()?, cur.number()?));
            b.cubic(c1, c2, p);
            next_cubic = Some(c2);
            p
        }
        b'Q' | b'T' => {
            let q = if cmd.eq_ignore_ascii_case(&b'Q') {
                abs((cur.number()?, cur.number()?))
            } else {
                last_quad.map_or(p0, |c| (2.0 * p0.0 - c.0, 2.0 * p0.1 - c.1))
            };
            let p = abs((cur.number()?, cur.number()?));
            // Degree elevation: exact.
            let c1 = (
                p0.0 + 2.0 / 3.0 * (q.0 - p0.0),
                p0.1 + 2.0 / 3.0 * (q.1 - p0.1),
            );
            let c2 = (p.0 + 2.0 / 3.0 * (q.0 - p.0), p.1 + 2.0 / 3.0 * (q.1 - p.1));
            b.cubic(c1, c2, p);
            next_quad = Some(q);
            p
        }
        b'A' => {
            let rx = cur.number()?;
            let ry = cur.number()?;
            let rot = cur.number()?;
            let large = cur.flag()?;
            let sweep = cur.flag()?;
            let p = abs((cur.number()?, cur.number()?));
            for [c1, c2, e] in arc_to_cubics(p0, rx, ry, rot, large, sweep, p) {
                b.cubic(c1, c2, e);
            }
            p
        }
        _ => return None,
    };
    *pen = end;
    *last_cubic = next_cubic;
    *last_quad = next_quad;
    Some(())
}
