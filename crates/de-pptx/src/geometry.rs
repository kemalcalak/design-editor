//! Shape geometry: preset shapes (`a:prstGeom`) and custom paths
//! (`a:custGeom`) as editor primitives or SVG path data.
//!
//! A path node paints its `d` fitted to its own box (the path's tight
//! bounds are scaled onto `width` × `height`), so a path is returned with
//! the bounds it actually covers inside the shape box; the caller places
//! the node there. Presets drawn as paths always span the whole box.

use roxmltree::Node;

use crate::context::PX_PER_EMU;
use crate::xml;

pub enum Geometry {
    Rect {
        radius: f64,
    },
    Ellipse,
    /// A straight segment across the box (direction from the flips).
    Line,
    Path {
        d: String,
        bounds: Bounds,
    },
    /// A preset with no mapping: drawn as its box, reported by name.
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Read the geometry of an `spPr`. No geometry element means a rectangle
/// (text boxes carry `prstGeom rect`, but placeholders often omit it).
pub fn read(sp_pr: Option<Node<'_, '_>>, w: f64, h: f64) -> Geometry {
    let Some(sp_pr) = sp_pr else {
        return Geometry::Rect { radius: 0.0 };
    };
    if let Some(custom) = xml::child(sp_pr, "custGeom") {
        return custom_path(custom, w, h).unwrap_or(Geometry::Rect { radius: 0.0 });
    }
    let Some(preset) = xml::child(sp_pr, "prstGeom") else {
        return Geometry::Rect { radius: 0.0 };
    };
    let name = preset.attribute("prst").unwrap_or("rect");
    let adj = |index: usize, default: f64| adjust(preset, index, default) / 100_000.0;
    let short = w.min(h);
    let full = Bounds {
        x: 0.0,
        y: 0.0,
        w,
        h,
    };
    let path = |points: &[(f64, f64)]| Geometry::Path {
        d: polygon(points),
        bounds: full,
    };
    match name {
        "rect" | "flowChartProcess" => Geometry::Rect { radius: 0.0 },
        "roundRect" | "flowChartAlternateProcess" => Geometry::Rect {
            radius: short * adj(1, 16_667.0),
        },
        "ellipse" | "flowChartConnector" => Geometry::Ellipse,
        "line" | "straightConnector1" | "bentConnector2" | "bentConnector3" | "bentConnector4"
        | "bentConnector5" | "curvedConnector2" | "curvedConnector3" | "curvedConnector4"
        | "curvedConnector5" => Geometry::Line,
        "triangle" | "flowChartExtract" => {
            let apex = w * adj(1, 50_000.0);
            path(&[(apex, 0.0), (w, h), (0.0, h)])
        }
        "rtTriangle" => path(&[(0.0, 0.0), (w, h), (0.0, h)]),
        "diamond" | "flowChartDecision" => {
            path(&[(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)])
        }
        "parallelogram" | "flowChartInputOutput" => {
            let dx = short * adj(1, 25_000.0);
            path(&[(dx, 0.0), (w, 0.0), (w - dx, h), (0.0, h)])
        }
        "trapezoid" => {
            let dx = short * adj(1, 25_000.0);
            path(&[(dx, 0.0), (w - dx, 0.0), (w, h), (0.0, h)])
        }
        "hexagon" => {
            let dx = short * adj(1, 25_000.0);
            path(&[
                (dx, 0.0),
                (w - dx, 0.0),
                (w, h / 2.0),
                (w - dx, h),
                (dx, h),
                (0.0, h / 2.0),
            ])
        }
        "octagon" => {
            let d = short * adj(1, 29_289.0);
            path(&[
                (d, 0.0),
                (w - d, 0.0),
                (w, d),
                (w, h - d),
                (w - d, h),
                (d, h),
                (0.0, h - d),
                (0.0, d),
            ])
        }
        "pentagon" => path(&[
            (w / 2.0, 0.0),
            (w, h * 0.382),
            (w * 0.809, h),
            (w * 0.191, h),
            (0.0, h * 0.382),
        ]),
        "homePlate" => {
            let tip = w - short * adj(1, 50_000.0);
            path(&[(0.0, 0.0), (tip, 0.0), (w, h / 2.0), (tip, h), (0.0, h)])
        }
        "chevron" => {
            let d = short * adj(1, 50_000.0);
            path(&[
                (0.0, 0.0),
                (w - d, 0.0),
                (w, h / 2.0),
                (w - d, h),
                (0.0, h),
                (d, h / 2.0),
            ])
        }
        "rightArrow" | "leftArrow" | "upArrow" | "downArrow" => {
            arrow(name, w, h, adj(1, 50_000.0), adj(2, 50_000.0))
        }
        other => Geometry::Unsupported(other.to_string()),
    }
}

/// An `<a:gd name="adjN" fmla="val 25000"/>` value, or the preset default.
fn adjust(preset: Node<'_, '_>, index: usize, default: f64) -> f64 {
    let wanted = [format!("adj{index}"), "adj".to_string()];
    xml::path(preset, &["avLst"])
        .into_iter()
        .flat_map(|av| xml::children(av, "gd"))
        .filter(|gd| {
            gd.attribute("name")
                .is_some_and(|n| n == wanted[0] || (index == 1 && n == wanted[1]))
        })
        .find_map(|gd| {
            gd.attribute("fmla")?
                .strip_prefix("val ")?
                .trim()
                .parse()
                .ok()
        })
        .unwrap_or(default)
}

/// A block arrow pointing right, rotated into place for the other three.
/// `shaft` is the shaft thickness as a fraction of the cross axis, `head`
/// the head length as a fraction of the short side.
fn arrow(name: &str, w: f64, h: f64, shaft: f64, head: f64) -> Geometry {
    let horizontal = matches!(name, "rightArrow" | "leftArrow");
    let (len, cross) = if horizontal { (w, h) } else { (h, w) };
    let head_len = w.min(h) * head;
    let s0 = cross * (1.0 - shaft) / 2.0;
    let s1 = cross - s0;
    let neck = len - head_len;
    // Right-pointing arrow in (along, across) coordinates.
    let right = [
        (0.0, s0),
        (neck, s0),
        (neck, 0.0),
        (len, cross / 2.0),
        (neck, cross),
        (neck, s1),
        (0.0, s1),
    ];
    let points: Vec<(f64, f64)> = right
        .iter()
        .map(|&(a, c)| match name {
            "rightArrow" => (a, c),
            "leftArrow" => (len - a, c),
            "downArrow" => (c, a),
            _ => (c, len - a),
        })
        .collect();
    Geometry::Path {
        d: polygon(&points),
        bounds: Bounds {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
    }
}

fn polygon(points: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, (x, y)) in points.iter().enumerate() {
        d.push_str(if i == 0 { "M" } else { " L" });
        d.push_str(&format!("{} {}", num(*x), num(*y)));
    }
    d.push_str(" Z");
    d
}

fn num(v: f64) -> String {
    let rounded = (v * 100.0).round() / 100.0;
    if rounded == rounded.trunc() {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

/// Bounds accumulated from sampled points.
struct Extent {
    min: (f64, f64),
    max: (f64, f64),
}

impl Extent {
    fn new() -> Self {
        Self {
            min: (f64::INFINITY, f64::INFINITY),
            max: (f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }

    fn add(&mut self, (x, y): (f64, f64)) {
        self.min = (self.min.0.min(x), self.min.1.min(y));
        self.max = (self.max.0.max(x), self.max.1.max(y));
    }

    fn bounds(&self) -> Option<Bounds> {
        (self.min.0.is_finite() && self.max.0.is_finite()).then_some(Bounds {
            x: self.min.0,
            y: self.min.1,
            w: self.max.0 - self.min.0,
            h: self.max.1 - self.min.1,
        })
    }
}

/// `a:custGeom` → one SVG path over all its sub-paths, in shape-box px.
fn custom_path(custom: Node<'_, '_>, w: f64, h: f64) -> Option<Geometry> {
    let list = xml::child(custom, "pathLst")?;
    let mut d = String::new();
    let mut extent = Extent::new();
    for path in xml::children(list, "path") {
        let sx = xml::attr_f64(path, "w")
            .filter(|v| *v > 0.0)
            .map_or(PX_PER_EMU, |pw| w / pw);
        let sy = xml::attr_f64(path, "h")
            .filter(|v| *v > 0.0)
            .map_or(PX_PER_EMU, |ph| h / ph);
        let point = |pt: Node<'_, '_>| -> (f64, f64) {
            (
                xml::attr_f64(pt, "x").unwrap_or(0.0) * sx,
                xml::attr_f64(pt, "y").unwrap_or(0.0) * sy,
            )
        };
        let mut current = (0.0, 0.0);
        for cmd in xml::elements(path) {
            let pts: Vec<(f64, f64)> = xml::children(cmd, "pt").map(point).collect();
            match cmd.tag_name().name() {
                "moveTo" if !pts.is_empty() => {
                    current = pts[0];
                    extent.add(current);
                    d.push_str(&format!("M{} {} ", num(current.0), num(current.1)));
                }
                "lnTo" if !pts.is_empty() => {
                    current = pts[0];
                    extent.add(current);
                    d.push_str(&format!("L{} {} ", num(current.0), num(current.1)));
                }
                "cubicBezTo" if pts.len() == 3 => {
                    sample_cubic(&mut extent, current, pts[0], pts[1], pts[2]);
                    current = pts[2];
                    d.push_str(&format!(
                        "C{} {} {} {} {} {} ",
                        num(pts[0].0),
                        num(pts[0].1),
                        num(pts[1].0),
                        num(pts[1].1),
                        num(current.0),
                        num(current.1)
                    ));
                }
                "quadBezTo" if pts.len() == 2 => {
                    let (c, end) = (pts[0], pts[1]);
                    for i in 1..=16 {
                        let t = f64::from(i) / 16.0;
                        let u = 1.0 - t;
                        extent.add((
                            u * u * current.0 + 2.0 * u * t * c.0 + t * t * end.0,
                            u * u * current.1 + 2.0 * u * t * c.1 + t * t * end.1,
                        ));
                    }
                    current = end;
                    d.push_str(&format!(
                        "Q{} {} {} {} ",
                        num(c.0),
                        num(c.1),
                        num(end.0),
                        num(end.1)
                    ));
                }
                "arcTo" => {
                    let rx = xml::attr_f64(cmd, "wR").unwrap_or(0.0) * sx;
                    let ry = xml::attr_f64(cmd, "hR").unwrap_or(0.0) * sy;
                    let start =
                        (xml::attr_f64(cmd, "stAng").unwrap_or(0.0) / 60_000.0).to_radians();
                    let sweep =
                        (xml::attr_f64(cmd, "swAng").unwrap_or(0.0) / 60_000.0).to_radians();
                    let centre = (current.0 - rx * start.cos(), current.1 - ry * start.sin());
                    for i in 1..=24 {
                        let a = start + sweep * f64::from(i) / 24.0;
                        extent.add((centre.0 + rx * a.cos(), centre.1 + ry * a.sin()));
                    }
                    // Two half arcs: an SVG arc whose end meets its start
                    // (a full turn) draws nothing.
                    for half in 1..=2 {
                        let angle = start + sweep * f64::from(half) / 2.0;
                        current = (centre.0 + rx * angle.cos(), centre.1 + ry * angle.sin());
                        d.push_str(&format!(
                            "A{} {} 0 0 {} {} {} ",
                            num(rx),
                            num(ry),
                            u8::from(sweep > 0.0),
                            num(current.0),
                            num(current.1)
                        ));
                    }
                }
                "close" => d.push_str("Z "),
                _ => {}
            }
        }
    }
    let bounds = extent.bounds()?;
    Some(Geometry::Path {
        d: d.trim_end().to_string(),
        bounds,
    })
}

fn sample_cubic(
    extent: &mut Extent,
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
) {
    for i in 1..=16 {
        let t = f64::from(i) / 16.0;
        let u = 1.0 - t;
        let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        extent.add((
            a * p0.0 + b * p1.0 + c * p2.0 + e * p3.0,
            a * p0.1 + b * p1.1 + c * p2.1 + e * p3.1,
        ));
    }
}
