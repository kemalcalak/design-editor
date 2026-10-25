//! Design tree → [`Element`] tree.
//!
//! Children keep their array order, which is the auto-layout flow order. The
//! canvas paints the FIRST child on top, while HTML paints later siblings on
//! top, so wherever siblings can overlap (a stack, or a parent with
//! absolutely positioned children) each child gets a descending `z-index`.

use super::assets::Assets;
use super::ir::{Content, Decls, Element};
use super::layout::{
    apply_absolute, apply_container, apply_size, child_parent, is_absolute, node_sizing, Parent,
};
use super::paint::{
    apply_background, apply_compositing, apply_effects, apply_stroke, fills_of, node_effects,
    radius_css,
};
use super::shapes::{line_svg, path_svg, polygon_svg};
use super::text::{apply_text, inlines, link_of, needs_inner_wrapper, node_link, text_sizing};
use super::value::{color, font_family, px, slug};
use crate::codegen_targets::codegen_motion_css;
use crate::codegen_targets::{image_object_fit, tabs_nav_markup, widget_markup};
use crate::{node_children, node_hidden};
use de_editor_core::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::{ContainerProps, ImageFitMode, PenNode, ScriptNode};
use jian_ops_schema::sizing::SizingBehavior;

pub(crate) struct Walker<'a> {
    pub assets: &'a mut Assets,
    /// Write `data-name` / `data-id` on every element.
    pub layer_data: bool,
    /// Inside an `<a>`: nested links are dropped (`<a>` cannot nest).
    pub in_link: bool,
}

fn kind(node: &PenNode) -> &'static str {
    match node {
        PenNode::Frame(_) | PenNode::Script(_) => "frame",
        PenNode::Group(_) => "group",
        PenNode::Rectangle(_) => "rect",
        PenNode::Ellipse(_) => "ellipse",
        PenNode::Line(_) => "line",
        PenNode::Polygon(_) => "polygon",
        PenNode::Path(_) => "path",
        PenNode::Text(_) => "text",
        PenNode::Image(_) => "image",
        PenNode::IconFont(_) => "icon",
        PenNode::Ref(_) => "ref",
        PenNode::TextInput(_) => "input",
        PenNode::TextArea(_) => "textarea",
        PenNode::NumberInput(_) => "number-input",
        PenNode::Select(_) => "select",
        PenNode::Switch(_) => "switch",
        PenNode::Checkbox(_) => "checkbox",
        PenNode::Slider(_) => "slider",
        PenNode::RadioGroup(_) => "radio-group",
        PenNode::Progress(_) => "progress",
        PenNode::Tabs(_) => "tabs",
        // Never emitted: `node_hidden` skips canvas-only nodes (#12).
        PenNode::Note(_) | PenNode::Context(_) | PenNode::Prompt(_) => "note",
        PenNode::Browser(_) => "browser",
    }
}

fn container_of(node: &PenNode) -> Option<&ContainerProps> {
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => Some(&n.container),
        PenNode::Group(n) => Some(&n.container),
        PenNode::Rectangle(n) => Some(&n.container),
        _ => None,
    }
}

/// The node's literal box, when both sides are numbers.
fn literal_size(node: &PenNode) -> Option<(f64, f64)> {
    match node_sizing(node) {
        (Some(SizingBehavior::Number(w)), Some(SizingBehavior::Number(h)), _) => Some((*w, *h)),
        _ => None,
    }
}

/// Motion declarations the shared motion module writes inline.
fn apply_motion(decls: &mut Decls, node: &PenNode) {
    for chunk in codegen_motion_css::inline_style(node).split(';') {
        let Some((prop, value)) = chunk.split_once(':') else {
            continue;
        };
        let prop: &'static str = match prop.trim() {
            "transition" => "transition",
            "animation" => "animation",
            "animation-timeline" => "animation-timeline",
            "animation-range" => "animation-range",
            _ => continue,
        };
        decls.set(prop, value.trim().to_string());
    }
}

impl Walker<'_> {
    pub fn roots(&mut self, nodes: &[PenNode]) -> Vec<Element> {
        nodes
            .iter()
            .filter_map(|n| self.element(n, Parent::Root, None))
            .collect()
    }

    /// `z` is this child's `z-index` when its siblings may overlap.
    fn element(&mut self, node: &PenNode, parent: Parent, z: Option<usize>) -> Option<Element> {
        if node_hidden(node) {
            return None;
        }
        let base = node.base();
        let hint = base
            .name
            .as_deref()
            .map(slug)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kind(node).to_string());
        let mut el = Element::new("div", hint.clone());
        let absolute = is_absolute(base, parent);
        if absolute {
            apply_absolute(&mut el.decls, base);
        } else if parent == Parent::Stack {
            el.decls.set("grid-area", "1 / 1");
        }
        if let Some(z) = z {
            if !absolute {
                el.decls.set("position", "relative");
            }
            el.decls.set("z-index", z.to_string());
        }
        let (w, h, limits) = node_sizing(node);
        let (w, h) = match node {
            PenNode::Text(t) => text_sizing(t),
            _ => (w, h),
        };
        apply_size(&mut el.decls, w, h, limits, parent, absolute);
        let size = literal_size(node);
        let mut extra_shadows = Vec::new();

        match node {
            PenNode::Frame(_) | PenNode::Script(_) | PenNode::Group(_) | PenNode::Rectangle(_) => {
                let c = container_of(node).expect("container variant");
                apply_background(&mut el.decls, fills_of(node), size, self.assets, &hint);
                extra_shadows = apply_stroke(&mut el.decls, node);
                if let Some(r) = radius_css(&c.corner_radius) {
                    el.decls.set("border-radius", r);
                }
                if c.clip_content == Some(true) {
                    el.decls.set("overflow", "hidden");
                }
                let link = self.link(link_of(node));
                if let Some(href) = &link {
                    el.tag = "a";
                    el.attrs.push(("href".into(), href.clone()));
                    el.decls.set("text-decoration", "none");
                    el.decls.set("color", "inherit");
                }
                let children = node_children(node);
                if !children.is_empty() {
                    apply_container(&mut el.decls, c);
                    if el.decls.get("position").is_none() {
                        el.decls.set("position", "relative");
                    }
                    let outer = self.in_link;
                    self.in_link = outer || link.is_some();
                    el.content = Content::Children(self.children(children, child_parent(c)));
                    self.in_link = outer;
                }
            }
            PenNode::Ellipse(_) => {
                apply_background(&mut el.decls, fills_of(node), size, self.assets, &hint);
                extra_shadows = apply_stroke(&mut el.decls, node);
                el.decls.set("border-radius", "50%");
            }
            PenNode::Text(t) => {
                apply_text(&mut el.decls, t, self.assets, &hint);
                el.tag = "p";
                if let Some(href) = self.link(node_link(t)) {
                    el.tag = "a";
                    el.attrs.push(("href".into(), href));
                    if el.decls.get("display").is_none() {
                        el.decls.set("display", "block");
                    }
                }
                let mut runs = inlines(t);
                if self.in_link || el.tag == "a" {
                    runs.iter_mut().for_each(|r| r.href = None);
                } else {
                    runs.iter_mut()
                        .for_each(|r| r.href = r.href.take().and_then(|h| safe_href(&h)));
                }
                el.content = if needs_inner_wrapper(&el.decls) {
                    let mut inner = Element::new("span", format!("{hint}-inner"));
                    inner.content = Content::Text(runs);
                    Content::Children(vec![inner])
                } else {
                    Content::Text(runs)
                };
            }
            PenNode::Image(img) => {
                el.tag = "img";
                let src = self.assets.url(img.src.as_str(), &hint);
                el.attrs.push(("src".into(), src));
                el.attrs
                    .push(("alt".into(), base.name.clone().unwrap_or_default()));
                let fit = match img.object_fit {
                    Some(ImageFitMode::Fill) | None => "cover",
                    _ => image_object_fit(node).unwrap_or("cover"),
                };
                el.decls.set("object-fit", fit);
                el.decls.set("display", "block");
                if let Some(r) = radius_css(&img.corner_radius) {
                    el.decls.set("border-radius", r);
                }
            }
            PenNode::IconFont(icon) => {
                el.tag = "span";
                el.attrs.push(("role".into(), "img".into()));
                el.attrs
                    .push(("aria-label".into(), icon.icon_font_name.clone()));
                el.attrs
                    .push(("data-icon".into(), icon.icon_font_name.clone()));
                if let Some(family) = icon.icon_font_family.as_deref() {
                    el.attrs
                        .push(("data-icon-family".into(), family.to_string()));
                    if family.to_ascii_lowercase().starts_with("material") {
                        el.decls
                            .set("font-family", font_family("Material Symbols Outlined"));
                        el.content = Content::Text(vec![super::ir::Inline {
                            text: icon.icon_font_name.clone(),
                            decls: Decls::default(),
                            href: None,
                        }]);
                    }
                }
                el.decls.set("display", "inline-block");
                if let Some(c) = first_solid(node) {
                    el.decls.set("color", c);
                }
            }
            PenNode::Path(p) => {
                if let Some((view_box, inner)) = path_svg(node, p) {
                    svg(&mut el, view_box, inner);
                }
            }
            PenNode::Polygon(p) => {
                let (view_box, inner) = polygon_svg(node, p);
                svg(&mut el, view_box, inner);
            }
            PenNode::Line(l) => {
                let (lw, lh, view_box, inner) = line_svg(node, l);
                el.decls.set("width", px(lw));
                el.decls.set("height", px(lh));
                el.decls.set("flex-shrink", "0");
                svg(&mut el, view_box, inner);
            }
            PenNode::Tabs(t) => {
                apply_background(&mut el.decls, fills_of(node), size, self.assets, &hint);
                extra_shadows = apply_stroke(&mut el.decls, node);
                let mut kids = Vec::new();
                if let Some(nav) = tabs_nav_markup(node) {
                    kids.push(Element::raw(nav));
                }
                // The canvas shows only the active tab's panel.
                let active = t
                    .tabs
                    .iter()
                    .flatten()
                    .position(|tab| t.value.as_deref() == Some(tab.value.as_str()))
                    .unwrap_or(0);
                let mut panels = self.children(
                    t.children.as_deref().unwrap_or(&[]),
                    Parent::Flow {
                        horizontal: false,
                        auto_layout: true,
                    },
                );
                for (i, panel) in panels.iter_mut().enumerate() {
                    if i != active {
                        panel.attrs.push(("hidden".into(), String::new()));
                    }
                }
                kids.extend(panels);
                el.decls.set("display", "flex");
                el.decls.set("flex-direction", "column");
                el.decls.set("align-items", "flex-start");
                el.content = Content::Children(kids);
            }
            PenNode::Ref(_) => return None,
            _ => {
                apply_background(&mut el.decls, fills_of(node), size, self.assets, &hint);
                extra_shadows = apply_stroke(&mut el.decls, node);
                if let Some(widget) = widget_markup(node) {
                    el.content = Content::Raw(widget);
                }
            }
        }

        apply_effects(
            &mut el.decls,
            node_effects(node),
            matches!(node, PenNode::Text(_)),
            extra_shadows,
        );
        apply_compositing(&mut el.decls, node);
        apply_motion(&mut el.decls, node);
        if let Some(id) = codegen_motion_css::motion_attr_value(node) {
            el.attrs.push(("data-de-motion-node".into(), id));
        }
        if self.layer_data {
            if let Some(name) = base.name.as_deref().filter(|n| !n.is_empty()) {
                el.attrs.push(("data-name".into(), name.to_string()));
            }
            el.attrs.push(("data-id".into(), base.id.clone()));
        }
        Some(el)
    }

    /// A node's link, unless it sits inside another link or uses a scheme
    /// that should not ship in a published page.
    fn link(&self, href: Option<String>) -> Option<String> {
        if self.in_link {
            return None;
        }
        safe_href(&href?)
    }

    fn children(&mut self, children: &[PenNode], parent: Parent) -> Vec<Element> {
        let visible: Vec<&PenNode> = children.iter().filter(|c| !node_hidden(c)).collect();
        let overlap = visible.len() > 1
            && (parent == Parent::Stack || visible.iter().any(|c| is_absolute(c.base(), parent)));
        let count = visible.len();
        visible
            .into_iter()
            .enumerate()
            .filter_map(|(i, child)| {
                let z = overlap.then_some(count - i);
                self.element(child, parent, z)
            })
            .collect()
    }
}

/// Links keep web, mail, phone, fragment and relative targets; other schemes
/// (`javascript:`, `data:`, …) are dropped.
fn safe_href(href: &str) -> Option<String> {
    let h = href.trim();
    let scheme = h
        .split_once(':')
        .map(|(s, _)| s)
        .filter(|s| !s.contains(['/', '?', '#']));
    match scheme.map(str::to_ascii_lowercase).as_deref() {
        None | Some("http" | "https" | "mailto" | "tel") => Some(h.to_string()),
        _ => None,
    }
}

fn first_solid(node: &PenNode) -> Option<String> {
    fills_of(node).iter().find_map(|f| match f {
        jian_ops_schema::style::PenFill::Solid(b) => Some(color(&b.color)),
        _ => None,
    })
}

fn svg(el: &mut Element, view_box: String, inner: String) {
    el.tag = "svg";
    el.attrs.push(("viewBox".into(), view_box));
    el.attrs.push(("preserveAspectRatio".into(), "none".into()));
    el.decls.set("overflow", "visible");
    el.decls.set("display", "block");
    el.content = Content::Raw(inner);
}
