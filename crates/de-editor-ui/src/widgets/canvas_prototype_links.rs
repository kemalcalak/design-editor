//! Prototype links on the editor canvas (#371): a curved arrow from each
//! interactive node to the screen (or section) it navigates to, labelled
//! with its trigger; Back and Open URL show a small badge on the node
//! instead, and a broken link draws red.
//!
//! The links come from `de_editor_core::interaction_graph` for the active
//! page (instances expanded, so a master's link shows once per instance
//! from the instance's node), cached per document revision. They show for
//! the selection (the selected nodes and everything inside them), or for
//! the whole page with View ▸ Show prototype links. The canvas paints them
//! in screen space over the nodes, so they follow pan and zoom; exports
//! and screenshots never build a `CanvasViewport`, so they never show
//! there. Off-screen links are culled.

use std::cell::RefCell;
use std::rc::Rc;

use de_editor_core::interaction_edit::InteractionTrigger;
use de_editor_core::interaction_graph::{flow_graph, FlowAction};
use de_editor_core::EditorState;

use crate::layout_scene::ScenePage;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// What a link shows on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasLinkKind {
    /// An arrow to `target` (a screen frame or a scroll-to section).
    Arrow { target: String },
    /// A Back badge on the node.
    Back,
    /// An Open URL badge with the host (`example.com`).
    OpenUrl { host: String },
    /// A link that goes nowhere: a red badge on the node.
    Broken,
}

/// One link of the active page, ready to paint.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasLink {
    /// Canvas id of the node the link starts at.
    pub anchor: String,
    pub trigger: InteractionTrigger,
    pub kind: CanvasLinkKind,
    /// The anchors of the node and its ancestors: the selection filter.
    pub scope: Vec<String>,
}

/// What the cached links were built from: the document's identity and
/// revision, and the page (index and its top-level count, so a second
/// editor state in the same thread cannot be served a stale list).
type CacheKey = (u64, u64, usize, usize);

thread_local! {
    static CACHE: RefCell<Option<(CacheKey, Rc<Vec<CanvasLink>>)>> = const { RefCell::new(None) };
}

/// Every link of the active page, rebuilt only when the document or the
/// page changed.
pub fn page_links(state: &EditorState) -> Rc<Vec<CanvasLink>> {
    let key = (
        state.document_generation(),
        state.document_revision(),
        state.ui.active_page_index,
        state.active_children().len(),
    );
    if let Some(hit) = CACHE.with(|cache| {
        cache
            .borrow()
            .as_ref()
            .filter(|(cached, _)| *cached == key)
            .map(|(_, links)| links.clone())
    }) {
        return hit;
    }
    let links = Rc::new(build(state));
    CACHE.with(|cache| *cache.borrow_mut() = Some((key, links.clone())));
    links
}

fn build(state: &EditorState) -> Vec<CanvasLink> {
    let page_id = match state.doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages
            .get(state.ui.active_page_index)
            .map(|page| page.id.clone()),
        _ => Some("0".to_owned()),
    };
    let Some(page_id) = page_id else {
        return Vec::new();
    };
    flow_graph(&state.doc, Some(&page_id))
        .links
        .into_iter()
        .filter(|link| !link.hidden)
        .filter_map(|link| {
            let kind = if link.broken.is_some() {
                CanvasLinkKind::Broken
            } else {
                match link.action {
                    FlowAction::Back => CanvasLinkKind::Back,
                    FlowAction::OpenUrl => CanvasLinkKind::OpenUrl {
                        host: url_host(link.to.as_deref().unwrap_or_default()),
                    },
                    FlowAction::Navigate | FlowAction::ScrollTo => {
                        // A target on another page (a master's link) has
                        // nothing to point at here.
                        if link.target_page_id.as_deref() != Some(page_id.as_str()) {
                            return None;
                        }
                        CanvasLinkKind::Arrow {
                            target: link.target_id?,
                        }
                    }
                    FlowAction::Other(_) => return None,
                }
            };
            Some(CanvasLink {
                anchor: link.anchor,
                trigger: link.trigger,
                kind,
                scope: link.scope,
            })
        })
        .collect()
}

/// The links the canvas shows: all of them with the View toggle on, else
/// those starting at or inside a selected node.
pub fn visible_links(state: &EditorState) -> Vec<CanvasLink> {
    let show_all = state.editor_ui.prototype.show_links;
    if !show_all && state.selection.set.is_empty() && !state.selection.anchor.is_real() {
        return Vec::new();
    }
    let selected: Vec<&str> = state
        .selection
        .set
        .iter()
        .map(|id| id.as_str())
        .chain(std::iter::once(state.selection.anchor.as_str()))
        .filter(|id| !id.is_empty())
        .collect();
    page_links(state)
        .iter()
        .filter(|link| show_all || link.scope.iter().any(|id| selected.contains(&id.as_str())))
        .cloned()
        .collect()
}

/// `https://www.example.com/a` → `example.com`.
pub fn url_host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_owned()
}

const LINK: Color = Color {
    r: 0.231,
    g: 0.51,
    b: 0.965,
    a: 1.0,
};
const BROKEN: Color = Color {
    r: 0.898,
    g: 0.2,
    b: 0.2,
    a: 1.0,
};
const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const LABEL_FONT: f32 = 10.0;
const CURVE_STEPS: usize = 24;

/// Paint `links` over `page`, in screen space.
pub(super) fn paint(
    cx: &mut PaintCx<'_>,
    page: &ScenePage,
    links: &[CanvasLink],
    canvas: Rect,
    viewport: &de_editor_core::Viewport,
    locale: de_editor_core::Locale,
) {
    let to_screen = |r: Rect| Rect {
        origin: Point2D::new(
            canvas.origin.x + viewport.pan_x + r.origin.x * viewport.zoom,
            canvas.origin.y + viewport.pan_y + r.origin.y * viewport.zoom,
        ),
        size: Point2D::new(r.size.x * viewport.zoom, r.size.y * viewport.zoom),
    };
    let cull = Rect {
        origin: Point2D::new(canvas.origin.x - 64.0, canvas.origin.y - 64.0),
        size: Point2D::new(canvas.size.x + 128.0, canvas.size.y + 128.0),
    };
    // Curves first, then every label on top of them. A node's labels
    // (an arrow's trigger, a Back / Open URL / broken badge) stack down
    // its right edge so several links on one node stay readable.
    let mut labels: Vec<(Rect, String, String, Color)> = Vec::new();
    for link in links {
        let Some(node) = page.find(&link.anchor).filter(|n| !n.hidden) else {
            continue;
        };
        let from = to_screen(node.bounds);
        let label = trigger_label(locale, link.trigger);
        let (text, color) = match &link.kind {
            CanvasLinkKind::Arrow { target } => {
                let Some(target) = page.find(target) else {
                    continue;
                };
                let (start, end) = endpoints(from, to_screen(target.bounds));
                if !intersects(cull, bbox(start, end)) {
                    continue;
                }
                paint_curve(cx, start, end, LINK);
                (label.to_owned(), LINK)
            }
            CanvasLinkKind::Back => (
                format!("{label} · {}", t(locale, "prototype.back", "Back")),
                LINK,
            ),
            CanvasLinkKind::OpenUrl { host } => (format!("{label} · ↗ {host}"), LINK),
            CanvasLinkKind::Broken => (
                format!(
                    "{label} · {}",
                    t(locale, "prototype.brokenLink", "Broken link")
                ),
                BROKEN,
            ),
        };
        if intersects(cull, from) {
            labels.push((from, link.anchor.clone(), text, color));
        }
    }
    let mut stacks: Vec<(&str, f32)> = Vec::new();
    for (from, anchor, text, color) in &labels {
        let offset = match stacks.iter_mut().find(|(id, _)| id == anchor) {
            Some((_, y)) => y,
            None => {
                stacks.push((anchor.as_str(), 0.0));
                &mut stacks.last_mut().expect("pushed").1
            }
        };
        let origin = Point2D::new(from.origin.x + from.size.x + 6.0, from.origin.y + *offset);
        *offset += paint_badge(cx, origin, text, *color) + 4.0;
    }
}

/// Start on the source's side facing the target, end on the target's
/// facing edge just under its top, where the frame label does not sit.
fn endpoints(from: Rect, to: Rect) -> (Point2D, Point2D) {
    let from_mid_y = from.origin.y + from.size.y / 2.0;
    let end_y = to.origin.y + (to.size.y / 2.0).min(24.0);
    if to.origin.x >= from.origin.x + from.size.x / 2.0 {
        (
            Point2D::new(from.origin.x + from.size.x, from_mid_y),
            Point2D::new(to.origin.x, end_y),
        )
    } else {
        (
            Point2D::new(from.origin.x, from_mid_y),
            Point2D::new(to.origin.x + to.size.x, end_y),
        )
    }
}

fn bbox(a: Point2D, b: Point2D) -> Rect {
    let min = Point2D::new(a.x.min(b.x), a.y.min(b.y));
    Rect {
        origin: min,
        size: Point2D::new((a.x - b.x).abs().max(1.0), (a.y - b.y).abs().max(1.0)),
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.origin.x < b.origin.x + b.size.x
        && b.origin.x < a.origin.x + a.size.x
        && a.origin.y < b.origin.y + b.size.y
        && b.origin.y < a.origin.y + a.size.y
}

/// A horizontal S-curve from `start` to `end`, ending in an arrowhead.
fn paint_curve(cx: &mut PaintCx<'_>, start: Point2D, end: Point2D, color: Color) {
    let dir = if end.x >= start.x { 1.0 } else { -1.0 };
    let reach = ((end.x - start.x).abs() / 2.0).max(40.0);
    let c1 = Point2D::new(start.x + dir * reach, start.y);
    let c2 = Point2D::new(end.x - dir * reach, end.y);
    let at = |t: f32| {
        let u = 1.0 - t;
        Point2D::new(
            u * u * u * start.x
                + 3.0 * u * u * t * c1.x
                + 3.0 * u * t * t * c2.x
                + t * t * t * end.x,
            u * u * u * start.y
                + 3.0 * u * u * t * c1.y
                + 3.0 * u * t * t * c2.y
                + t * t * t * end.y,
        )
    };
    let mut previous = start;
    for step in 1..=CURVE_STEPS {
        let point = at(step as f32 / CURVE_STEPS as f32);
        cx.backend.stroke_line(previous, point, color, 1.5);
        previous = point;
    }
    cx.backend
        .fill_oval(Rect::xywh(start.x - 3.0, start.y - 3.0, 6.0, 6.0), color);
    // The curve arrives horizontally, so the head points along x.
    let head = [
        end,
        Point2D::new(end.x - dir * 8.0, end.y - 4.5),
        Point2D::new(end.x - dir * 8.0, end.y + 4.5),
    ];
    cx.backend.fill_polygon(&head, color);
}

/// A filled pill with white text at `origin` (top-left); returns its
/// height.
fn paint_badge(cx: &mut PaintCx<'_>, origin: Point2D, text: &str, color: Color) -> f32 {
    const PAD_X: f32 = 5.0;
    const HEIGHT: f32 = 15.0;
    let width = cx.backend.measure_text_family(
        text,
        LABEL_FONT,
        crate::widgets::text_metrics::CHROME_FONT_FAMILY,
    ) + PAD_X * 2.0;
    let rect = Rect::xywh(origin.x, origin.y, width, HEIGHT);
    cx.backend.fill_round_rect(rect, HEIGHT / 2.0, color);
    let layout = TextLayout::single_run(
        text,
        crate::widgets::text_metrics::CHROME_FONT_FAMILY,
        LABEL_FONT,
        WHITE.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend
        .draw_text(&layout, Point2D::new(origin.x + PAD_X, origin.y + 11.0));
    HEIGHT
}

fn trigger_label(locale: de_editor_core::Locale, trigger: InteractionTrigger) -> &'static str {
    match trigger {
        InteractionTrigger::Tap => t(locale, "prototype.trigger.tap", "Tap"),
        InteractionTrigger::Hover => t(locale, "prototype.trigger.hover", "Hover"),
        InteractionTrigger::LongPress => t(locale, "prototype.trigger.longPress", "Long press"),
    }
}

fn t(locale: de_editor_core::Locale, key: &'static str, fallback: &'static str) -> &'static str {
    crate::widgets::property_panel_interactions::translate(locale, key, fallback)
}

#[cfg(test)]
#[path = "canvas_prototype_links_tests.rs"]
mod tests;
