//! The live browser node (#20) on the editor canvas.
//!
//! The scene carries a browser as `NodeKind::Other("browser")` with its
//! frame, corner radius and stroke, and the address in the widget's
//! `value_str` (`de-pen-loader`'s `browser_to_payload`). The editor canvas
//! paints a page box: a blank page with a globe and the address, clipped
//! to the node's corners, then the node's stroke. Drop shadows paint in the
//! node painter's shared shadow pass ([`is_painted`] opts the node in).
//!
//! Browsers are canvas-only, like annotations: they paint only while
//! `canvas_viewport` paints the page (inside
//! `canvas_annotation::with_annotations`); PNG and PDF exports,
//! screenshots and mask sources leave them out.

use crate::layout_scene::{NodeKind, SceneNode};
use crate::widgets::canvas_annotation::editor_canvas_dark;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// Globe glyph and address label size (doc px), capped on screen.
const GLYPH: f32 = 28.0;
const LABEL_SIZE: f32 = 13.0;
/// Below this on-screen width the label is left out (it would not fit).
const MIN_LABEL_W: f32 = 96.0;
/// Longest address shown before it is cut with an ellipsis.
const MAX_LABEL_CHARS: usize = 48;

/// Whether a scene `kind` tag names a browser node.
pub fn is_browser_tag(tag: &str) -> bool {
    tag == "browser"
}

/// Whether `node` is a browser the editor canvas is painting now.
pub(super) fn is_painted(node: &SceneNode) -> bool {
    matches!(&node.kind, NodeKind::Other(tag) if is_browser_tag(tag))
        && editor_canvas_dark().is_some()
}

/// The address the node shows, `None` when it has none.
pub fn browser_url(node: &SceneNode) -> Option<&str> {
    node.widget
        .as_ref()
        .filter(|w| w.kind == "browser")
        .and_then(|w| w.value_str.as_deref())
}

/// The address without its scheme, cut to [`MAX_LABEL_CHARS`].
fn display_url(url: &str) -> String {
    let bare = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .trim_end_matches('/');
    if bare.chars().count() <= MAX_LABEL_CHARS {
        return bare.to_string();
    }
    let mut cut: String = bare.chars().take(MAX_LABEL_CHARS - 1).collect();
    cut.push('…');
    cut
}

/// Paint `node` as a page box when the editor canvas is painting. Outside
/// the canvas nothing paints (the node is canvas-only).
pub(super) fn paint(cx: &mut PaintCx<'_>, node: &SceneNode, world_rect: Rect, zoom: f32) {
    let Some(dark) = editor_canvas_dark() else {
        return;
    };
    if world_rect.size.x <= 0.0 || world_rect.size.y <= 0.0 {
        return;
    }
    // A shade off the canvas surface (theme `canvas_surface`, #1B1B1B /
    // #F3F3F3) so an empty page box reads as a page.
    let (page, ink) = if dark {
        (
            Color::rgb_u8(0x2A, 0x2A, 0x2F),
            Color::rgb_u8(0x9C, 0x9C, 0xA6),
        )
    } else {
        (
            Color::rgb_u8(0xFF, 0xFF, 0xFF),
            Color::rgb_u8(0x71, 0x71, 0x7A),
        )
    };
    let radius = node.corner_radius * zoom;
    cx.backend.save();
    cx.backend.clip_round_rect(world_rect, radius);
    cx.backend.fill_rect(world_rect, page);
    paint_placeholder(cx, node, world_rect, zoom, ink);
    cx.backend.restore();
    super::canvas_viewport_stroke_paint::paint_node_stroke(cx, node, world_rect, zoom);
}

/// The globe and the address, centred in the box.
fn paint_placeholder(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    ink: Color,
) {
    let glyph = (GLYPH * zoom).min(world_rect.size.x.min(world_rect.size.y) * 0.5);
    let label = browser_url(node)
        .map(display_url)
        .filter(|_| world_rect.size.x >= MIN_LABEL_W);
    let label_size = LABEL_SIZE * zoom;
    let gap = 8.0 * zoom;
    let block_h = glyph + label.as_ref().map_or(0.0, |_| gap + label_size);
    let top = world_rect.origin.y + (world_rect.size.y - block_h) * 0.5;
    let centre_x = world_rect.origin.x + world_rect.size.x * 0.5;
    draw_icon(
        cx.backend,
        Icon::Globe,
        Point2D::new(centre_x - glyph * 0.5, top),
        glyph,
        ink,
        1.5,
    );
    if let Some(label) = label {
        let layout = TextLayout::single_run(&label, "", label_size, ink.to_jian(), Point2D::ZERO);
        let width = cx
            .backend
            .measure_text(&label, label_size)
            .min(world_rect.size.x);
        cx.backend.draw_text(
            &layout,
            Point2D::new(centre_x - width * 0.5, top + glyph + gap + label_size * 0.8),
        );
    }
}

#[cfg(test)]
#[path = "canvas_browser_tests.rs"]
mod tests;
