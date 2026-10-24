//! Node-tree walk for the pen.dev conversion: per-node defaults, layout
//! position, visibility and path `viewBox`, plus removal of node kinds the
//! canonical schema cannot hold. Paint lives in [`super::paint`], variable
//! references in [`super::variables`].

use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Map, Value};

use super::variables::{self, Theme, Variables};
use super::{paint, shader, AssetReader, PenDevReport};

/// pen.dev node kinds with no canonical equivalent. They are removed with
/// their subtree and counted in the report.
const DROPPED_KINDS: &[(&str, &str)] = &[
    ("note", "note"),
    ("context", "context note"),
    ("script", "script node"),
    ("browser", "browser node"),
];

/// Entity keys pen.dev uses for its own editor and AI tooling only.
const EDITOR_ONLY_KEYS: &[&str] = &["placeholder", "context", "metadata"];

/// Shared state for one conversion.
pub(super) struct Ctx<'a> {
    pub(super) assets: AssetReader<'a>,
    pub(super) report: &'a mut PenDevReport,
    /// Relative image URL → `op-image:` reference (`None` once known
    /// missing), so a bitmap used by many fills is read and encoded once.
    pub(super) images: HashMap<String, Option<String>>,
    /// Table id → data URL, written to the document's `images` table.
    pub(super) image_table: Map<String, Value>,
    pub(super) variables: Variables,
    /// GLSL files and SkSL compile results of the shader paints.
    pub(super) shaders: shader::ShaderCache,
}

impl<'a> Ctx<'a> {
    pub(super) fn new(
        assets: AssetReader<'a>,
        report: &'a mut PenDevReport,
        variables: Variables,
    ) -> Self {
        Self {
            assets,
            report,
            images: HashMap::new(),
            image_table: Map::new(),
            variables,
            shaders: shader::ShaderCache::default(),
        }
    }
}

/// How a visited object is used: a node in the tree, or an instance's
/// property patch (a `descendants` entry without `type`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Node,
    Patch,
}

/// Convert a `children` array and everything below it. `parent_flow` says
/// whether the owning node is an auto-layout frame (its children's `x` / `y`
/// are ignored). Iterative so a deep design cannot overflow the stack; each
/// entry carries the theme its variable references resolve against.
pub(super) fn convert_children(children: &mut Vec<Value>, parent_flow: bool, ctx: &mut Ctx<'_>) {
    let mut stack: Vec<(&mut Value, bool, Role, Rc<Theme>)> = Vec::new();
    let root_theme = Rc::new(ctx.variables.default_theme().clone());
    children.retain(|child| keep_child(child, ctx.report));
    stack.extend(
        children
            .iter_mut()
            .map(|child| (child, parent_flow, Role::Node, Rc::clone(&root_theme))),
    );
    while let Some((node, in_flow, role, theme)) = stack.pop() {
        let Value::Object(map) = node else {
            continue;
        };
        let theme = variables::scoped_theme(map, &theme).map_or(theme, Rc::new);
        convert_node(map, in_flow, role, &theme, ctx);
        let flow = is_auto_layout(map);
        for (key, value) in map.iter_mut() {
            match (key.as_str(), value) {
                ("children", Value::Array(kids)) => {
                    kids.retain(|child| keep_child(child, ctx.report));
                    stack.extend(
                        kids.iter_mut()
                            .map(|child| (child, flow, Role::Node, Rc::clone(&theme))),
                    );
                }
                ("descendants", Value::Object(overrides)) => {
                    overrides.retain(|_, value| keep_child(value, ctx.report));
                    for value in overrides.values_mut() {
                        // pen.dev replaces a descendant with an override that
                        // has `type`; the canonical resolver only merges keys
                        // (`ref_resolve::apply_overrides`), so the original
                        // child's other fields survive underneath.
                        let role = if value.get("type").is_some() {
                            ctx.report.note("component child replacement (merged)");
                            Role::Node
                        } else {
                            Role::Patch
                        };
                        stack.push((value, false, role, Rc::clone(&theme)));
                    }
                }
                _ => {}
            }
        }
    }
}

fn keep_child(child: &Value, report: &mut PenDevReport) -> bool {
    let kind = child.get("type").and_then(Value::as_str);
    match DROPPED_KINDS.iter().find(|(name, _)| Some(*name) == kind) {
        Some((_, feature)) => {
            report.note(feature);
            false
        }
        None => true,
    }
}

fn convert_node(
    map: &mut Map<String, Value>,
    in_flow: bool,
    role: Role,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) {
    // First, so every conversion below sees concrete values.
    variables::resolve_node(map, &ctx.variables, theme, ctx.report);
    // An absolutely positioned child is out of its parent's flow.
    let absolute = map.get("layoutPosition").and_then(Value::as_str) == Some("absolute");
    match map.get("type").and_then(Value::as_str) {
        Some("frame") => apply_frame_defaults(map),
        Some("path") => apply_path_view_box(map, in_flow && !absolute, ctx.report),
        _ => {}
    }
    for key in EDITOR_ONLY_KEYS {
        map.remove(*key);
    }
    for axis in ["width", "height"] {
        strip_sizing_fallback(map, axis);
    }
    convert_visibility(map, ctx.report);
    convert_layout_position(map);
    paint::convert_blend_mode(map, ctx.report);
    paint::convert_fill_key(map, theme, ctx);
    paint::convert_stroke(map, role, theme, ctx);
    paint::convert_effects(map, theme, ctx);
}

/// pen.dev frames lay out horizontally and hug their content unless the
/// file says otherwise; the canonical schema's defaults are free-form.
fn apply_frame_defaults(map: &mut Map<String, Value>) {
    let layout = map
        .entry("layout")
        .or_insert_with(|| Value::String("horizontal".to_owned()));
    if layout.as_str() == Some("none") {
        return;
    }
    for axis in ["width", "height"] {
        map.entry(axis)
            .or_insert_with(|| Value::String("fit_content".to_owned()));
    }
}

/// `fit_content(100)` / `fill_container(100)`: pen.dev's fallback size for
/// an unresolvable dependency. The canonical keyword has no fallback slot.
fn strip_sizing_fallback(map: &mut Map<String, Value>, axis: &str) {
    if let Some(Value::String(size)) = map.get_mut(axis) {
        if let Some(open) = size.find('(') {
            size.truncate(open);
        }
    }
}

/// `enabled: false` hides a node in pen.dev; the canonical spelling is
/// `visible: false`.
fn convert_visibility(map: &mut Map<String, Value>, report: &mut PenDevReport) {
    match map.remove("enabled") {
        Some(Value::Bool(false)) => {
            map.insert("visible".to_owned(), Value::Bool(false));
        }
        Some(Value::String(_)) => report.note("variable-bound visibility"),
        _ => {}
    }
}

/// `layoutPosition: "absolute"` lifts a child out of its parent's flow. The
/// canonical layout keeps a child absolute when it carries the `overlay`
/// role (`jian-core` `flow_child::keeps_absolute`).
fn convert_layout_position(map: &mut Map<String, Value>) {
    if map
        .remove("layoutPosition")
        .as_ref()
        .and_then(Value::as_str)
        == Some("absolute")
    {
        map.entry("role")
            .or_insert_with(|| Value::String("overlay".to_owned()));
    }
}

fn is_auto_layout(map: &Map<String, Value>) -> bool {
    map.get("type").and_then(Value::as_str) == Some("frame")
        && matches!(
            map.get("layout").and_then(Value::as_str),
            Some("horizontal" | "vertical")
        )
}

/// A pen.dev path maps its `viewBox` region onto the node box; the canonical
/// renderer maps the geometry's tight bounds instead. Shrink the box to the
/// part of the viewBox the geometry covers so the drawing lands where pen.dev
/// draws it. Inside auto layout the box size drives the flow, so it is kept
/// and the difference reported.
fn apply_path_view_box(map: &mut Map<String, Value>, in_flow: bool, report: &mut PenDevReport) {
    let Some(view_box) = map.remove("viewBox") else {
        return;
    };
    let Some([vb_x, vb_y, vb_w, vb_h]) = numbers4(&view_box) else {
        report.note("path viewBox");
        return;
    };
    let geometry = map
        .get("geometry")
        .or_else(|| map.get("d"))
        .and_then(Value::as_str);
    let Some((bx, by, bw, bh)) = geometry.and_then(de_editor_core::svg_path_data_bounds) else {
        report.note("path viewBox");
        return;
    };
    let (bx, by, bw, bh) = (f64::from(bx), f64::from(by), f64::from(bw), f64::from(bh));
    const EPSILON: f64 = 0.01;
    let matches_bounds = (bx - vb_x).abs() < EPSILON
        && (by - vb_y).abs() < EPSILON
        && (bw - vb_w).abs() < EPSILON
        && (bh - vb_h).abs() < EPSILON;
    if matches_bounds {
        return;
    }
    let size = (
        map.get("width").and_then(Value::as_f64),
        map.get("height").and_then(Value::as_f64),
    );
    let (Some(width), Some(height)) = size else {
        report.note("path viewBox");
        return;
    };
    if in_flow || vb_w <= 0.0 || vb_h <= 0.0 {
        report.note("path viewBox");
        return;
    }
    let (sx, sy) = (width / vb_w, height / vb_h);
    let x = map.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let y = map.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    map.insert("x".to_owned(), (x + (bx - vb_x) * sx).into());
    map.insert("y".to_owned(), (y + (by - vb_y) * sy).into());
    map.insert("width".to_owned(), (bw * sx).into());
    map.insert("height".to_owned(), (bh * sy).into());
}

fn numbers4(value: &Value) -> Option<[f64; 4]> {
    let items = value.as_array()?;
    if items.len() != 4 {
        return None;
    }
    let mut out = [0.0; 4];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item.as_f64()?;
    }
    Some(out)
}
