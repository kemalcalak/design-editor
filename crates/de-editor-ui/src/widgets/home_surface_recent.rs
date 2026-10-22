//! The Recent section: its header with the grid / list toggle, the card
//! grid, the list, and the "No designs yet" panel.

use super::copy::{edited_label, tr};
use super::icons::{draw_glyph, HomeGlyph};
use super::layout::{CARD_THUMB_H, LIST_HEADER_H};
use super::paint::{baseline, caption, draw_text, draw_text_weighted};
use super::{HomeLayout, HomeRecent, HomeSurface, MONO, SANS};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::text_metrics::fit_in_family;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::{HomeHit, RecentView};

/// List column widths after the flexible name column.
const FOLDER_W: f32 = 360.0;
const EDITED_W: f32 = 140.0;

pub(super) fn paint_recent(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let header = layout.recent_header;
    caption(
        cx,
        surface,
        tr(surface.locale, "home.recent.title"),
        header.origin.x,
        baseline(header, 11.0),
    );
    if surface.recent.is_empty() {
        paint_empty(surface, cx, layout.recent_frame);
        return;
    }
    paint_view_toggle(surface, cx, layout);
    match surface.state.recent_view {
        RecentView::Grid => {
            for (index, (rect, entry)) in
                layout.recent_items.iter().zip(&surface.recent).enumerate()
            {
                paint_card(surface, cx, *rect, entry, index);
            }
        }
        RecentView::List => paint_list(surface, cx, layout),
    }
}

fn paint_view_toggle(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    let frame = Rect::xywh(
        layout.view_grid.origin.x - 2.0,
        layout.view_grid.origin.y - 2.0,
        52.0,
        27.0,
    );
    cx.backend.stroke_round_rect(frame, 7.0, p.border, 1.0);
    for (rect, view, glyph) in [
        (
            layout.view_grid,
            RecentView::Grid,
            HomeGlyph::Shared(Icon::LayoutGrid),
        ),
        (layout.view_list, RecentView::List, HomeGlyph::List),
    ] {
        let selected = surface.state.recent_view == view;
        let hovered = surface.state.hover == Some(HomeHit::RecentView(view));
        if selected || hovered {
            cx.backend.fill_round_rect(rect, 5.0, p.subtle);
        }
        draw_glyph(
            cx.backend,
            glyph,
            Point2D::new(rect.origin.x + 5.0, rect.origin.y + 5.0),
            13.0,
            if selected { p.text } else { p.text_3 },
            1.7,
        );
    }
}

fn age(surface: &HomeSurface<'_>, entry: &HomeRecent) -> String {
    if surface.wall_secs == 0 {
        return String::new();
    }
    crate::widgets::file_menu::format_age(
        surface.ui,
        surface.wall_secs.saturating_sub(entry.modified_at),
    )
}

fn paint_card(
    surface: &HomeSurface<'_>,
    cx: &mut PaintCx<'_>,
    rect: Rect,
    entry: &HomeRecent,
    index: usize,
) {
    let p = surface.palette;
    let hovered = surface.state.hover == Some(HomeHit::Recent(index));
    cx.backend.save();
    cx.backend.clip_round_rect(rect, 10.0);
    cx.backend.fill_rect(rect, p.surface);
    let thumb = Rect::xywh(rect.origin.x, rect.origin.y, rect.size.x, CARD_THUMB_H);
    cx.backend.fill_rect(thumb, p.subtle);
    if !paint_thumbnail(cx, entry, thumb) {
        draw_glyph(
            cx.backend,
            HomeGlyph::File,
            Point2D::new(
                thumb.origin.x + (thumb.size.x - 22.0) / 2.0,
                thumb.origin.y + (thumb.size.y - 22.0) / 2.0,
            ),
            22.0,
            p.text_3,
            1.5,
        );
    }
    cx.backend.fill_rect(
        Rect::xywh(
            rect.origin.x,
            thumb.origin.y + thumb.size.y,
            rect.size.x,
            1.0,
        ),
        p.border,
    );
    cx.backend.restore();
    cx.backend
        .stroke_round_rect(rect, 10.0, if hovered { p.text_3 } else { p.border }, 1.0);
    let x = rect.origin.x + 12.0;
    let text_w = rect.size.x - 24.0;
    let name = fit_in_family(cx.backend, &entry.name, text_w, 13.0, SANS);
    draw_text_weighted(
        cx,
        &name,
        SANS,
        13.0,
        p.text,
        500,
        Point2D::new(x, thumb.origin.y + thumb.size.y + 24.0),
    );
    let age = age(surface, entry);
    if !age.is_empty() {
        let label = edited_label(surface.locale, &age);
        let label = fit_in_family(cx.backend, &label, text_w, 12.0, SANS);
        draw_text(
            cx,
            &label,
            SANS,
            12.0,
            p.text_3,
            Point2D::new(x, thumb.origin.y + thumb.size.y + 44.0),
        );
    }
}

/// Paint the document's saved thumbnail into `thumb`, cropped to fill
/// it. Returns `false` while there is none (never saved by
/// this build, or still decoding) so the caller paints the file glyph.
fn paint_thumbnail(cx: &mut PaintCx<'_>, entry: &HomeRecent, thumb: Rect) -> bool {
    use crate::widgets::canvas_viewport_image::{
        cached_bytes_for, note_pending_decode, required_raster_edge,
    };
    let id =
        de_editor_core::home_thumbnail::home_thumbnail_image_id(&entry.path, entry.modified_at);
    let Some(bytes) = cached_bytes_for(id) else {
        return false;
    };
    let max_edge = required_raster_edge(thumb, cx.backend.dpi_scale());
    if !cx.backend.image_decoded(id, &bytes, max_edge) {
        note_pending_decode(id, max_edge);
    }
    if !cx.backend.image_resident(id) {
        return false;
    }
    cx.backend.draw_image_with_options(
        thumb,
        id,
        &bytes,
        crate::ImageDrawMode::Fill,
        crate::ImageAdjustments::default(),
        1.0,
        0.0,
    );
    true
}

fn paint_list(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &HomeLayout) {
    let p = surface.palette;
    let frame = layout.recent_frame;
    cx.backend.fill_round_rect(frame, 10.0, p.surface);
    cx.backend.stroke_round_rect(frame, 10.0, p.border, 1.0);
    let (folder_x, edited_x) = list_columns(frame);
    let head = Rect::xywh(frame.origin.x, frame.origin.y, frame.size.x, LIST_HEADER_H);
    let head_y = baseline(head, 11.0);
    caption(
        cx,
        surface,
        tr(surface.locale, "home.recent.name"),
        frame.origin.x + 16.0,
        head_y,
    );
    if folder_x > 0.0 {
        caption(
            cx,
            surface,
            tr(surface.locale, "home.recent.folder"),
            folder_x,
            head_y,
        );
    }
    caption(
        cx,
        surface,
        tr(surface.locale, "home.recent.editedColumn"),
        edited_x,
        head_y,
    );
    for (index, (rect, entry)) in layout.recent_items.iter().zip(&surface.recent).enumerate() {
        cx.backend.fill_rect(
            Rect::xywh(
                rect.origin.x + 1.0,
                rect.origin.y - 1.0,
                rect.size.x - 2.0,
                1.0,
            ),
            p.border,
        );
        let hovered = surface.state.hover == Some(HomeHit::Recent(index));
        if hovered {
            let last = index + 1 == layout.recent_items.len();
            let wash = Rect::xywh(
                rect.origin.x + 1.0,
                rect.origin.y,
                rect.size.x - 2.0,
                rect.size.y,
            );
            if last {
                cx.backend.save();
                cx.backend.clip_round_rect(frame, 10.0);
                cx.backend.fill_rect(wash, p.subtle);
                cx.backend.restore();
            } else {
                cx.backend.fill_rect(wash, p.subtle);
            }
        }
        let y = baseline(*rect, 13.0);
        draw_glyph(
            cx.backend,
            HomeGlyph::File,
            Point2D::new(rect.origin.x + 16.0, rect.origin.y + 14.5),
            15.0,
            p.text_3,
            1.6,
        );
        let name_end = if folder_x > 0.0 { folder_x } else { edited_x } - 16.0;
        let name_x = rect.origin.x + 41.0;
        let name = fit_in_family(cx.backend, &entry.name, name_end - name_x, 13.0, SANS);
        draw_text(cx, &name, SANS, 13.0, p.text, Point2D::new(name_x, y));
        if folder_x > 0.0 {
            let folder = fit_in_family(cx.backend, &entry.folder, FOLDER_W - 16.0, 11.0, MONO);
            draw_text(
                cx,
                &folder,
                MONO,
                11.0,
                p.text_3,
                Point2D::new(folder_x, baseline(*rect, 11.0)),
            );
        }
        draw_text(
            cx,
            &age(surface, entry),
            SANS,
            13.0,
            p.text_2,
            Point2D::new(edited_x, y),
        );
    }
}

/// The folder and edited column x positions; the folder column drops
/// out (`0`) when the list is too narrow to give the name room.
fn list_columns(frame: Rect) -> (f32, f32) {
    let edited_x = frame.origin.x + frame.size.x - 16.0 - EDITED_W;
    let folder_x = edited_x - 16.0 - FOLDER_W;
    if folder_x - frame.origin.x < 220.0 {
        (0.0, edited_x)
    } else {
        (folder_x, edited_x)
    }
}

fn paint_empty(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, frame: Rect) {
    let p = surface.palette;
    cx.backend.fill_round_rect(frame, 10.0, p.surface);
    cx.backend.stroke_round_rect(frame, 10.0, p.border, 1.0);
    let cx_mid = frame.origin.x + frame.size.x / 2.0;
    draw_icon(
        cx.backend,
        Icon::FolderOpen,
        Point2D::new(cx_mid - 11.0, frame.origin.y + 62.0),
        22.0,
        p.text_3,
        1.5,
    );
    let title = tr(surface.locale, "home.recent.emptyTitle");
    let title_w = cx.backend.measure_text_family(title, 14.0, SANS);
    draw_text_weighted(
        cx,
        title,
        SANS,
        14.0,
        p.text,
        600,
        Point2D::new(cx_mid - title_w / 2.0, frame.origin.y + 116.0),
    );
    let body = tr(surface.locale, "home.recent.emptyBody");
    let body = fit_in_family(cx.backend, body, frame.size.x - 40.0, 12.0, SANS);
    let body_w = cx.backend.measure_text_family(&body, 12.0, SANS);
    draw_text(
        cx,
        &body,
        SANS,
        12.0,
        p.text_2,
        Point2D::new(cx_mid - body_w / 2.0, frame.origin.y + 140.0),
    );
}
