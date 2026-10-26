//! `txBody` → one editable text node.
//!
//! Paragraphs become lines of one styled text (`\n` between them), runs
//! become segments, and bullets / numbering become a prefix on the line
//! (the editor's text model has no list paragraphs). The body's insets
//! shrink the node's box so the first glyph sits where PowerPoint draws it.

use roxmltree::Node;
use serde_json::{json, Map, Value};

use crate::color::Rgba;
use crate::context::{round, Ctx, PxBox, PX_PER_EMU, PX_PER_PT};
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
/// Default insets (left, top, right, bottom): 0.1" sides, 0.05" top and
/// bottom. Table cells use the same defaults for their margins.
pub const DEFAULT_INSETS_EMU: [f64; 4] = [91_440.0, 45_720.0, 91_440.0, 45_720.0];

pub struct TextSource<'a, 'i> {
    pub body: Node<'a, 'i>,
    pub chain: StyleChain<'a, 'i>,
    /// `a:bodyPr` of the shape, then of its layout and master placeholders.
    pub body_props: Vec<Node<'a, 'i>>,
    /// Insets in EMU (left, top, right, bottom) that replace the
    /// `bodyPr` ones: a table cell keeps its margins on `tcPr`.
    pub insets: Option<[f64; 4]>,
}

/// How a run looks; segments with equal styles merge.
#[derive(Clone, PartialEq)]
struct SegmentStyle {
    family: Option<String>,
    size_px: f64,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    color: Rgba,
    href: Option<String>,
}

#[derive(Clone)]
struct Segment {
    text: String,
    style: SegmentStyle,
}

impl Segment {
    fn to_json(&self) -> Value {
        let style = &self.style;
        let mut seg = Map::new();
        seg.insert("text".into(), json!(self.text));
        if let Some(family) = &style.family {
            seg.insert("fontFamily".into(), json!(family));
        }
        seg.insert("fontSize".into(), json!(round(style.size_px)));
        seg.insert("fontWeight".into(), json!(weight(style.bold)));
        if style.italic {
            seg.insert("fontStyle".into(), json!("italic"));
        }
        seg.insert("fill".into(), json!(style.color.hex_with_alpha()));
        if style.underline {
            seg.insert("underline".into(), json!(true));
        }
        if style.strike {
            seg.insert("strikethrough".into(), json!(true));
        }
        if let Some(href) = &style.href {
            seg.insert("href".into(), json!(href));
        }
        Value::Object(seg)
    }

    fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

fn weight(bold: bool) -> u32 {
    if bold {
        700
    } else {
        400
    }
}

/// The first `name` attribute along the body-property chain.
fn body_attr<'s>(source: &'s TextSource<'_, '_>, name: &str) -> Option<&'s str> {
    source.body_props.iter().find_map(|b| b.attribute(name))
}

/// `normAutofit` on the shape's own body: PowerPoint shrank the text to
/// fit and recorded by how much.
struct Autofit {
    font_scale: f64,
    spacing_reduction: f64,
}

impl Autofit {
    fn of(source: &TextSource<'_, '_>) -> Self {
        let autofit = source
            .body_props
            .first()
            .and_then(|body| xml::child(*body, "normAutofit"));
        let pct = |name: &str| {
            autofit
                .and_then(|a| xml::attr_f64(a, name))
                .map(|v| v / 100_000.0)
        };
        Self {
            font_scale: pct("fontScale").unwrap_or(1.0),
            spacing_reduction: pct("lnSpcReduction").unwrap_or(0.0),
        }
    }
}

/// Text node fields (no `id` / position: the caller merges them in) and
/// the inset box, or `None` when the body holds no visible text.
pub fn text_node(
    ctx: &mut Ctx<'_, '_, '_>,
    rels: &Relationships,
    source: &TextSource<'_, '_>,
    b: PxBox,
) -> Option<(Map<String, Value>, PxBox)> {
    if body_attr(source, "vert").is_some_and(|v| v != "horz") {
        ctx.warnings.add("vertical text (shown horizontal)");
    }
    let autofit = Autofit::of(source);
    let (segments, para) = collect_paragraphs(ctx, rels, source, autofit.font_scale)?;
    let first = segments.iter().find(|s| !s.is_blank())?.style.clone();

    let mut node = Map::new();
    node.insert("type".into(), json!("text"));
    node.insert(
        "content".into(),
        Value::Array(segments.iter().map(Segment::to_json).collect()),
    );
    if let Some(family) = &first.family {
        node.insert("fontFamily".into(), json!(family));
    }
    node.insert("fontSize".into(), json!(round(first.size_px)));
    node.insert("fontWeight".into(), json!(weight(first.bold)));
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
        Some(LineSpacing::Percent(p)) => SINGLE_SPACING * (p - autofit.spacing_reduction).max(0.5),
        Some(LineSpacing::Points(pt)) if first.size_px > 0.0 => pt * PX_PER_PT / first.size_px,
        _ => SINGLE_SPACING * (1.0 - autofit.spacing_reduction).max(0.5),
    };
    node.insert("lineHeight".into(), json!(round(line_height)));
    node.insert("textGrowth".into(), json!("fixed-width-height"));

    let inner = inset_box(source, b);
    node.insert("width".into(), json!(round(inner.w)));
    node.insert("height".into(), json!(round(inner.h)));
    Some((node, inner))
}

/// Every paragraph as segments, plus the resolved style of the first
/// paragraph that has text (it sets the node-level alignment and
/// spacing). `None` when nothing visible is left.
fn collect_paragraphs(
    ctx: &mut Ctx<'_, '_, '_>,
    rels: &Relationships,
    source: &TextSource<'_, '_>,
    font_scale: f64,
) -> Option<(Vec<Segment>, ParaStyle)> {
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

        let mut line = runs(ctx, rels, *p, &para.run, font_scale);
        let has_text = line.iter().any(|s| !s.is_blank());
        let bullet = para.bullet.clone().unwrap_or(Bullet::None);
        let number = count(&mut numbers, level, &bullet, has_text);
        if has_text {
            if let Some(prefix) = bullet_prefix(&bullet, para.bullet_font.as_deref(), number) {
                let indent = "    ".repeat(level);
                let mut mark = segment(
                    ctx,
                    &para.run,
                    &format!("{indent}{prefix} "),
                    font_scale,
                    None,
                );
                // The marker takes the size and colour of the text it leads.
                if let Some(first) = line.first() {
                    mark.style.size_px = first.style.size_px;
                    mark.style.color = first.style.color;
                }
                line.insert(0, mark);
            }
        }
        if index + 1 < paragraphs.len() {
            match line.last_mut() {
                Some(last) => last.text.push('\n'),
                None => line.push(segment(ctx, &para.run, "\n", font_scale, None)),
            }
        }
        if first_para.is_none() && has_text {
            first_para = Some(para);
        }
        segments.extend(line);
    }

    // Trailing empty paragraphs only add height PowerPoint never shows.
    while segments.last().is_some_and(Segment::is_blank) {
        segments.pop();
    }
    let last = segments.last_mut()?;
    let trimmed = last.text.trim_end_matches('\n').len();
    last.text.truncate(trimmed);
    Some((merge(segments), first_para.unwrap_or_default()))
}

/// The runs, fields and line breaks of one paragraph.
fn runs(
    ctx: &Ctx<'_, '_, '_>,
    rels: &Relationships,
    p: Node<'_, '_>,
    base_run: &RunStyle,
    font_scale: f64,
) -> Vec<Segment> {
    let mut line = Vec::new();
    for el in xml::elements(p) {
        let own = || xml::child(el, "rPr").map(|r| read_run(r, &ctx.colors));
        match el.tag_name().name() {
            "r" | "fld" => {
                let text = xml::child(el, "t").and_then(|t| t.text()).unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                let mut run = own().unwrap_or_default();
                run.inherit(base_run);
                let href = xml::path(el, &["rPr", "hlinkClick"])
                    .and_then(|h| xml::rel_attr(h, "id"))
                    .and_then(|id| rels.get(id))
                    .filter(|rel| rel.external)
                    .map(|rel| rel.target.clone());
                line.push(segment(ctx, &run, text, font_scale, href));
            }
            "br" => {
                let run = own().unwrap_or_else(|| base_run.clone());
                line.push(segment(ctx, &run, "\n", font_scale, None));
            }
            _ => {}
        }
    }
    line
}

/// Advance the auto-number counters for a paragraph at `level` and return
/// its number. Numbering counts paragraphs of the same level; a shallower
/// paragraph restarts the deeper levels, a non-numbered one its own.
fn count(numbers: &mut [u32; 9], level: usize, bullet: &Bullet, has_text: bool) -> u32 {
    for deeper in numbers.iter_mut().skip(level + 1) {
        *deeper = 0;
    }
    numbers[level] = match bullet {
        Bullet::AutoNum { start, .. } if has_text && numbers[level] == 0 => *start,
        Bullet::AutoNum { .. } if has_text => numbers[level].saturating_add(1),
        _ => 0,
    };
    numbers[level]
}

/// `b` shrunk by the body insets (or the source's override).
fn inset_box(source: &TextSource<'_, '_>, b: PxBox) -> PxBox {
    const NAMES: [&str; 4] = ["lIns", "tIns", "rIns", "bIns"];
    let [l, t, r, bottom] = std::array::from_fn(|i| {
        source.insets.map_or_else(
            || {
                body_attr(source, NAMES[i])
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(DEFAULT_INSETS_EMU[i])
            },
            |insets| insets[i],
        ) * PX_PER_EMU
    });
    PxBox {
        x: b.x + l,
        y: b.y + t,
        w: (b.w - l - r).max(1.0),
        h: (b.h - t - bottom).max(1.0),
    }
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
        style: SegmentStyle {
            family,
            size_px: size_pt * PX_PER_PT,
            bold: run.bold.unwrap_or(false),
            italic: run.italic.unwrap_or(false),
            underline: run.underline.unwrap_or(false),
            strike: run.strike.unwrap_or(false),
            color: run
                .color
                .or_else(|| ctx.colors.scheme_color("tx1"))
                .unwrap_or(Rgba::BLACK),
            href,
        },
    }
}

fn merge(segments: Vec<Segment>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for seg in segments {
        match out.last_mut() {
            Some(prev) if prev.style == seg.style => prev.text.push_str(&seg.text),
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
