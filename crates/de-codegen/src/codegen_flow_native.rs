//! Flutter / SwiftUI / Compose emission of grid and wrap containers
//! (#379): the container keeps its authored box, its children flow inside
//! the platform layout from [`super::codegen_flow`], and each child keeps
//! its size with its own subtree painted inside it.

use super::codegen_flow::flow_of;
use super::{emit_node_compose, emit_node_flutter, emit_node_swiftui, flutter_color};
use crate::{
    fmt_num, node_children, node_fill_css, node_is_ellipse, node_is_text, node_origin, node_size,
    node_text, parse_color,
};
use jian_ops_schema::node::{Padding, PenNode, ScriptNode};

/// `[top, right, bottom, left]` padding of a container node.
fn padding_trbl(node: &PenNode) -> [f64; 4] {
    let padding = match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => {
            n.container.padding.as_ref()
        }
        PenNode::Group(n) => n.container.padding.as_ref(),
        PenNode::Rectangle(n) => n.container.padding.as_ref(),
        _ => None,
    };
    match padding {
        Some(Padding::Uniform(v)) => [*v; 4],
        Some(Padding::XY([v, h])) => [*v, *h, *v, *h],
        Some(Padding::LtrB([t, r, b, l])) => [*t, *r, *b, *l],
        _ => [0.0; 4],
    }
}

fn rgba_u8(node: &PenNode) -> Option<[u8; 4]> {
    let (r, g, b, a) = node_fill_css(node).and_then(|c| parse_color(&c))?;
    let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some([to_u8(r), to_u8(g), to_u8(b), to_u8(a)])
}

/// Flutter: `Positioned` container + `GridView.count` / `Wrap`.
pub(super) fn emit_flutter_flow(out: &mut String, node: &PenNode, depth: usize) -> bool {
    let Some(flow) = flow_of(node) else {
        return false;
    };
    let indent = "  ".repeat(depth);
    let (x, y) = node_origin(node);
    let (w, h) = node_size(node);
    let [t, r, b, l] = padding_trbl(node);
    let color = node_fill_css(node)
        .map(|c| flutter_color(&c))
        .unwrap_or_else(|| "null".into());
    out.push_str(&format!(
        "{indent}Positioned(left: {}, top: {}, child: Container(width: {}, height: {}, decoration: BoxDecoration(color: {color}), padding: EdgeInsets.fromLTRB({}, {}, {}, {}), child: {}\n",
        fmt_num(x),
        fmt_num(y),
        fmt_num(w),
        fmt_num(h),
        fmt_num(l),
        fmt_num(t),
        fmt_num(r),
        fmt_num(b),
        flow.flutter_open()
    ));
    for child in &flow.children {
        let inner = "  ".repeat(depth + 1);
        let (cw, ch) = node_size(child);
        let body = if node_is_text(child) {
            format!("Text({:?})", node_text(child).unwrap_or_default())
        } else {
            let fill = node_fill_css(child)
                .map(|c| flutter_color(&c))
                .unwrap_or_else(|| "null".into());
            format!("Container(decoration: BoxDecoration(color: {fill}))")
        };
        out.push_str(&format!(
            "{inner}SizedBox(width: {}, height: {}, child: Stack(children: [\n{inner}  Positioned.fill(child: {body}),\n",
            fmt_num(cw),
            fmt_num(ch)
        ));
        for grandchild in node_children(child) {
            emit_node_flutter(out, grandchild, depth + 2);
        }
        out.push_str(&format!("{inner}])),\n"));
    }
    out.push_str(&format!("{indent}]))),\n"));
    true
}

/// SwiftUI: `LazyVGrid` framed and positioned like the container.
pub(super) fn emit_swiftui_flow(out: &mut String, node: &PenNode, depth: usize) -> bool {
    let Some(flow) = flow_of(node) else {
        return false;
    };
    let indent = "  ".repeat(depth);
    let (x, y) = node_origin(node);
    let (w, h) = node_size(node);
    let [t, r, b, l] = padding_trbl(node);
    out.push_str(&format!("{indent}{}\n", flow.swiftui_open()));
    for child in &flow.children {
        let inner = "  ".repeat(depth + 1);
        let (cw, ch) = node_size(child);
        let view = if node_is_text(child) {
            format!("Text({:?})", node_text(child).unwrap_or_default())
        } else {
            let shape = if node_is_ellipse(child) {
                "Ellipse()"
            } else {
                "Rectangle()"
            };
            match rgba_u8(child) {
                Some([r, g, b, a]) => format!(
                    "{shape}.fill(Color(red: {:.3}, green: {:.3}, blue: {:.3}, opacity: {:.3}))",
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    a as f32 / 255.0
                ),
                None => format!("{shape}.fill(Color.clear)"),
            }
        };
        out.push_str(&format!(
            "{inner}ZStack(alignment: .topLeading) {{\n{inner}  {view}\n"
        ));
        for grandchild in node_children(child) {
            emit_node_swiftui(out, grandchild, depth + 2);
        }
        out.push_str(&format!(
            "{inner}}}.frame(width: {}, height: {})\n",
            fmt_num(cw),
            fmt_num(ch)
        ));
    }
    out.push_str(&format!(
        "{indent}}}.padding(EdgeInsets(top: {}, leading: {}, bottom: {}, trailing: {})).frame(width: {}, height: {}, alignment: .topLeading).position(x: {}, y: {})\n",
        fmt_num(t),
        fmt_num(l),
        fmt_num(b),
        fmt_num(r),
        fmt_num(w),
        fmt_num(h),
        fmt_num(x + w / 2.0),
        fmt_num(y + h / 2.0)
    ));
    true
}

/// Compose: a sized `Box` holding `LazyVerticalGrid` / `FlowRow`.
pub(super) fn emit_compose_flow(out: &mut String, node: &PenNode, depth: usize) -> bool {
    let Some(flow) = flow_of(node) else {
        return false;
    };
    let indent = "  ".repeat(depth);
    let (x, y) = node_origin(node);
    let (w, h) = node_size(node);
    let [t, r, b, l] = padding_trbl(node);
    let background = |n: &PenNode| {
        rgba_u8(n)
            .map(|[r, g, b, a]| format!(".background(Color({r}, {g}, {b}, {a}))"))
            .unwrap_or_default()
    };
    out.push_str(&format!(
        "{indent}Box(modifier = Modifier.offset(x = {}.dp, y = {}.dp).size(width = {}.dp, height = {}.dp){}.padding(start = {}.dp, top = {}.dp, end = {}.dp, bottom = {}.dp)) {{\n",
        fmt_num(x),
        fmt_num(y),
        fmt_num(w),
        fmt_num(h),
        background(node),
        fmt_num(l),
        fmt_num(t),
        fmt_num(r),
        fmt_num(b)
    ));
    let inner = "  ".repeat(depth + 1);
    out.push_str(&format!("{inner}{}\n", flow.compose_open()));
    for child in &flow.children {
        let item = "  ".repeat(depth + 2);
        let (cw, ch) = node_size(child);
        let (open, close) = if flow.compose_items() {
            ("item { ", " }")
        } else {
            ("", "")
        };
        if node_is_text(child) {
            out.push_str(&format!(
                "{item}{open}Text(text = {:?}){close}\n",
                node_text(child).unwrap_or_default()
            ));
            continue;
        }
        out.push_str(&format!(
            "{item}{open}Box(modifier = Modifier.size(width = {}.dp, height = {}.dp){}) {{\n",
            fmt_num(cw),
            fmt_num(ch),
            background(child)
        ));
        for grandchild in node_children(child) {
            emit_node_compose(out, grandchild, depth + 3);
        }
        out.push_str(&format!("{item}}}{close}\n"));
    }
    out.push_str(&format!("{inner}}}\n{indent}}}\n"));
    true
}
