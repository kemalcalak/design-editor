//! `a:tbl` → a group of cell boxes with editable text.
//!
//! The editor has no table node, so each cell is a rectangle (its fill
//! and a hairline border) under its text. Merged cells span their grid
//! columns / rows. Table styles (banding, header row colours) are not
//! resolved: cells without their own fill come in white, and that is
//! reported once per table.

use roxmltree::Node;
use serde_json::{json, Value};

use crate::context::{base, round, Ctx, PxBox, Xfrm, PX_PER_EMU};
use crate::frame::solid_or_none;
use crate::shape::Walk;
use crate::text::{self, TextSource};
use crate::text_style::StyleChain;
use crate::xml;

const BORDER: &str = "#BFBFBF";

pub fn table(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    frame: Node<'_, '_>,
    tbl: Node<'_, '_>,
    b: PxBox,
    xfrm: &Xfrm,
) -> Option<Value> {
    let columns: Vec<f64> = xml::child(tbl, "tblGrid")
        .into_iter()
        .flat_map(|g| xml::children(g, "gridCol"))
        .map(|c| xml::attr_f64(c, "w").unwrap_or(0.0) * walk.map.scale.0)
        .collect();
    let rows: Vec<Node<'_, '_>> = xml::children(tbl, "tr").collect();
    if columns.is_empty() || rows.is_empty() {
        return None;
    }
    let heights: Vec<f64> = rows
        .iter()
        .map(|r| xml::attr_f64(*r, "h").unwrap_or(0.0) * walk.map.scale.1)
        .collect();
    let col_x: Vec<f64> = prefix_sums(&columns);
    let row_y: Vec<f64> = prefix_sums(&heights);
    let styled = xml::path(tbl, &["tblPr", "tableStyleId"]).is_some();
    let mut unstyled_cells = false;

    let mut cells = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in xml::children(*row, "tc").enumerate() {
            if c >= columns.len()
                || xml::attr_bool(cell, "hMerge") == Some(true)
                || xml::attr_bool(cell, "vMerge") == Some(true)
            {
                continue;
            }
            let span_c = xml::attr_i64(cell, "gridSpan").unwrap_or(1).max(1) as usize;
            let span_r = xml::attr_i64(cell, "rowSpan").unwrap_or(1).max(1) as usize;
            let c_end = (c + span_c).min(columns.len());
            let r_end = (r + span_r).min(rows.len());
            let cell_box = PxBox {
                x: col_x[c],
                y: row_y[r],
                w: col_x[c_end] - col_x[c],
                h: row_y[r_end] - row_y[r],
            };
            let tc_pr = xml::child(cell, "tcPr");
            let fill = tc_pr.and_then(|p| solid_or_none(ctx, walk, p));
            unstyled_cells |= fill.is_none();
            cells.push(json!({
                "type": "rectangle",
                "id": ctx.ids.next(),
                "x": round(cell_box.x), "y": round(cell_box.y),
                "width": round(cell_box.w), "height": round(cell_box.h),
                "fill": [fill.unwrap_or_else(|| json!({ "type": "solid", "color": "#FFFFFF" }))],
                "stroke": { "thickness": 1, "align": "inside",
                            "fill": [{ "type": "solid", "color": BORDER }] },
            }));
            if let Some(text) = cell_text(ctx, walk, cell, tc_pr, cell_box) {
                cells.push(text);
            }
        }
    }
    if styled && unstyled_cells {
        ctx.warnings.add("table style (cell colours not imported)");
    }
    let mut node = base(ctx.ids.next(), frame, b, Some(xfrm), false);
    node.insert("type".into(), json!("group"));
    node.insert("width".into(), json!(round(b.w)));
    node.insert("height".into(), json!(round(b.h)));
    node.insert("children".into(), Value::Array(cells));
    Some(Value::Object(node))
}

fn prefix_sums(sizes: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(sizes.len() + 1);
    let mut at = 0.0;
    out.push(at);
    for size in sizes {
        at += size;
        out.push(at);
    }
    out
}

fn cell_text(
    ctx: &mut Ctx<'_, '_, '_>,
    walk: &Walk<'_>,
    cell: Node<'_, '_>,
    tc_pr: Option<Node<'_, '_>>,
    b: PxBox,
) -> Option<Value> {
    let body = xml::child(cell, "txBody")?;
    let mut chain = StyleChain::default();
    chain.push(xml::child(body, "lstStyle"));
    chain.push(ctx.default_text);
    let source = TextSource {
        body,
        chain,
        // Cell margins live on `tcPr` (marL / marT / …), not `bodyPr`.
        body_props: Vec::new(),
    };
    let margin = |name: &str, default: f64| {
        tc_pr
            .and_then(|p| xml::attr_f64(p, name))
            .unwrap_or(default)
            * PX_PER_EMU
    };
    let inner = PxBox {
        x: b.x + margin("marL", 91_440.0),
        y: b.y + margin("marT", 45_720.0),
        w: (b.w - margin("marL", 91_440.0) - margin("marR", 91_440.0)).max(1.0),
        h: (b.h - margin("marT", 45_720.0) - margin("marB", 45_720.0)).max(1.0),
    };
    // `text_node` applies default insets; hand it a box that undoes them
    // so the cell margins are the only ones.
    let default_l = 91_440.0 * PX_PER_EMU;
    let default_t = 45_720.0 * PX_PER_EMU;
    let padded = PxBox {
        x: inner.x - default_l,
        y: inner.y - default_t,
        w: inner.w + 2.0 * default_l,
        h: inner.h + 2.0 * default_t,
    };
    let (mut fields, at) = text::text_node(ctx, walk.rels, &source, padded)?;
    if let Some(anchor) = tc_pr.and_then(|p| p.attribute("anchor")) {
        let vertical = match anchor {
            "ctr" => "middle",
            "b" => "bottom",
            _ => "top",
        };
        fields.insert("textAlignVertical".into(), json!(vertical));
    }
    fields.insert("id".into(), json!(ctx.ids.next()));
    fields.insert("x".into(), json!(round(at.x)));
    fields.insert("y".into(), json!(round(at.y)));
    Some(Value::Object(fields))
}
