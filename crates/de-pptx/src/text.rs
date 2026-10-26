//! `txBody` → one editable text node.
//!
//! Paragraphs become lines of one styled text (`\n` between them), runs
//! become segments, and bullets / numbering become a prefix on the line
//! (the editor's text model has no list paragraphs). The body's insets
//! shrink the node's box so the first glyph sits where PowerPoint draws it.

use roxmltree::Node;
use serde_json::{json, Map, Value};

use crate::color::Rgba;
use crate::context::{round, Ctx, PxBox, PX_PER_EMU};
use crate::package::Relationships;
use crate::text_style::{
    bullet_prefix, read_paragraph, read_run, Bullet, LineSpacing, ParaStyle, RunStyle, StyleChain,
};
use crate::xml;

/// PowerPoint's size for text no style names.
const DEFAULT_SIZE_PT: f64 = 18.0;
/// Single line spacing is about 1.2 em for the fonts decks use; the
/// editor's `lineHeight` is a multiple of the font size.
const SINGLE_SPACING: f64 = 1.2;
/// Default insets: 0.1" left / right, 0.05" top / bottom.
const DEFAULT_INSETS_EMU: [f64; 4] = [91_440.0, 45_720.0, 91_440.0, 45_720.0];

pub struct TextSource<'a, 'i> {
    pub body: Node<'a, 'i>,
    pub chain: StyleChain<'a, 'i>,
    /// `a:bodyPr` of the shape, then of its layout and master placeholders.
    pub body_props: Vec<Node<'a, 'i>>,
}

#[derive(Clone, PartialEq)]
struct Segment {
    text: String,
    family: Option<String>,
    size_px: f64,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    color: Rgba,
    href: Option<String>,
}

impl Segment {
    fn same_style(&self, other: &Segment) -> bool {
        Segment {
            text: String::new(),
            ..self.clone()
        } == Segment {
            text: String::new(),
            ..other.clone()
        }
    }

    fn to_json(&self) -> Value {
        let mut seg = Map::new();
        seg.insert("text".into(), json!(self.text));
        if let Some(family) = &self.family {
            seg.insert("fontFamily".into(), json!(family));
        }
        seg.insert("fontSize".into(), json!(round(self.size_px)));
        seg.insert(
            "fontWeight".into(),
            json!(if self.bold { 700 } else { 400 }),
        );
        if self.italic {
            seg.insert("fontStyle".into(), json!("italic"));
        }
        seg.insert("fill".into(), json!(self.color.hex_with_alpha()));
        if self.underline {
            seg.insert("underline".into(), json!(true));
        }
        if self.strike {
            seg.insert("strikethrough".into(), json!(true));
        }
        if let Some(href) = &self.href {
            seg.insert("href".into(), json!(href));
        }
        Value::Object(seg)
    }
}

/// The first `name` attribute along the body-property chain.
fn body_attr<'s>(source: &'s TextSource<'_, '_>, name: &str) -> Option<&'s str> {
    source.body_props.iter().find_map(|b| b.attribute(name))
}

/// Text node fields (no `id` / position: the caller merges them in) and
/// the inset box, or `None` when the body holds no visible text.
pub fn text_node(
    ctx: &mut Ctx<'_, '_, '_>,
    rels: &Relationships,
    source: &TextSource<'_, '_>,
    b: PxBox,
) -> Option<(Map<String, Value>, PxBox)> {
    let own_body = source.body_props.first().copied();
    let autofit = own_body.and_then(|body| xml::child(body, "normAutofit"));
    let font_scale = autofit
        .and_then(|a| xml::attr_f64(a, "fontScale"))
        .map_or(1.0, |s| s / 100_000.0);
    let spacing_reduction = autofit
        .and_then(|a| xml::attr_f64(a, "lnSpcReduction"))
        .map_or(0.0, |s| s / 100_000.0);
    if body_attr(source, "vert").is_some_and(|v| v != "horz") {
        ctx.warnings.add("vertical text (shown horizontal)");
    }

    let mut segments: Vec<Segment> = Vec::new();
    let mut first_para: Option<ParaStyle> = None;
    let mut numbers = [0u32; 9];
    let paragraphs: Vec<Node<'_, '_>> = xml::children(source.body, "p").collect();
    for (index, p) in paragraphs.iter().enumerate() {
        let ppr = xml::child(*p, "pPr");
        let level = ppr
            .and_then(|n| xml::attr_i64(n, "lvl"))
            .unwrap_or(0)
            .clamp(0, 8) as usize;
        let mut para = ppr
            .map(|n| read_paragraph(n, &ctx.colors))
            .unwrap_or_default();
        para.inherit(&source.chain.level(level, &ctx.colors));
        let base_run = para.run.clone();

        let mut line: Vec<Segment> = Vec::new();
        for el in xml::elements(*p) {
            match el.tag_name().name() {
                "r" | "fld" => {
                    let text = xml::child(el, "t").and_then(|t| t.text()).unwrap_or("");
                    if text.is_empty() {
                        continue;
                    }
                    let mut run = xml::child(el, "rPr")
                        .map(|r| read_run(r, &ctx.colors))
                        .unwrap_or_default();
                    run.inherit(&base_run);
                    let href = xml::path(el, &["rPr", "hlinkClick"])
                        .and_then(|h| xml::rel_attr(h, "id"))
                        .and_then(|id| rels.get(id))
                        .filter(|rel| rel.external)
                        .map(|rel| rel.target.clone());
                    line.push(segment(ctx, &run, text, font_scale, href));
                }
                "br" => {
                    let run = xml::child(el, "rPr")
                        .map(|r| read_run(r, &ctx.colors))
                        .unwrap_or_else(|| base_run.clone());
                    line.push(segment(ctx, &run, "\n", font_scale, None));
                }
                _ => {}
            }
        }

        let has_text = line.iter().any(|s| !s.text.trim().is_empty());
        // Numbering counts paragraphs of the same level; a shallower
        // paragraph restarts the deeper levels.
        for deeper in numbers.iter_mut().skip(level + 1) {
            *deeper = 0;
        }
        let bullet = para.bullet.clone().unwrap_or(Bullet::None);
        match &bullet {
            Bullet::AutoNum { start, .. } if has_text => {
                numbers[level] = if numbers[level] == 0 {
                    *start
                } else {
                    numbers[level] + 1
                };
            }
            _ => numbers[level] = 0,
        }
        if has_text {
            if let Some(prefix) =
                bullet_prefix(&bullet, para.bullet_font.as_deref(), numbers[level])
            {
                let indent = "    ".repeat(level);
                let mut mark = segment(
                    ctx,
                    &base_run,
                    &format!("{indent}{prefix} "),
                    font_scale,
                    None,
                );
                if let Some(first) = line.first() {
                    mark.size_px = first.size_px;
                    mark.color = first.color;
                }
                line.insert(0, mark);
            }
        }
        if index + 1 < paragraphs.len() {
            match line.last_mut() {
                Some(last) => last.text.push('\n'),
                None => line.push(segment(ctx, &base_run, "\n", font_scale, None)),
            }
        }
        if first_para.is_none() && has_text {
            first_para = Some(para);
        }
        segments.extend(line);
    }

    if !segments.iter().any(|s| !s.text.trim().is_empty()) {
        return None;
    }
    // Trailing empty paragraphs only add height PowerPoint never shows.
    while segments.last().is_some_and(|s| s.text.trim().is_empty()) {
        segments.pop();
    }
    if let Some(last) = segments.last_mut() {
        let trimmed = last.text.trim_end_matches('\n').len();
        last.text.truncate(trimmed);
    }
    let merged = merge(segments);
    let first = merged.iter().find(|s| !s.text.trim().is_empty())?.clone();
    let para = first_para.unwrap_or_default();

    let mut node = Map::new();
    node.insert("type".into(), json!("text"));
    node.insert(
        "content".into(),
        Value::Array(merged.iter().map(Segment::to_json).collect()),
    );
    if let Some(family) = &first.family {
        node.insert("fontFamily".into(), json!(family));
    }
    node.insert("fontSize".into(), json!(round(first.size_px)));
    node.insert(
        "fontWeight".into(),
        json!(if first.bold { 700 } else { 400 }),
    );
    node.insert(
        "fill".into(),
        json!([{ "type": "solid", "color": first.color.hex() }]),
    );
    let align = match para.align.as_deref() {
        Some("ctr") => "center",
        Some("r") => "right",
        Some("just") | Some("dist") | Some("justLow") | Some("thaiDist") => "justify",
        _ => "left",
    };
    node.insert("textAlign".into(), json!(align));
    let vertical = match body_attr(source, "anchor") {
        Some("ctr") => "middle",
        Some("b") => "bottom",
        _ => "top",
    };
    node.insert("textAlignVertical".into(), json!(vertical));
    let line_height = match para.line_spacing {
        Some(LineSpacing::Percent(p)) => SINGLE_SPACING * (p - spacing_reduction).max(0.5),
        Some(LineSpacing::Points(pt)) if first.size_px > 0.0 => pt * 96.0 / 72.0 / first.size_px,
        _ => SINGLE_SPACING * (1.0 - spacing_reduction).max(0.5),
    };
    node.insert("lineHeight".into(), json!(round(line_height)));
    node.insert("textGrowth".into(), json!("fixed-width-height"));

    let inset = |name: &str, i: usize| {
        body_attr(source, name)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(DEFAULT_INSETS_EMU[i])
            * PX_PER_EMU
    };
    let (l, t, r, bottom) = (
        inset("lIns", 0),
        inset("tIns", 1),
        inset("rIns", 2),
        inset("bIns", 3),
    );
    let inner = PxBox {
        x: b.x + l,
        y: b.y + t,
        w: (b.w - l - r).max(1.0),
        h: (b.h - t - bottom).max(1.0),
    };
    node.insert("width".into(), json!(round(inner.w)));
    node.insert("height".into(), json!(round(inner.h)));
    Some((node, inner))
}

fn segment(
    ctx: &Ctx<'_, '_, '_>,
    run: &RunStyle,
    text: &str,
    scale: f64,
    href: Option<String>,
) -> Segment {
    let size_pt = run.size_pt.unwrap_or(DEFAULT_SIZE_PT) * scale;
    let family = run
        .typeface
        .as_deref()
        .and_then(|face| ctx.theme.typeface(face))
        .or_else(|| ctx.theme.minor_font.clone());
    Segment {
        text: text.to_string(),
        family,
        size_px: size_pt * 96.0 / 72.0,
        bold: run.bold.unwrap_or(false),
        italic: run.italic.unwrap_or(false),
        underline: run.underline.unwrap_or(false),
        strike: run.strike.unwrap_or(false),
        color: run
            .color
            .or_else(|| ctx.colors.scheme_color("tx1"))
            .unwrap_or(Rgba::BLACK),
        href,
    }
}

fn merge(segments: Vec<Segment>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for seg in segments {
        match out.last_mut() {
            Some(prev) if prev.same_style(&seg) => prev.text.push_str(&seg.text),
            _ => out.push(seg),
        }
    }
    out
}

/// The run style a shape's `p:style/a:fontRef` contributes: its colour,
/// and the theme font it names (`major` / `minor`).
pub fn font_ref(shape: Node<'_, '_>, ctx: &Ctx<'_, '_, '_>) -> RunStyle {
    let Some(r) = xml::path(shape, &["style", "fontRef"]) else {
        return RunStyle::default();
    };
    RunStyle {
        color: ctx.colors.color_in(r),
        typeface: match r.attribute("idx") {
            Some("major") => Some("+mj-lt".to_string()),
            Some("minor") => Some("+mn-lt".to_string()),
            _ => None,
        },
        ..RunStyle::default()
    }
}
