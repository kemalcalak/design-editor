//! The live browser node (#20) on the editor canvas.
//!
//! The scene carries a browser as `NodeKind::Other("browser")` with its
//! frame, corner radius and stroke, and the address in the widget's
//! `value_str` (`de-pen-loader`'s `browser_to_payload`). The editor canvas
//! paints a page box clipped to the node's corners: the live frame a host
//! streams, else the page snapshot a host rendered (`browser_textures`), else a blank page with a globe and
//! the address; then the node's stroke. Drop shadows paint in the node
//! painter's shared shadow pass ([`is_painted`] opts the node in).
//!
//! Browsers are canvas-only, like annotations: they paint only while
//! `canvas_viewport` paints the page (inside
//! `canvas_annotation::with_annotations`) or the preview paints its
//! scene (inside `browser_preview::with_clip`, #324: there a page is an
//! embedded page, in the light look); PNG and PDF exports, screenshots
//! and mask sources leave them out.

use crate::layout_scene::{NodeKind, SceneNode};
use crate::widgets::canvas_annotation::editor_canvas_dark;
use crate::widgets::canvas_viewport_image::{cached_bytes_for, paint_cached_raster};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::widgets::{browser_preview, browser_textures};
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

/// Whether `node` is a browser the editor canvas or the preview is
/// painting now.
pub(super) fn is_painted(node: &SceneNode) -> bool {
    matches!(&node.kind, NodeKind::Other(tag) if is_browser_tag(tag)) && surface().is_some()
}

/// The surface that shows pages, while one paints.
#[derive(Clone, Copy)]
enum Surface {
    /// The editor canvas, in its theme (`true`: dark).
    Canvas(bool),
    /// The preview: always the light look (a page is white).
    Preview,
}

fn surface() -> Option<Surface> {
    editor_canvas_dark()
        .map(Surface::Canvas)
        .or_else(|| browser_preview::painting().map(|_| Surface::Preview))
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

/// Paint `node` as a page box when the editor canvas or the preview is
/// painting. Elsewhere nothing paints (the node is canvas-only).
pub(super) fn paint(cx: &mut PaintCx<'_>, node: &SceneNode, world_rect: Rect, zoom: f32) {
    let Some(surface) = surface() else {
        return;
    };
    let (dark, preview) = match surface {
        Surface::Canvas(dark) => (dark, false),
        Surface::Preview => (false, true),
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
    // Recorded only here, where the node is on screen: a culled browser
    // never reaches the host's render queue.
    let live = paint_live_frame(cx, node, world_rect);
    if preview {
        browser_preview::note(&node.id, world_rect, live == Some(true));
    } else {
        browser_textures::note_visible(&node.id, [world_rect.size.x, world_rect.size.y]);
    }
    let drawn = live.is_some()
        || browser_textures::ready(&node.id)
            .and_then(|(_, texture)| Some((texture, cached_bytes_for(texture)?)))
            .is_some_and(|(texture, bytes)| {
                paint_cached_raster(cx, world_rect, texture, &bytes, 0.0)
            });
    if !drawn {
        paint_placeholder(cx, node, world_rect, zoom, ink);
    }
    cx.backend.restore();
    super::canvas_viewport_stroke_paint::paint_node_stroke(cx, node, world_rect, zoom);
}

/// The newest frame of the node's live stream (#324), or the last one kept
/// after the stream stopped, when its raster is resident. A running
/// stream's box is recorded so a host that keeps the canvas in a retained
/// layer repaints only this node when the next frame lands. Returns
/// whether its stream still runs, `None` when no frame was drawn.
fn paint_live_frame(cx: &mut PaintCx<'_>, node: &SceneNode, world_rect: Rect) -> Option<bool> {
    let (texture, streaming) = browser_textures::live_for_paint(&node.id)?;
    if !cx.backend.image_resident(texture) {
        return None;
    }
    cx.backend
        .draw_image_with_options_transform_blend_and_tile_scale(
            world_rect,
            texture,
            &[],
            crate::ImageDrawMode::Stretch,
            crate::ImageAdjustments::default(),
            1.0,
            0.0,
            None,
            crate::ImageBlendMode::Normal,
            None,
            1.0,
        );
    if streaming {
        super::canvas_animation_regions::record_live(world_rect);
    }
    Some(streaming)
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
    // Why the page is missing (#283), under the address.
    let reason = label.as_ref().and(browser_textures::failure(&node.id));
    let label_size = LABEL_SIZE * zoom;
    let gap = 8.0 * zoom;
    let block_h = glyph
        + label.as_ref().map_or(0.0, |_| gap + label_size)
        + reason.map_or(0.0, |_| gap * 0.5 + label_size);
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
        let layout = TextLayout::single_run(
            &label,
            text_metrics::CHROME_FONT_FAMILY,
            label_size,
            ink.to_jian(),
            Point2D::ZERO,
        );
        let width =
            text_metrics::measure_chrome(cx.backend, &label, label_size).min(world_rect.size.x);
        let label_y = top + glyph + gap + label_size * 0.8;
        cx.backend
            .draw_text(&layout, Point2D::new(centre_x - width * 0.5, label_y));
        if let Some(reason) = reason {
            let layout = TextLayout::single_run(
                reason,
                text_metrics::CHROME_FONT_FAMILY,
                label_size,
                ink.to_jian(),
                Point2D::ZERO,
            );
            let width =
                text_metrics::measure_chrome(cx.backend, reason, label_size).min(world_rect.size.x);
            cx.backend.draw_text(
                &layout,
                Point2D::new(centre_x - width * 0.5, label_y + gap * 0.5 + label_size),
            );
        }
    }
}

#[cfg(test)]
#[path = "canvas_browser_tests.rs"]
mod tests;
