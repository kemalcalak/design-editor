//! DrawingML colours: the six colour models plus the modifier chain
//! (`lumMod`, `lumOff`, `tint`, `shade`, `alpha`, …).
//!
//! Scheme colours resolve through the slide's colour map onto the theme.
//! `phClr` only appears inside the theme's style lists, which the
//! importer reads through their references' own colours instead.

use std::collections::HashMap;

use roxmltree::Node;

use crate::xml;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba::opaque(0.0, 0.0, 0.0);
    pub const WHITE: Rgba = Rgba::opaque(1.0, 1.0, 1.0);

    pub const fn opaque(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// A DrawingML `val` (`RRGGBB`, no `#`).
    pub fn from_hex(hex: &str) -> Option<Self> {
        let opts = de_util::hex_color::HexOptions {
            require_hash: false,
            allow_rgb_shorthand: false,
            allow_rgba_shorthand: false,
            allow_alpha: false,
        };
        let [r, g, b, _] = de_util::hex_color::parse_hex_rgba8(hex.trim(), opts)?;
        Some(Self::opaque(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
        ))
    }

    /// `#RRGGBB`. Opacity travels separately (fill `opacity`) where the
    /// schema has a slot for it.
    pub fn hex(&self) -> String {
        let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02X}{:02X}{:02X}",
            byte(self.r),
            byte(self.g),
            byte(self.b)
        )
    }

    /// `#RRGGBB`, or `#RRGGBBAA` when translucent — for the text segment
    /// `fill`, which is a bare colour string.
    pub fn hex_with_alpha(&self) -> String {
        if self.a >= 0.999 {
            return self.hex();
        }
        let alpha = (self.a.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("{}{alpha:02X}", self.hex())
    }
}

/// Everything a colour element may need to resolve.
#[derive(Clone, Copy)]
pub struct ColorContext<'a> {
    pub scheme: &'a HashMap<String, Rgba>,
    pub map: &'a HashMap<String, String>,
}

impl<'a> ColorContext<'a> {
    /// A scheme slot name (`tx1`, `accent2`, `bg1`) through the colour map.
    pub fn scheme_color(&self, name: &str) -> Option<Rgba> {
        let slot = self.map.get(name).map(String::as_str).unwrap_or(name);
        self.scheme.get(slot).copied()
    }

    /// The first colour element under `parent` (`<a:solidFill>`, `<a:gs>`,
    /// `<a:fontRef>`, …).
    pub fn color_in(&self, parent: Node<'_, '_>) -> Option<Rgba> {
        xml::elements(parent).find_map(|el| self.color(el))
    }

    /// Resolve one colour element, modifiers included. `None` for a node
    /// that is not a colour.
    pub fn color(&self, el: Node<'_, '_>) -> Option<Rgba> {
        let base = match el.tag_name().name() {
            "srgbClr" => Rgba::from_hex(el.attribute("val")?)?,
            "sysClr" => el
                .attribute("lastClr")
                .and_then(Rgba::from_hex)
                .or_else(|| system_color(el.attribute("val")?))?,
            "schemeClr" => self.scheme_color(el.attribute("val")?)?,
            "prstClr" => preset_color(el.attribute("val")?)?,
            "scrgbClr" => Rgba::opaque(
                pct(el, "r")?.clamp(0.0, 1.0).powf(1.0 / 2.2),
                pct(el, "g")?.clamp(0.0, 1.0).powf(1.0 / 2.2),
                pct(el, "b")?.clamp(0.0, 1.0).powf(1.0 / 2.2),
            ),
            "hslClr" => {
                // 60000ths of a degree; `hsl_to_rgb` takes sixths of a turn.
                let hue = xml::attr_f64(el, "hue")? / 3_600_000.0;
                hsl_to_rgb(hue, pct(el, "sat")?, pct(el, "lum")?)
            }
            _ => return None,
        };
        Some(apply_modifiers(base, el))
    }
}

/// A `val` in thousandths of a percent (`50000` = 50%) as a fraction.
fn pct(el: Node<'_, '_>, name: &str) -> Option<f64> {
    Some(xml::attr_f64(el, name)? / 100_000.0)
}

fn apply_modifiers(mut color: Rgba, el: Node<'_, '_>) -> Rgba {
    for modifier in xml::elements(el) {
        // `inv` (and `gray`) carry no value.
        match modifier.tag_name().name() {
            "inv" => {
                color.r = 1.0 - color.r;
                color.g = 1.0 - color.g;
                color.b = 1.0 - color.b;
                continue;
            }
            "gray" => {
                let luma = 0.3 * color.r + 0.59 * color.g + 0.11 * color.b;
                color = Rgba {
                    r: luma,
                    g: luma,
                    b: luma,
                    a: color.a,
                };
                continue;
            }
            _ => {}
        }
        let Some(v) = pct(modifier, "val") else {
            continue;
        };
        match modifier.tag_name().name() {
            "alpha" => color.a = v.clamp(0.0, 1.0),
            "alphaMod" => color.a = (color.a * v).clamp(0.0, 1.0),
            "alphaOff" => color.a = (color.a + v).clamp(0.0, 1.0),
            "tint" => {
                color.r = 1.0 - (1.0 - color.r) * v;
                color.g = 1.0 - (1.0 - color.g) * v;
                color.b = 1.0 - (1.0 - color.b) * v;
            }
            "shade" => {
                color.r *= v;
                color.g *= v;
                color.b *= v;
            }
            "lumMod" | "lumOff" | "satMod" | "satOff" => {
                let (h, mut s, mut l) = rgb_to_hsl(color);
                match modifier.tag_name().name() {
                    "lumMod" => l *= v,
                    "lumOff" => l += v,
                    "satMod" => s *= v,
                    _ => s += v,
                }
                let alpha = color.a;
                color = hsl_to_rgb(h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                color.a = alpha;
            }
            _ => {}
        }
    }
    color
}

/// Hue in sixths of a turn (0..6), saturation and lightness 0..1.
fn rgb_to_hsl(c: Rgba) -> (f64, f64, f64) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f64::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == c.r {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgba {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h = h.rem_euclid(6.0);
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Rgba::opaque(r + m, g + m, b + m)
}

fn system_color(name: &str) -> Option<Rgba> {
    match name {
        "windowText" | "menuText" | "captionText" | "btnText" => Some(Rgba::BLACK),
        "window" | "menu" | "btnHighlight" | "highlightText" => Some(Rgba::WHITE),
        _ => None,
    }
}

/// The preset names decks actually use; the rest of the 140 CSS names
/// are rare enough that black is a fine stand-in.
fn preset_color(name: &str) -> Option<Rgba> {
    let hex = match name {
        "black" => "000000",
        "white" => "FFFFFF",
        "red" => "FF0000",
        "green" => "008000",
        "blue" => "0000FF",
        "yellow" => "FFFF00",
        "gray" | "grey" => "808080",
        "darkGray" | "dkGray" => "A9A9A9",
        "lightGray" | "ltGray" => "D3D3D3",
        "orange" => "FFA500",
        "purple" => "800080",
        "navy" => "000080",
        "teal" => "008080",
        _ => "000000",
    };
    Rgba::from_hex(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(fragment: &str) -> Rgba {
        let src = format!(
            "<a:solidFill xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">{fragment}</a:solidFill>"
        );
        let doc = roxmltree::Document::parse(&src).expect("xml");
        let scheme = HashMap::from([
            ("dk1".to_string(), Rgba::BLACK),
            (
                "accent1".to_string(),
                Rgba::from_hex("4472C4").expect("hex"),
            ),
        ]);
        let map = HashMap::from([("tx1".to_string(), "dk1".to_string())]);
        let ctx = ColorContext {
            scheme: &scheme,
            map: &map,
        };
        ctx.color_in(doc.root_element()).expect("a colour")
    }

    #[test]
    fn scheme_colours_go_through_the_colour_map() {
        assert_eq!(resolve("<a:schemeClr val=\"tx1\"/>").hex(), "#000000");
        assert_eq!(resolve("<a:schemeClr val=\"accent1\"/>").hex(), "#4472C4");
    }

    #[test]
    fn luminance_modifiers_lighten_like_powerpoint() {
        // "Accent 1, lighter 40%": lumMod 60% + lumOff 40%.
        let c = resolve(
            "<a:schemeClr val=\"accent1\"><a:lumMod val=\"60000\"/><a:lumOff val=\"40000\"/></a:schemeClr>",
        );
        assert_eq!(c.hex(), "#8FAADC");
    }

    #[test]
    fn hsl_hue_and_inversion() {
        assert_eq!(
            resolve("<a:hslClr hue=\"14400000\" sat=\"100000\" lum=\"50000\"/>").hex(),
            "#0000FF"
        );
        assert_eq!(
            resolve("<a:srgbClr val=\"000000\"><a:inv/></a:srgbClr>").hex(),
            "#FFFFFF"
        );
    }

    #[test]
    fn alpha_rides_along() {
        let c = resolve("<a:srgbClr val=\"FF0000\"><a:alpha val=\"50000\"/></a:srgbClr>");
        assert_eq!(c.hex_with_alpha(), "#FF000080");
    }
}
