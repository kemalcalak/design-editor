//! Pictures, graphic frames (tables, charts, SmartArt, OLE objects) and
//! `mc:AlternateContent`.
//!
//! Nothing here fails the import. Content the editor cannot model comes in
//! as the picture PowerPoint stored for older readers when there is one,
//! otherwise as a labelled placeholder box, and each case is counted as a
//! warning.

use roxmltree::Node;
use serde_json::{json, Value};

use crate::context::{base, round, Ctx, Placeholder, PxBox, Xfrm};
use crate::paint::{self, Fill};
use crate::shape::{walk_tree, Walk};
use crate::table;
use crate::xml;

/// `p:pic` → an image node. `xfrm_override` places a picture that stands
/// in for a graphic frame (an OLE object's preview).
pub fn picture(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    el: Node<'_, '_>,
    xfrm_override: Option<Xfrm>,
) -> Option<Value> {
    let xfrm = xfrm_override.or_else(|| picture_xfrm(ctx, el))?;
    let b = xfrm.px(walk.map);
    if let Some(nv_pr) = xml::path(el, &["nvPicPr", "nvPr"]) {
        if xml::child(nv_pr, "videoFile").is_some() {
            ctx.warnings.add("video (poster image only)");
        } else if xml::child(nv_pr, "audioFile").is_some() {
            ctx.warnings.add("audio (icon only)");
        } else if xml::descendant(nv_pr, "media").is_some() {
            // PowerPoint 2010 embeds media through a `p14:media` extension.
            ctx.warnings.add("video or audio (poster image only)");
        }
    }
    let blip_fill = xml::child(el, "blipFill");
    let url = blip_fill.and_then(|bf| paint::image_url(ctx, walk.rels, bf));
    let Some(url) = url else {
        return Some(placeholder_box(ctx, el, b, &xfrm, "Picture"));
    };
    if blip_fill
        .and_then(|bf| xml::child(bf, "srcRect"))
        .is_some_and(|r| r.attributes().any(|a| a.value().trim() != "0"))
    {
        ctx.warnings.add("picture crop (approximated)");
    }
    let mut node = base(ctx.ids.next(), el, b, Some(&xfrm), true);
    node.insert("type".into(), json!("image"));
    node.insert("src".into(), json!(url));
    node.insert("objectFit".into(), json!("fill"));
    node.insert("width".into(), json!(round(b.w)));
    node.insert("height".into(), json!(round(b.h)));
    Some(Value::Object(node))
}

/// A picture placeholder inherits its box like a shape does.
fn picture_xfrm(ctx: &Ctx<'_, '_, '_>, el: Node<'_, '_>) -> Option<Xfrm> {
    let own = xml::path(el, &["spPr", "xfrm"]).and_then(Xfrm::read);
    own.or_else(|| {
        let ph = Placeholder::of(el)?;
        [ctx.layout_placeholder(&ph), ctx.master_placeholder(&ph)]
            .into_iter()
            .flatten()
            .find_map(|n| xml::path(n, &["spPr", "xfrm"]).and_then(Xfrm::read))
    })
}

pub fn graphic_frame(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    el: Node<'_, '_>,
) -> Option<Value> {
    let xfrm = xml::child(el, "xfrm").and_then(Xfrm::read)?;
    let b = xfrm.px(walk.map);
    let data = xml::path(el, &["graphic", "graphicData"])?;
    let uri = data.attribute("uri").unwrap_or("");
    if let Some(tbl) = xml::child(data, "tbl") {
        return table::table(ctx, walk, el, tbl, b, &xfrm);
    }
    // An OLE object carries its own preview picture.
    if uri.ends_with("/ole") {
        ctx.warnings.add("embedded object (preview picture)");
        if let Some(pic) = data
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "pic")
        {
            return picture(ctx, walk, pic, Some(xfrm));
        }
        return Some(placeholder_box(ctx, el, b, &xfrm, "Object"));
    }
    let label = frame_label(uri);
    ctx.warnings
        .add(format!("{} (placeholder)", label.to_lowercase()));
    Some(placeholder_box(ctx, el, b, &xfrm, label))
}

fn frame_label(uri: &str) -> &'static str {
    if uri.contains("chart") {
        "Chart"
    } else if uri.contains("diagram") {
        "SmartArt"
    } else if uri.contains("model3d") {
        "3D model"
    } else {
        "Object"
    }
}

/// `mc:AlternateContent`: the `Fallback` is what PowerPoint stored for
/// readers without the newer feature — a picture of a chart, a plain shape
/// for an effect — so it is the faithful static rendering here.
pub fn alternate_content(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    el: Node<'_, '_>,
) -> Option<Value> {
    let choice = xml::child(el, "Choice");
    let fallback = xml::child(el, "Fallback").filter(|f| xml::elements(*f).next().is_some());
    let Some(fallback) = fallback else {
        return walk_tree(ctx, walk, choice?).into_iter().next();
    };
    let replaced = choice
        .and_then(|c| xml::descendant(c, "graphicData"))
        .and_then(|d| d.attribute("uri"))
        .map(frame_label);
    if let Some(label) = replaced {
        ctx.warnings
            .add(format!("{} (fallback picture)", label.to_lowercase()));
    }
    let mut nodes = walk_tree(ctx, walk, fallback);
    match nodes.len() {
        0 => None,
        1 => nodes.pop(),
        _ => Some(json!({
            "type": "group",
            "id": ctx.ids.next(),
            "x": 0, "y": 0,
            "children": nodes,
        })),
    }
}

/// A light box with a centred label where content could not come in.
pub fn placeholder_box(
    ctx: &mut Ctx<'_, '_, '_>,
    el: Node<'_, '_>,
    b: PxBox,
    xfrm: &Xfrm,
    label: &str,
) -> Value {
    let mut node = base(ctx.ids.next(), el, b, Some(xfrm), false);
    node.insert("type".into(), json!("frame"));
    node.insert("layout".into(), json!("none"));
    node.insert("width".into(), json!(round(b.w)));
    node.insert("height".into(), json!(round(b.h)));
    node.insert(
        "fill".into(),
        json!([{ "type": "solid", "color": "#F2F2F2" }]),
    );
    node.insert(
        "stroke".into(),
        json!({ "thickness": 1, "align": "inside", "dashPattern": [4, 4],
                "fill": [{ "type": "solid", "color": "#A6A6A6" }] }),
    );
    node.insert(
        "children".into(),
        json!([{
            "type": "text", "id": ctx.ids.next(), "x": 0, "y": 0,
            "width": round(b.w), "height": round(b.h),
            "content": label, "fontSize": 18, "textAlign": "center",
            "textAlignVertical": "middle", "textGrowth": "fixed-width-height",
            "fill": [{ "type": "solid", "color": "#7F7F7F" }],
        }]),
    );
    Value::Object(node)
}

/// The paint (solid, gradient or picture) authored directly in `parent`,
/// if any: a table cell's own background.
pub fn authored_paint(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    parent: Node<'_, '_>,
) -> Option<Value> {
    match paint::read_fill(ctx, walk.rels, parent) {
        Fill::Paint(p) => Some(p),
        _ => None,
    }
}
