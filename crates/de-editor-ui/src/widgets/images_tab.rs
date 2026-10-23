//! The left rail's Images tab (#30): Search / Generate buttons over a
//! two-column grid of every image node on the active page.
//!
//! A cell click selects its node and frames it on the canvas; the buttons
//! hand off to the image-node popovers (`de_editor_core::document_images`).
//! Layout, paint and hit-test all run from [`layout`], so a click can never
//! land on a cell laid out differently from the one painted.

pub use de_editor_core::document_images::ImagesTabHit;
use de_editor_core::document_images::{
    apply_images_tab_action, document_images, DocumentImage, ImagesTabAction,
};
use de_editor_core::{EditorState, NodeId};

use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::host_canvas_geometry::{rail_body_rect, rail_tab_on_show};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel_image_preview::paint_image_source;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{ImageAdjustments, ImageDrawMode, Point2D, Rect, TextLayout, Theme};

const PAD: f32 = 12.0;
const BUTTON_TOP: f32 = 12.0;
const BUTTON_H: f32 = 32.0;
const BUTTON_GAP: f32 = 8.0;
const BUTTON_RADIUS: f32 = 8.0;
const BUTTON_FONT: f32 = 13.0;
const ICON: f32 = 15.0;
const ICON_GAP: f32 = 6.0;
/// Space between the buttons and the grid's clip edge.
const GRID_GAP_TOP: f32 = 14.0;
const COLS: usize = 2;
const CELL_GAP: f32 = 10.0;
const CELL_RADIUS: f32 = 8.0;
/// Image aspect inside a cell (height / width).
const THUMB_ASPECT: f32 = 0.75;
/// Name row under the thumbnail.
const LABEL_H: f32 = 22.0;
const LABEL_FONT: f32 = 12.0;

/// The tab's geometry for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagesTabLayout {
    pub search: Rect,
    pub generate: Rect,
    /// The scrolling region; cells outside it are clipped.
    pub grid: Rect,
    /// One rect per image, already offset by the scroll.
    pub cells: Vec<Rect>,
}

fn cell_size(body: Rect) -> (f32, f32) {
    let w = (body.size.x - PAD * 2.0 - CELL_GAP * (COLS as f32 - 1.0)) / COLS as f32;
    (w.max(0.0), w.max(0.0) * THUMB_ASPECT + LABEL_H)
}

fn grid_rect(body: Rect) -> Rect {
    let top = body.origin.y + BUTTON_TOP + BUTTON_H + GRID_GAP_TOP;
    Rect {
        origin: Point2D::new(body.origin.x, top),
        size: Point2D::new(body.size.x, (body.origin.y + body.size.y - top).max(0.0)),
    }
}

/// How far the grid can scroll for `count` images.
pub fn max_scroll(body: Rect, count: usize) -> f32 {
    let (_w, h) = cell_size(body);
    let rows = count.div_ceil(COLS) as f32;
    let content = rows * h + (rows - 1.0).max(0.0) * CELL_GAP + PAD;
    (content - grid_rect(body).size.y).max(0.0)
}

/// Lay the tab out in `body` (the rail below its tab row).
pub fn layout(body: Rect, count: usize, scroll: f32) -> ImagesTabLayout {
    let button_w = (body.size.x - PAD * 2.0 - BUTTON_GAP) / 2.0;
    let button = |i: f32| Rect {
        origin: Point2D::new(
            body.origin.x + PAD + i * (button_w + BUTTON_GAP),
            body.origin.y + BUTTON_TOP,
        ),
        size: Point2D::new(button_w.max(0.0), BUTTON_H),
    };
    let grid = grid_rect(body);
    let (w, h) = cell_size(body);
    let top = grid.origin.y - scroll.clamp(0.0, max_scroll(body, count));
    let cells = (0..count)
        .map(|i| {
            let row = (i / COLS) as f32;
            let col = (i % COLS) as f32;
            Rect {
                origin: Point2D::new(
                    body.origin.x + PAD + col * (w + CELL_GAP),
                    top + row * (h + CELL_GAP),
                ),
                size: Point2D::new(w, h),
            }
        })
        .collect();
    ImagesTabLayout {
        search: button(0.0),
        generate: button(1.0),
        grid,
        cells,
    }
}

/// Whether the Images tab owns the rail's body this frame.
pub fn images_tab_on_show(state: &EditorState) -> bool {
    rail_tab_on_show(state, de_editor_core::LeftPanelTab::Images)
}

fn frame_layout(state: &EditorState, body: Rect, images: &[DocumentImage]) -> ImagesTabLayout {
    layout(body, images.len(), state.editor_ui.images_tab_scroll.offset)
}

/// What `point` hits, when the tab is on show.
pub fn hit_test(state: &EditorState, body: Rect, point: Point2D) -> Option<ImagesTabHit> {
    if !images_tab_on_show(state) || !body.contains(point) {
        return None;
    }
    let images = document_images(state);
    let l = frame_layout(state, body, &images);
    if l.search.contains(point) {
        return Some(ImagesTabHit::Button(ImagesTabAction::Search));
    }
    if l.generate.contains(point) {
        return Some(ImagesTabHit::Button(ImagesTabAction::Generate));
    }
    let cell = l.grid.contains(point).then(|| {
        l.cells
            .iter()
            .zip(&images)
            .find(|(cell, _)| cell.contains(point))
            .map(|(_, image)| ImagesTabHit::Image(image.node_id.clone()))
    });
    Some(cell.flatten().unwrap_or(ImagesTabHit::Inside))
}

/// Apply a press the host has already let every text input go for.
/// Returns the node the host should frame on the canvas (camera only),
/// for a thumbnail press.
pub fn apply_hit(state: &mut EditorState, hit: ImagesTabHit, now_ms: u64) -> Option<NodeId> {
    match hit {
        ImagesTabHit::Button(action) => {
            apply_images_tab_action(state, action, now_ms);
            None
        }
        ImagesTabHit::Image(id) => {
            state.editor_ui.image_panel.close_popovers();
            state.set_single_selection(id.clone());
            Some(id)
        }
        ImagesTabHit::Inside => None,
    }
}

/// [`hit_test`] for a host that has not laid the rail out: the body rect
/// is only resolved once the tab is known to be on show, because that walk
/// is not free and this runs for every press anywhere in the editor.
pub fn hit_at(state: &EditorState, viewport_h: f32, point: Point2D) -> Option<ImagesTabHit> {
    if !images_tab_on_show(state) {
        return None;
    }
    hit_test(state, rail_body_rect(state, viewport_h), point)
}

/// Track the hovered button / cell. Returns `(owns_point, changed)` in the
/// shape of `cursor_hover_flow::prompt_center_hover`: the whole tab body is
/// cursor-owned chrome, so its gutters never leak hover to the canvas.
/// Runs on every cursor move, so it bails before any layout when the tab
/// is not on show.
pub fn update_hover(state: &mut EditorState, viewport_h: f32, point: Point2D) -> (bool, bool) {
    if !images_tab_on_show(state) {
        return (false, state.editor_ui.images_tab_hover.take().is_some());
    }
    let body = rail_body_rect(state, viewport_h);
    let owns_point = body.contains(point);
    let next = hit_test(state, body, point).filter(|h| *h != ImagesTabHit::Inside);
    let changed = state.editor_ui.images_tab_hover != next;
    state.editor_ui.images_tab_hover = next;
    (owns_point, changed)
}

/// The scroll bound for the current document, for the hosts' wheel path.
pub fn current_max_scroll(state: &EditorState, body: Rect) -> f32 {
    max_scroll(body, document_images(state).len())
}

/// Paint the tab into `body`. The rail's card and tab row are already down.
pub fn paint(cx: &mut PaintCx<'_>, state: &EditorState, body: Rect) {
    if !images_tab_on_show(state) {
        return;
    }
    let ui = &state.editor_ui;
    let theme = theme_for(ui);
    let images = document_images(state);
    let l = frame_layout(state, body, &images);
    let hover = ui.images_tab_hover.as_ref();

    for (rect, action, icon, key) in [
        (
            l.search,
            ImagesTabAction::Search,
            Icon::Search,
            "common.search",
        ),
        (
            l.generate,
            ImagesTabAction::Generate,
            Icon::Sparkles,
            "common.generate",
        ),
    ] {
        let hovered = hover == Some(&ImagesTabHit::Button(action));
        paint_button(cx, &theme, rect, icon, translate(ui, key), hovered);
    }

    if images.is_empty() {
        let text = translate(ui, "imagesTab.empty");
        let w = text_metrics::measure_chrome(cx.backend, text, LABEL_FONT);
        cx.backend.draw_text(
            &TextLayout::single_run(
                text,
                text_metrics::CHROME_FONT_FAMILY,
                LABEL_FONT,
                theme.muted_foreground.to_jian(),
                Point2D::ZERO,
            ),
            Point2D::new(
                l.grid.origin.x + (l.grid.size.x - w) / 2.0,
                l.grid.origin.y + 40.0,
            ),
        );
        return;
    }

    let selected = &state.selection.anchor;
    cx.backend.save();
    cx.backend.clip_rect(l.grid);
    for (cell, image) in l.cells.iter().zip(&images) {
        if cell.origin.y > l.grid.origin.y + l.grid.size.y
            || cell.origin.y + cell.size.y < l.grid.origin.y
        {
            continue;
        }
        let hovered = matches!(hover, Some(ImagesTabHit::Image(id)) if *id == image.node_id);
        paint_cell(
            cx,
            &theme,
            *cell,
            image,
            image.node_id == *selected,
            hovered,
        );
    }
    cx.backend.restore();
}

fn paint_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    icon: Icon,
    label: &str,
    hovered: bool,
) {
    let fill = if hovered {
        theme.button_hover
    } else {
        theme.muted
    };
    cx.backend.fill_round_rect(rect, BUTTON_RADIUS, fill);
    cx.backend
        .stroke_round_rect(rect, BUTTON_RADIUS, theme.border, 1.0);
    let label = text_metrics::fit_chrome(
        cx.backend,
        label,
        (rect.size.x - ICON - ICON_GAP - 16.0).max(0.0),
        BUTTON_FONT,
    );
    let label_w = text_metrics::measure_chrome_weighted(cx.backend, &label, BUTTON_FONT, 500);
    let x = rect.origin.x + (rect.size.x - ICON - ICON_GAP - label_w) / 2.0;
    draw_icon(
        cx.backend,
        icon,
        Point2D::new(x, rect.origin.y + (rect.size.y - ICON) / 2.0),
        ICON,
        theme.foreground,
        1.6,
    );
    cx.backend.draw_text(
        &TextLayout::single_run(
            &label,
            text_metrics::CHROME_FONT_FAMILY,
            BUTTON_FONT,
            theme.foreground.to_jian(),
            Point2D::ZERO,
        )
        .with_font_weight(500),
        Point2D::new(
            x + ICON + ICON_GAP,
            jian_widgets::centered_text_baseline_y(rect, BUTTON_FONT),
        ),
    );
}

fn paint_cell(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    cell: Rect,
    image: &DocumentImage,
    selected: bool,
    hovered: bool,
) {
    let thumb = Rect {
        origin: cell.origin,
        size: Point2D::new(cell.size.x, cell.size.y - LABEL_H),
    };
    cx.backend.fill_round_rect(thumb, CELL_RADIUS, theme.muted);
    cx.backend.save();
    cx.backend.clip_round_rect(thumb, CELL_RADIUS);
    let painted = !image.src.is_empty()
        && paint_image_source(
            cx,
            thumb,
            &image.src,
            ImageDrawMode::Crop,
            ImageAdjustments::default(),
        );
    cx.backend.restore();
    if !painted {
        // A placeholder or an unreadable source: say "image", not blank.
        draw_icon(
            cx.backend,
            Icon::Image,
            Point2D::new(
                thumb.origin.x + (thumb.size.x - 20.0) / 2.0,
                thumb.origin.y + (thumb.size.y - 20.0) / 2.0,
            ),
            20.0,
            theme.muted_subtle,
            1.6,
        );
    }
    if selected {
        cx.backend
            .stroke_round_rect(thumb, CELL_RADIUS, theme.primary, 2.0);
    } else {
        let edge = if hovered {
            theme.muted_subtle
        } else {
            theme.border
        };
        cx.backend.stroke_round_rect(thumb, CELL_RADIUS, edge, 1.0);
    }
    let name = if image.name.is_empty() {
        image.node_id.as_str()
    } else {
        image.name.as_str()
    };
    let name = text_metrics::fit_chrome(cx.backend, name, cell.size.x, LABEL_FONT);
    let label_row = Rect {
        origin: Point2D::new(cell.origin.x, thumb.origin.y + thumb.size.y),
        size: Point2D::new(cell.size.x, LABEL_H),
    };
    let color = if selected {
        theme.foreground
    } else {
        theme.muted_foreground
    };
    cx.backend.draw_text(
        &TextLayout::single_run(
            &name,
            text_metrics::CHROME_FONT_FAMILY,
            LABEL_FONT,
            color.to_jian(),
            Point2D::ZERO,
        ),
        Point2D::new(
            cell.origin.x + 2.0,
            jian_widgets::centered_text_baseline_y(label_row, LABEL_FONT),
        ),
    );
}

#[cfg(test)]
#[path = "images_tab_tests.rs"]
mod images_tab_tests;
