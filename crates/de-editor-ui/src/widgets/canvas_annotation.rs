//! Note / context / prompt annotations (#12) on the editor canvas.
//!
//! The scene carries an annotation as `NodeKind::Other("note" | "context" |
//! "prompt")` with its content in `text` and a prompt's model in the widget
//! label (`de-pen-loader`'s `annotation_to_payload`). The editor canvas
//! paints it as a sticky note: a tinted card with a soft shadow, the kind's
//! glyph (and the model) in a header row, and the content wrapped below.
//!
//! Annotations are canvas-only. `canvas_viewport` paints the page inside
//! [`with_annotations`]; every other caller of the node painter (PNG and
//! PDF exports, screenshots, mask sources) leaves them out, like the
//! in-progress overlay of `canvas_placeholder`.
//!
//! The body text paints through the ordinary text painter on the node
//! [`annotation_body`] derives, which the native host also uses for the
//! inline editor's caret geometry, so typing into a note lines up with the
//! painted glyphs.

use std::cell::Cell;

use super::canvas_viewport::EditCaret;
use crate::layout_scene::{NodeKind, SceneNode, SceneTextVerticalAlign};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// Inner padding of the card (doc px).
const PAD: f32 = 12.0;
/// Header row height: glyph and model label (doc px).
const HEADER_H: f32 = 30.0;
const GLYPH: f32 = 14.0;
const LABEL_SIZE: f32 = 11.0;
const RADIUS: f32 = 6.0;

thread_local! {
    /// `Some(dark)` while the editor canvas paints a page.
    static CANVAS_THEME: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Run `paint` with annotations shown in the light or dark look.
pub(crate) fn with_annotations<R>(dark: bool, paint: impl FnOnce() -> R) -> R {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CANVAS_THEME.with(|theme| theme.set(self.0));
        }
    }
    let _restore = Restore(CANVAS_THEME.with(|theme| theme.replace(Some(dark))));
    paint()
}

/// Whether a scene `kind` tag names an annotation.
pub fn is_annotation_tag(tag: &str) -> bool {
    crate::canvas_only::is_canvas_only_tag(tag)
}

/// Whether `node` is a note / context / prompt annotation.
pub fn is_annotation(node: &SceneNode) -> bool {
    matches!(&node.kind, NodeKind::Other(tag) if is_annotation_tag(tag))
}

struct Palette {
    paper: Color,
    edge: Color,
    ink: Color,
    accent: Color,
}

fn rgb(hex: u32) -> Color {
    Color::rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn palette(tag: &str, dark: bool) -> Palette {
    let (paper, edge, ink, accent) = match (tag, dark) {
        ("context", false) => (0xE0F2FE, 0xBAE6FD, 0x0C3B57, 0x0284C7),
        ("context", true) => (0x13304A, 0x1E4D73, 0xD6EEFC, 0x38BDF8),
        ("prompt", false) => (0xEDE9FE, 0xDDD6FE, 0x2E1F5E, 0x7C3AED),
        ("prompt", true) => (0x2A2352, 0x41367A, 0xE7E2FD, 0xA78BFA),
        (_, false) => (0xFEF3C7, 0xFDE68A, 0x423510, 0xD97706),
        (_, true) => (0x3A3118, 0x5C4B1C, 0xFCEFC7, 0xFBBF24),
    };
    Palette {
        paper: rgb(paper),
        edge: rgb(edge),
        ink: rgb(ink),
        accent: rgb(accent),
    }
}

fn glyph(tag: &str) -> Icon {
    match tag {
        "context" => Icon::FileText,
        "prompt" => Icon::Sparkles,
        _ => Icon::StickyNote,
    }
}

/// The text node the content paints (and is edited) as: the annotation's
/// own id and type face, set inside the card below the header row.
pub fn annotation_body(node: &SceneNode) -> Option<SceneNode> {
    if !is_annotation(node) {
        return None;
    }
    let mut body = node.clone();
    body.kind = NodeKind::Text;
    body.children.clear();
    body.widget = None;
    body.text_wrap = true;
    body.text_vertical_align = SceneTextVerticalAlign::Top;
    body.stroke = None;
    body.fill_layers.clear();
    body.effects.clear();
    body.bounds = Rect {
        origin: Point2D::new(node.bounds.origin.x + PAD, node.bounds.origin.y + HEADER_H),
        size: Point2D::new(
            (node.bounds.size.x - 2.0 * PAD).max(0.0),
            (node.bounds.size.y - HEADER_H - PAD).max(0.0),
        ),
    };
    Some(body)
}

/// Paint `node` as a sticky note when the editor canvas is painting.
/// Returns whether it painted (false outside the canvas or for another kind).
pub(super) fn paint(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    viewport_origin: Point2D,
    zoom: f32,
    edit_caret: &Option<EditCaret>,
) -> bool {
    let NodeKind::Other(tag) = &node.kind else {
        return false;
    };
    let Some(dark) = CANVAS_THEME.with(Cell::get) else {
        return false;
    };
    if !is_annotation_tag(tag) {
        return false;
    }
    let colors = palette(tag, dark);
    let radius = RADIUS * zoom;
    let lift = 2.0 * zoom;
    let shadow = Rect {
        origin: Point2D::new(world_rect.origin.x, world_rect.origin.y + lift),
        size: world_rect.size,
    };
    let shadow_alpha = if dark { 0.45 } else { 0.12 };
    cx.backend.fill_drop_shadow(
        shadow,
        radius,
        6.0 * zoom,
        Color::rgba_u8(0, 0, 0, shadow_alpha),
    );
    cx.backend.fill_round_rect(world_rect, radius, colors.paper);
    cx.backend
        .stroke_round_rect(world_rect, radius, colors.edge, 1.0);

    cx.backend.save();
    cx.backend.clip_rect(world_rect);
    let glyph_size = GLYPH * zoom;
    let glyph_origin = Point2D::new(
        world_rect.origin.x + PAD * zoom,
        world_rect.origin.y + (HEADER_H - GLYPH) * 0.5 * zoom,
    );
    draw_icon(
        cx.backend,
        glyph(tag),
        glyph_origin,
        glyph_size,
        colors.accent,
        1.5,
    );
    if let Some(model) = node.widget.as_ref().and_then(|w| w.label.as_deref()) {
        let size = LABEL_SIZE * zoom;
        let layout =
            TextLayout::single_run(model, "", size, colors.accent.to_jian(), Point2D::ZERO);
        let top = world_rect.origin.y + (HEADER_H - LABEL_SIZE) * 0.5 * zoom;
        cx.backend.draw_text(
            &layout,
            Point2D::new(glyph_origin.x + glyph_size + 6.0 * zoom, top + size * 0.8),
        );
    }
    if let Some(mut body) = annotation_body(node) {
        body.fill = Some(colors.ink);
        let body_world = Rect {
            origin: Point2D::new(
                viewport_origin.x + body.bounds.origin.x * zoom,
                viewport_origin.y + body.bounds.origin.y * zoom,
            ),
            size: Point2D::new(body.bounds.size.x * zoom, body.bounds.size.y * zoom),
        };
        super::canvas_viewport_text_fill::paint_text_in_viewport(
            cx,
            &body,
            body_world,
            viewport_origin,
            zoom,
            edit_caret,
        );
    }
    cx.backend.restore();
    true
}

#[cfg(test)]
#[path = "canvas_annotation_tests.rs"]
mod tests;
