//! The shape tree walk: `p:spTree` / `p:grpSp` children → nodes.
//!
//! Shapes (`p:sp`) and connectors (`p:cxnSp`) are handled here; pictures,
//! graphic frames and placeholders for what cannot be expressed live in
//! [`crate::frame`].

use roxmltree::Node;
use serde_json::{json, Map, Value};

use crate::context::{base, round, Ctx, Mapping, Placeholder, PxBox, Xfrm};
use crate::frame;
use crate::geometry::{self, Geometry};
use crate::motion::Target;
use crate::package::Relationships;
use crate::paint::{self, Fill, Line};
use crate::text::{self, TextSource};
use crate::text_style::StyleChain;
use crate::xml;

/// Which shapes of a tree to bring in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// A slide: everything.
    Slide,
    /// A layout or master behind the slide: its artwork only. Their
    /// placeholders are prompts ("Click to add title"), not content.
    Decoration,
}

pub struct Walk<'r> {
    pub rels: &'r Relationships,
    pub map: Mapping,
    pub group_fill: Option<Value>,
    pub scope: Scope,
}

/// Convert every child of a shape tree / group.
pub fn walk_tree(ctx: &mut Ctx<'_, '_, '_>, walk: &Walk<'_>, tree: Node<'_, '_>) -> Vec<Value> {
    let mut out = Vec::new();
    for el in xml::elements(tree) {
        out.extend(element(ctx, walk, el));
    }
    out
}

fn element(ctx: &mut Ctx<'_, '_, '_>, walk: &Walk<'_>, el: Node<'_, '_>) -> Option<Value> {
    if walk.scope == Scope::Decoration && Placeholder::of(el).is_some() {
        return None;
    }
    let node = convert(ctx, walk, el)?;
    if walk.scope == Scope::Slide {
        record_target(ctx, el, &node);
    }
    Some(node)
}

/// Remember which node a slide shape became, for its animations.
fn record_target(ctx: &mut Ctx<'_, '_, '_>, el: Node<'_, '_>, node: &Value) {
    let Some(spid) = el
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "cNvPr")
        .and_then(|n| n.attribute("id"))
    else {
        return;
    };
    let (Some(id), Some(x), Some(y)) =
        (node["id"].as_str(), node["x"].as_f64(), node["y"].as_f64())
    else {
        return;
    };
    let size = |key: &str, line: &str| {
        node[key]
            .as_f64()
            .or_else(|| node[line].as_f64().map(f64::abs))
            .unwrap_or(0.0)
    };
    let (w, h) = (size("width", "x2"), size("height", "y2"));
    ctx.targets.entry(spid.to_string()).or_insert(Target {
        id: id.to_string(),
        center: (x + w / 2.0, y + h / 2.0),
        size: (w, h),
    });
}

fn convert(ctx: &mut Ctx<'_, '_, '_>, walk: &Walk<'_>, el: Node<'_, '_>) -> Option<Value> {
    match el.tag_name().name() {
        "sp" | "cxnSp" => shape(ctx, walk, el),
        "grpSp" => group(ctx, walk, el),
        "pic" => frame::picture(ctx, walk, el, None),
        "graphicFrame" => frame::graphic_frame(ctx, walk, el),
        "AlternateContent" => frame::alternate_content(ctx, walk, el),
        "contentPart" => {
            ctx.warnings.add("ink (not imported)");
            None
        }
        _ => None,
    }
}

fn group(ctx: &mut Ctx<'_, '_, '_>, walk: &Walk<'_>, el: Node<'_, '_>) -> Option<Value> {
    let gsp_pr = xml::child(el, "grpSpPr")?;
    let xfrm = xml::child(gsp_pr, "xfrm").and_then(Xfrm::read)?;
    let b = xfrm.px(walk.map);
    let group_fill = match paint::read_fill(ctx, walk.rels, gsp_pr) {
        Fill::Paint(p) => Some(p),
        Fill::Group => walk.group_fill.clone(),
        _ => None,
    };
    let inner = Walk {
        rels: walk.rels,
        map: xfrm.child_mapping(walk.map),
        group_fill,
        scope: walk.scope,
    };
    let children = walk_tree(ctx, &inner, el);
    if children.is_empty() {
        return None;
    }
    Some(wrap(ctx, el, b, &xfrm, children))
}

/// The inheritance sources of one shape: itself, then its layout and
/// master placeholders.
struct Sources<'a, 'i> {
    shape: Node<'a, 'i>,
    chain: Vec<Node<'a, 'i>>,
    placeholder: Option<Placeholder>,
}

impl<'a, 'i> Sources<'a, 'i> {
    fn of(ctx: &Ctx<'a, 'i, '_>, shape: Node<'a, 'i>) -> Self {
        let placeholder = Placeholder::of(shape);
        let mut chain = vec![shape];
        if let Some(ph) = &placeholder {
            chain.extend(ctx.layout_placeholder(ph));
            chain.extend(ctx.master_placeholder(ph));
        }
        Self {
            shape,
            chain,
            placeholder,
        }
    }

    fn sp_prs(&self) -> impl Iterator<Item = Node<'a, 'i>> + '_ {
        self.chain.iter().filter_map(|n| xml::child(*n, "spPr"))
    }

    fn xfrm(&self) -> Option<Xfrm> {
        self.sp_prs()
            .find_map(|p| xml::child(p, "xfrm").and_then(Xfrm::read))
    }
}

fn shape(ctx: &mut Ctx<'_, '_, '_>, walk: &Walk<'_>, el: Node<'_, '_>) -> Option<Value> {
    let sources = Sources::of(ctx, el);
    let xfrm = sources.xfrm()?;
    let b = xfrm.px(walk.map);
    let geometry_pr = sources
        .sp_prs()
        .find(|p| xml::child(*p, "prstGeom").is_some() || xml::child(*p, "custGeom").is_some());
    let geometry = geometry::read(geometry_pr, b.w, b.h);

    let fill = resolve_fill(ctx, walk, &sources);
    let stroke = resolve_line(ctx, &sources);
    let text = text_of(ctx, walk, &sources, b);

    let drawn = fill.is_some() || stroke.is_some() || matches!(geometry, Geometry::Line);
    let paint = drawn.then(|| paint_fields(ctx, el, &geometry, fill, stroke));

    match (paint, text) {
        (None, None) => None,
        (Some(fields), None) => Some(place(ctx, el, &geometry, fields, b, &xfrm)),
        (None, Some(text)) => Some(place_text(
            ctx,
            el,
            text,
            TextPlacement::Alone { b, xfrm: &xfrm },
        )),
        (Some(fields), Some(text)) => {
            // One group so the box and its text move together. The group
            // takes the rotation; the flips mirror the outline only, since
            // PowerPoint never mirrors text.
            let local = PxBox {
                x: 0.0,
                y: 0.0,
                ..b
            };
            let outline = place(
                ctx,
                el,
                &geometry,
                fields,
                local,
                &Xfrm { rot: 0.0, ..xfrm },
            );
            let label = place_text(ctx, el, text, TextPlacement::InGroup { origin: (b.x, b.y) });
            let unflipped = Xfrm {
                flip_h: false,
                flip_v: false,
                ..xfrm
            };
            Some(wrap(ctx, el, b, &unflipped, vec![outline, label]))
        }
    }
}

/// A group the size of `b` carrying `xfrm`'s rotation and flips, around
/// children placed in its own pixels. Rotation and flips pivot on the
/// group's centre, which is the shape box's centre, as in PowerPoint.
fn wrap(
    ctx: &mut Ctx<'_, '_, '_>,
    el: Node<'_, '_>,
    b: PxBox,
    xfrm: &Xfrm,
    children: Vec<Value>,
) -> Value {
    let mut node = base(ctx.ids.next(), el, b, Some(xfrm), true);
    node.insert("type".into(), json!("group"));
    node.insert("width".into(), json!(round(b.w)));
    node.insert("height".into(), json!(round(b.h)));
    node.insert("children".into(), Value::Array(children));
    Value::Object(node)
}

fn resolve_fill(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    sources: &Sources<'_, '_>,
) -> Option<Value> {
    for sp_pr in sources.sp_prs() {
        match paint::read_fill(ctx, walk.rels, sp_pr) {
            Fill::Unset => continue,
            Fill::None => return None,
            Fill::Group => return walk.group_fill.clone(),
            Fill::Paint(p) => return Some(p),
        }
    }
    match paint::style_ref(sources.shape, "fillRef", &ctx.colors) {
        Some((idx, Some(color))) if idx > 0 => Some(paint::solid(color)),
        _ => None,
    }
}

fn resolve_line(ctx: &mut Ctx<'_, '_, '_>, sources: &Sources<'_, '_>) -> Option<Value> {
    let style = paint::style_ref(sources.shape, "lnRef", &ctx.colors);
    let style_color = style.and_then(|(idx, c)| (idx > 0).then_some(c).flatten());
    for sp_pr in sources.sp_prs() {
        match paint::read_line(Some(sp_pr), &ctx.colors, style_color) {
            Line::Unset => continue,
            Line::None => return None,
            Line::Stroke(s) => return Some(s),
        }
    }
    let (idx, color) = style?;
    paint::style_line(idx, color?)
}

fn text_of(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    sources: &Sources<'_, '_>,
    b: PxBox,
) -> Option<(Map<String, Value>, PxBox)> {
    let body = xml::child(sources.shape, "txBody")?;
    let mut chain = StyleChain::default();
    let mut body_props = Vec::new();
    for source in &sources.chain {
        if let Some(tx) = xml::child(*source, "txBody") {
            chain.push(xml::child(tx, "lstStyle"));
            body_props.extend(xml::child(tx, "bodyPr"));
        }
    }
    if let Some(ph) = &sources.placeholder {
        chain.push(ctx.master_text_style(ph.master_style()));
    }
    chain.set_font_ref(text::font_ref(sources.shape, ctx));
    chain.push(ctx.default_text);
    let source = TextSource {
        body,
        chain,
        body_props,
        insets: None,
    };
    text::text_node(ctx, walk.rels, &source, b)
}

/// The paint fields of a shape (type, fill, stroke, path data), no box yet.
fn paint_fields(
    ctx: &mut Ctx<'_, '_, '_>,
    el: Node<'_, '_>,
    geometry: &Geometry,
    fill: Option<Value>,
    stroke: Option<Value>,
) -> Map<String, Value> {
    let mut node = Map::new();
    let fills = fill.map(|f| json!([f])).unwrap_or_else(|| json!([]));
    match geometry {
        Geometry::Rect { radius } => {
            node.insert("type".into(), json!("rectangle"));
            node.insert("fill".into(), fills);
            if *radius > 0.0 {
                node.insert("cornerRadius".into(), json!(round(*radius)));
            }
        }
        Geometry::Unsupported(name) => {
            ctx.warnings
                .add(format!("shape \"{name}\" (drawn as a rectangle)"));
            node.insert("type".into(), json!("rectangle"));
            node.insert("fill".into(), fills);
        }
        Geometry::Ellipse => {
            node.insert("type".into(), json!("ellipse"));
            node.insert("fill".into(), fills);
        }
        Geometry::Path { d, .. } => {
            node.insert("type".into(), json!("path"));
            node.insert("d".into(), json!(d));
            node.insert("fill".into(), fills);
        }
        Geometry::Line => {
            node.insert("type".into(), json!("line"));
            if el.tag_name().name() == "cxnSp" && geometry_is_bent(el) {
                ctx.warnings
                    .add("bent or curved connector (drawn straight)");
            }
            if has_arrowheads(el) {
                ctx.warnings.add("arrowheads (not imported)");
            }
        }
    }
    // A connector always shows: with no outline of its own it takes the
    // theme's thin line.
    let stroke = stroke.or_else(|| {
        matches!(geometry, Geometry::Line)
            .then(|| paint::style_line(paint::THIN_LINE, crate::color::Rgba::BLACK))
            .flatten()
    });
    if let Some(stroke) = stroke {
        node.insert("stroke".into(), stroke);
    }
    node
}

fn geometry_is_bent(el: Node<'_, '_>) -> bool {
    xml::descendant(el, "prstGeom")
        .and_then(|g| g.attribute("prst"))
        .is_some_and(|p| p.starts_with("bent") || p.starts_with("curved"))
}

fn has_arrowheads(el: Node<'_, '_>) -> bool {
    let Some(ln) = xml::path(el, &["spPr", "ln"]) else {
        return false;
    };
    ["headEnd", "tailEnd"].iter().any(|end| {
        xml::child(ln, end)
            .and_then(|e| e.attribute("type"))
            .is_some_and(|t| t != "none")
    })
}

/// Give paint fields their id and box. A line runs corner to corner, so
/// its flips pick the corners instead of mirroring the node.
fn place(
    ctx: &mut Ctx<'_, '_, '_>,
    el: Node<'_, '_>,
    geometry: &Geometry,
    fields: Map<String, Value>,
    b: PxBox,
    xfrm: &Xfrm,
) -> Value {
    let (at, node_xfrm) = match geometry {
        Geometry::Line => {
            let (x, x2) = if xfrm.flip_h {
                (b.x + b.w, -b.w)
            } else {
                (b.x, b.w)
            };
            let (y, y2) = if xfrm.flip_v {
                (b.y + b.h, -b.h)
            } else {
                (b.y, b.h)
            };
            let mut node = base(ctx.ids.next(), el, PxBox { x, y, ..b }, Some(xfrm), false);
            node.insert("x2".into(), json!(round(x2)));
            node.insert("y2".into(), json!(round(y2)));
            node.extend(fields);
            return Value::Object(node);
        }
        Geometry::Path { bounds, .. } => {
            let at = PxBox {
                x: b.x + bounds.x,
                y: b.y + bounds.y,
                w: bounds.w,
                h: bounds.h,
            };
            let transformed = xfrm.rot.abs() > 1e-6 || xfrm.flip_h || xfrm.flip_v;
            if transformed && at != b {
                // The path covers part of its box, so it would pivot on its
                // own centre; let a box-sized group carry the transform.
                let local = PxBox {
                    x: bounds.x,
                    y: bounds.y,
                    ..at
                };
                let path = sized(base(ctx.ids.next(), el, local, None, false), local, fields);
                return wrap(ctx, el, b, xfrm, vec![path]);
            }
            (at, xfrm)
        }
        _ => (b, xfrm),
    };
    sized(
        base(ctx.ids.next(), el, at, Some(node_xfrm), true),
        at,
        fields,
    )
}

/// `node` with `at`'s size and the paint fields.
fn sized(mut node: Map<String, Value>, at: PxBox, fields: Map<String, Value>) -> Value {
    node.insert("width".into(), json!(round(at.w.max(0.01))));
    node.insert("height".into(), json!(round(at.h.max(0.01))));
    node.extend(fields);
    Value::Object(node)
}

/// Where a text node goes.
enum TextPlacement<'x> {
    /// On its own: it carries the shape's name and rotation; `b` is the
    /// shape box its insets came from.
    Alone { b: PxBox, xfrm: &'x Xfrm },
    /// Inside the shape's group, whose origin is `origin`.
    InGroup { origin: (f64, f64) },
}

fn place_text(
    ctx: &mut Ctx<'_, '_, '_>,
    el: Node<'_, '_>,
    text: (Map<String, Value>, PxBox),
    placement: TextPlacement<'_>,
) -> Value {
    let (fields, inner) = text;
    match placement {
        TextPlacement::Alone { b, xfrm } => {
            let centred = (inner.x + inner.w / 2.0 - (b.x + b.w / 2.0)).abs() < 0.01
                && (inner.y + inner.h / 2.0 - (b.y + b.h / 2.0)).abs() < 0.01;
            if xfrm.rot.abs() > 1e-6 && !centred {
                // Uneven insets move the text's centre off the shape's;
                // rotate around the shape like PowerPoint does.
                let label = place_text(
                    ctx,
                    el,
                    (fields, inner),
                    TextPlacement::InGroup { origin: (b.x, b.y) },
                );
                let unflipped = Xfrm {
                    flip_h: false,
                    flip_v: false,
                    ..*xfrm
                };
                return wrap(ctx, el, b, &unflipped, vec![label]);
            }
            let mut node = base(ctx.ids.next(), el, inner, Some(xfrm), false);
            node.extend(fields);
            Value::Object(node)
        }
        TextPlacement::InGroup { origin } => {
            let at = PxBox {
                x: inner.x - origin.0,
                y: inner.y - origin.1,
                ..inner
            };
            let mut node = base(ctx.ids.next(), el, at, None, false);
            // The group carries the name; the text inside it is just the text.
            node.remove("name");
            node.remove("visible");
            node.extend(fields);
            Value::Object(node)
        }
    }
}
