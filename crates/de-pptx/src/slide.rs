//! One slide → one board: its layout and master behind it, its
//! background, its shapes, and its speaker notes.

use std::collections::HashMap;

use roxmltree::Node;
use serde_json::{json, Value};

use crate::color::{ColorContext, Rgba};
use crate::context::{round, Ctx, IdGen, Mapping};
use crate::error::PptxError;
use crate::media::Media;
use crate::motion;
use crate::package::{first_of_kind, Package, Relationships};
use crate::paint::{self, Fill};
use crate::shape::{walk_tree, Scope, Walk};
use crate::theme::{default_color_map, read_color_map, Theme};
use crate::warnings::Warnings;
use crate::xml;

pub struct SlideOut {
    pub board: Value,
    pub notes: Option<String>,
}

/// Everything a slide needs from outside itself.
pub struct SlideInput<'s, 'p> {
    pub package: &'p Package,
    pub part: &'s str,
    /// 1-based position in the deck, used for ids and the board name.
    pub number: usize,
    pub size: (f64, f64),
    pub default_text: Option<Node<'s, 's>>,
}

/// A part's text, parsed lazily by the caller (roxmltree borrows it).
struct Source {
    part: String,
    text: String,
    rels: Relationships,
}

impl Source {
    fn load(package: &Package, part: &str) -> Result<Self, PptxError> {
        Ok(Self {
            part: part.to_string(),
            text: package.text(part)?,
            rels: package.relationships(part)?,
        })
    }

    fn related(&self, package: &Package, kind: &str) -> Result<Option<Self>, PptxError> {
        match first_of_kind(&self.rels, kind) {
            Some(rel) if package.bytes(&rel.target).is_some() => {
                Self::load(package, &rel.target).map(Some)
            }
            _ => Ok(None),
        }
    }
}

pub fn build_slide(
    input: &SlideInput<'_, '_>,
    media: &mut Media<'_>,
    warnings: &mut Warnings,
) -> Result<SlideOut, PptxError> {
    let package = input.package;
    let slide = Source::load(package, input.part)?;
    let layout = slide.related(package, "slideLayout")?;
    let master = match &layout {
        Some(layout) => layout.related(package, "slideMaster")?,
        None => None,
    };
    let theme_src = match &master {
        Some(master) => master.related(package, "theme")?,
        None => None,
    };
    let notes_src = slide.related(package, "notesSlide")?;

    let slide_doc = xml::parse(&slide.text, &slide.part)?;
    let layout_doc = layout
        .as_ref()
        .map(|s| xml::parse(&s.text, &s.part))
        .transpose()?;
    let master_doc = master
        .as_ref()
        .map(|s| xml::parse(&s.text, &s.part))
        .transpose()?;
    let theme_doc = theme_src
        .as_ref()
        .map(|s| xml::parse(&s.text, &s.part))
        .transpose()?;
    let theme = theme_doc
        .as_ref()
        .map(|d| Theme::parse(d.root_element()))
        .unwrap_or_default();

    let slide_root = slide_doc.root_element();
    let layout_root = layout_doc.as_ref().map(|d| d.root_element());
    let master_root = master_doc.as_ref().map(|d| d.root_element());

    let mut color_map = default_color_map();
    if let Some(map) = master_root.and_then(|m| xml::child(m, "clrMap")) {
        read_color_map(map, &mut color_map);
    }
    for root in [layout_root, Some(slide_root)].into_iter().flatten() {
        if let Some(over) = xml::path(root, &["clrMapOvr", "overrideClrMapping"]) {
            read_color_map(over, &mut color_map);
        }
    }

    let prefix = format!("s{}", input.number);
    let mut ctx = Ctx {
        colors: ColorContext {
            scheme: &theme.scheme,
            map: &color_map,
        },
        theme: &theme,
        media,
        warnings,
        ids: IdGen::new(prefix.clone()),
        layout: layout_root,
        master: master_root,
        default_text: input.default_text,
        targets: HashMap::new(),
    };

    let empty = Relationships::new();
    let layout_rels = layout.as_ref().map_or(&empty, |s| &s.rels);
    let master_rels = master.as_ref().map_or(&empty, |s| &s.rels);
    let background = background(
        &mut ctx,
        &[
            (Some(slide_root), &slide.rels),
            (layout_root, layout_rels),
            (master_root, master_rels),
        ],
    );

    let shows = |root: Option<Node<'_, '_>>| {
        root.and_then(|r| xml::attr_bool(r, "showMasterSp"))
            .unwrap_or(true)
    };
    let mut children = Vec::new();
    let mut add_tree = |ctx: &mut Ctx<'_, '_, '_>,
                        root: Option<Node<'_, '_>>,
                        rels: &Relationships,
                        scope: Scope| {
        let Some(tree) = root.and_then(|r| xml::path(r, &["cSld", "spTree"])) else {
            return;
        };
        let walk = Walk {
            rels,
            map: Mapping::SLIDE,
            group_fill: None,
            scope,
        };
        children.extend(walk_tree(ctx, &walk, tree));
    };
    if shows(Some(slide_root)) && shows(layout_root) {
        add_tree(&mut ctx, master_root, master_rels, Scope::Decoration);
    }
    if shows(Some(slide_root)) {
        add_tree(&mut ctx, layout_root, layout_rels, Scope::Decoration);
    }
    add_tree(&mut ctx, Some(slide_root), &slide.rels, Scope::Slide);

    let (width, height) = input.size;
    let mut animations = motion::animations(slide_root, &ctx.targets, input.size, ctx.warnings);
    attach_animations(&mut children, &mut animations);
    let transition = motion::transition(slide_root, ctx.warnings);
    let mut board = json!({
        "type": "frame",
        "id": prefix,
        "name": format!("Slide {}", input.number),
        "x": 0, "y": 0,
        "width": round(width), "height": round(height),
        "layout": "none",
        "clipContent": true,
        "fill": [background],
        "children": children,
    });
    if xml::attr_bool(slide_root, "show") == Some(false) {
        board["visible"] = json!(false);
    }
    if let Some(transition) = transition {
        board["slideTransition"] = transition;
    }

    let notes = notes_src
        .as_ref()
        .map(|src| xml::parse(&src.text, &src.part))
        .transpose()?
        .and_then(|doc| notes_text(doc.root_element()));
    Ok(SlideOut { board, notes })
}

/// Put each node's animations on it, wherever it sits in the tree.
fn attach_animations(nodes: &mut [Value], animations: &mut motion::Animations) {
    if animations.is_empty() {
        return;
    }
    for node in nodes {
        if let Some(list) = node["id"].as_str().and_then(|id| animations.remove(id)) {
            node["animations"] = Value::Array(list);
        }
        if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
            attach_animations(children, animations);
        }
    }
}

/// The first background along slide → layout → master. `bgRef` points
/// into the theme's background styles; its colour is what they paint
/// with, so it stands in for the whole style.
fn background(
    ctx: &mut Ctx<'_, '_, '_>,
    chain: &[(Option<Node<'_, '_>>, &Relationships)],
) -> Value {
    for (root, rels) in chain {
        let Some(bg) = root.and_then(|r| xml::path(r, &["cSld", "bg"])) else {
            continue;
        };
        if let Some(bg_pr) = xml::child(bg, "bgPr") {
            if let Fill::Paint(paint) = paint::read_fill(ctx, rels, bg_pr) {
                return paint;
            }
        }
        if let Some(color) = xml::child(bg, "bgRef").and_then(|r| ctx.colors.color_in(r)) {
            return paint::solid(color);
        }
    }
    paint::solid(ctx.colors.scheme_color("bg1").unwrap_or(Rgba::WHITE))
}

/// The speaker notes: the text of the notes page's body placeholder.
fn notes_text(root: Node<'_, '_>) -> Option<String> {
    let tree = xml::path(root, &["cSld", "spTree"])?;
    let body = tree
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "sp")
        .find(|sp| {
            xml::path(*sp, &["nvSpPr", "nvPr", "ph"]).and_then(|ph| ph.attribute("type"))
                == Some("body")
        })?;
    let tx = xml::child(body, "txBody")?;
    let lines: Vec<String> = xml::children(tx, "p")
        .map(|p| {
            xml::elements(p)
                .map(|el| match el.tag_name().name() {
                    "r" | "fld" => xml::text_of(el),
                    "br" => "\n".to_string(),
                    _ => String::new(),
                })
                .collect()
        })
        .collect();
    let text = lines.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Slide size from `p:sldSz` in pixels; 16:9 at 13.333" × 7.5" when absent.
pub fn slide_size(presentation: Node<'_, '_>) -> (f64, f64) {
    let size = xml::child(presentation, "sldSz");
    let emu =
        |name: &str, default: f64| size.and_then(|s| xml::attr_f64(s, name)).unwrap_or(default);
    (
        emu("cx", 12_192_000.0) * crate::context::PX_PER_EMU,
        emu("cy", 6_858_000.0) * crate::context::PX_PER_EMU,
    )
}

/// Slide parts in `sldIdLst` order.
pub fn slide_parts(presentation: Node<'_, '_>, rels: &Relationships) -> Vec<String> {
    let mut seen = HashMap::new();
    xml::child(presentation, "sldIdLst")
        .into_iter()
        .flat_map(|list| xml::children(list, "sldId"))
        .filter_map(|id| rels.get(xml::rel_attr(id, "id")?))
        .filter(|rel| rel.kind == "slide" && !rel.external)
        .filter(|rel| seen.insert(rel.target.clone(), ()).is_none())
        .map(|rel| rel.target.clone())
        .collect()
}
