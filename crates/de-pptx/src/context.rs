//! Per-slide conversion state: colours, the layout and master the slide
//! inherits from, placeholder lookup, coordinate mapping and node ids.

use roxmltree::Node;
use serde_json::{json, Map, Value};

use crate::color::ColorContext;
use crate::media::Media;
use crate::theme::Theme;
use crate::warnings::Warnings;
use crate::xml;

/// Pixels per EMU: decks are authored at 96 px per inch like the export.
pub const PX_PER_EMU: f64 = 1.0 / 9525.0;
/// Pixels per typographic point (96 px and 72 pt per inch).
pub const PX_PER_PT: f64 = 96.0 / 72.0;

pub struct Ctx<'a, 'i, 'p> {
    pub colors: ColorContext<'a>,
    pub theme: &'a Theme,
    pub media: &'a mut Media<'p>,
    pub warnings: &'a mut Warnings,
    pub ids: IdGen,
    /// `p:sldLayout` / `p:sldMaster` roots.
    pub layout: Option<Node<'a, 'i>>,
    pub master: Option<Node<'a, 'i>>,
    /// The presentation's `defaultTextStyle`.
    pub default_text: Option<Node<'a, 'i>>,
}

/// Node ids are `s<slide>` for a board and `s<slide>-<n>` inside it, so
/// two decks imported into one document collide only by slide number,
/// which the MCP insert renames.
pub struct IdGen {
    prefix: String,
    next: u32,
}

impl IdGen {
    pub fn new(prefix: String) -> Self {
        Self { prefix, next: 0 }
    }

    pub fn next(&mut self) -> String {
        self.next += 1;
        format!("{}-{}", self.prefix, self.next)
    }
}

/// A placeholder reference (`<p:ph type="title" idx="1"/>`).
#[derive(Debug, Clone)]
pub struct Placeholder {
    pub kind: String,
    pub idx: Option<String>,
}

impl Placeholder {
    /// The `ph` element of a shape, picture or frame, if it is one.
    pub fn of(shape: Node<'_, '_>) -> Option<Self> {
        let nv = xml::elements(shape).find(|n| n.tag_name().name().starts_with("nv"))?;
        let ph = xml::path(nv, &["nvPr", "ph"])?;
        Some(Self {
            kind: ph.attribute("type").unwrap_or("obj").to_string(),
            idx: ph.attribute("idx").map(str::to_string),
        })
    }

    /// The master-level family a type inherits from.
    fn family(&self) -> &str {
        match self.kind.as_str() {
            "title" | "ctrTitle" => "title",
            "dt" | "ftr" | "sldNum" | "hdr" => self.kind.as_str(),
            _ => "body",
        }
    }

    /// Which `p:txStyles` list the master applies to this placeholder.
    pub fn master_style(&self) -> &'static str {
        match self.family() {
            "title" => "titleStyle",
            "body" => "bodyStyle",
            _ => "otherStyle",
        }
    }
}

impl<'a, 'i, 'p> Ctx<'a, 'i, 'p> {
    pub fn layout_placeholder(&self, ph: &Placeholder) -> Option<Node<'a, 'i>> {
        find_placeholder(self.layout?, ph, true)
    }

    pub fn master_placeholder(&self, ph: &Placeholder) -> Option<Node<'a, 'i>> {
        find_placeholder(self.master?, ph, false)
    }

    /// The master's `p:txStyles/<list>`.
    pub fn master_text_style(&self, list: &str) -> Option<Node<'a, 'i>> {
        xml::path(self.master?, &["txStyles", list])
    }
}

/// Find the placeholder `ph` inherits from in a layout or master tree.
/// Layouts match by `idx` first (that is how PowerPoint links a slide's
/// body to the right content box), then by type; masters by type family.
fn find_placeholder<'a, 'i>(
    root: Node<'a, 'i>,
    ph: &Placeholder,
    by_idx: bool,
) -> Option<Node<'a, 'i>> {
    let tree = xml::path(root, &["cSld", "spTree"])?;
    let candidates: Vec<(Node<'a, 'i>, Placeholder)> = tree
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "sp")
        .filter_map(|n| Placeholder::of(n).map(|p| (n, p)))
        .collect();
    if by_idx {
        if let Some(idx) = &ph.idx {
            if let Some((node, _)) = candidates.iter().find(|(_, p)| p.idx.as_ref() == Some(idx)) {
                return Some(*node);
            }
        }
        if let Some((node, _)) = candidates.iter().find(|(_, p)| p.kind == ph.kind) {
            return Some(*node);
        }
    }
    candidates
        .iter()
        .find(|(_, p)| p.family() == ph.family())
        .map(|(node, _)| *node)
}

/// Maps a container's child EMU coordinates to its local pixels:
/// `px = (emu - offset) * scale`. A group re-bases its children through
/// `chOff` / `chExt`.
#[derive(Debug, Clone, Copy)]
pub struct Mapping {
    pub off: (f64, f64),
    pub scale: (f64, f64),
}

impl Mapping {
    pub const SLIDE: Mapping = Mapping {
        off: (0.0, 0.0),
        scale: (PX_PER_EMU, PX_PER_EMU),
    };
}

/// An `a:xfrm` / `p:xfrm`.
#[derive(Debug, Clone, Copy)]
pub struct Xfrm {
    pub off: (f64, f64),
    pub ext: (f64, f64),
    pub rot: f64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub child_off: Option<(f64, f64)>,
    pub child_ext: Option<(f64, f64)>,
}

/// A box in the parent's local pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PxBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Xfrm {
    pub fn read(el: Node<'_, '_>) -> Option<Self> {
        let pair = |name: &str, a: &str, b: &str| {
            let n = xml::child(el, name)?;
            Some((xml::attr_f64(n, a)?, xml::attr_f64(n, b)?))
        };
        Some(Self {
            off: pair("off", "x", "y").unwrap_or((0.0, 0.0)),
            ext: pair("ext", "cx", "cy")?,
            rot: xml::attr_f64(el, "rot").unwrap_or(0.0) / 60_000.0,
            flip_h: xml::attr_bool(el, "flipH").unwrap_or(false),
            flip_v: xml::attr_bool(el, "flipV").unwrap_or(false),
            child_off: pair("chOff", "x", "y"),
            child_ext: pair("chExt", "cx", "cy"),
        })
    }

    pub fn px(&self, map: Mapping) -> PxBox {
        PxBox {
            x: (self.off.0 - map.off.0) * map.scale.0,
            y: (self.off.1 - map.off.1) * map.scale.1,
            w: self.ext.0 * map.scale.0,
            h: self.ext.1 * map.scale.1,
        }
    }

    /// The mapping for this group's children, in the group's own pixels.
    pub fn child_mapping(&self, parent: Mapping) -> Mapping {
        let child_off = self.child_off.unwrap_or(self.off);
        let child_ext = self.child_ext.unwrap_or(self.ext);
        let ratio = |ext: f64, ch: f64| if ch > 0.0 { ext / ch } else { 1.0 };
        Mapping {
            off: child_off,
            scale: (
                parent.scale.0 * ratio(self.ext.0, child_ext.0),
                parent.scale.1 * ratio(self.ext.1, child_ext.1),
            ),
        }
    }
}

/// The base fields every node carries: id, name, box, rotation, flips,
/// visibility. `flip` is false for a wrapper whose child takes the flips.
pub fn base(
    id: String,
    shape: Node<'_, '_>,
    b: PxBox,
    xfrm: Option<&Xfrm>,
    flip: bool,
) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("id".into(), json!(id));
    let c_nv_pr = shape
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "cNvPr");
    if let Some(name) = c_nv_pr
        .and_then(|n| n.attribute("name"))
        .filter(|n| !n.is_empty())
    {
        m.insert("name".into(), json!(name));
    }
    if c_nv_pr.and_then(|n| xml::attr_bool(n, "hidden")) == Some(true) {
        m.insert("visible".into(), json!(false));
    }
    m.insert("x".into(), json!(round(b.x)));
    m.insert("y".into(), json!(round(b.y)));
    if let Some(xfrm) = xfrm {
        if xfrm.rot.abs() > 1e-6 {
            m.insert("rotation".into(), json!(round(xfrm.rot)));
        }
        if flip && xfrm.flip_h {
            m.insert("flipX".into(), json!(true));
        }
        if flip && xfrm.flip_v {
            m.insert("flipY".into(), json!(true));
        }
    }
    m
}

/// Two decimals: EMU are integers, so finer digits are float noise.
pub fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
