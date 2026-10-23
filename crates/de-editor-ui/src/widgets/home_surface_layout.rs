//! Geometry for the Home surface: one rect set that paint and hit-test
//! both read, already scroll-translated. Split from the `home_surface`
//! spine so each stays under the repo's 800-line ceiling.

use super::copy::{estimate_width, kind_label, size_chip_label, tr};
use crate::Rect;
use de_editor_core::{HomeFamily, HomeSize, Locale, RecentView};

/// Windows narrower than this collapse the sidebar to an icon rail.
pub const RAIL_BREAK_W: f32 = 900.0;
pub const SIDEBAR_W: f32 = 240.0;
pub const RAIL_W: f32 = 64.0;
/// Home's own top bar; the column scrolls under it.
pub const HOME_TOPBAR_H: f32 = 60.0;
/// The borderless window's title strip (the editor TopBar's height): the
/// window-control dots and the drag band live here.
pub const WINDOW_STRIP_H: f32 = 40.0;
/// Most files the Recent section lists (two grid rows of four).
pub const RECENT_MAX: usize = de_editor_core::HOME_RECENT_MAX;

const COLUMN_MAX_W: f32 = 1000.0;
const GUTTER: f32 = 32.0;
const GUTTER_NARROW: f32 = 20.0;
const PAD_TOP: f32 = 36.0;
const PAD_TOP_NARROW: f32 = 28.0;
const PAD_BOTTOM: f32 = 40.0;
const NAV_H: f32 = 34.0;
const NAV_GAP: f32 = 2.0;
const CHIP_H: f32 = 34.0;
const CHIP_GAP: f32 = 8.0;
pub(crate) const CHIP_FONT: f32 = 13.0;
const INPUT_H: f32 = 96.0;
const TOOLBAR_H: f32 = 50.0;
pub(crate) const SIZE_CHIP_FONT: f32 = 12.0;
pub(crate) const MODEL_FONT: f32 = 12.0;
const SIZE_MENU_W: f32 = 220.0;
pub(crate) const SIZE_MENU_HEADER_H: f32 = 26.0;
pub(crate) const SIZE_ROW_H: f32 = 32.0;
const IMPORT_H: f32 = 64.0;
const IMPORT_GAP: f32 = 12.0;
/// Columns narrower than this stack the three import cards.
const IMPORT_STACK_W: f32 = 720.0;
const RECENT_HEADER_H: f32 = 27.0;
const CARD_GAP: f32 = 14.0;
pub(crate) const CARD_THUMB_H: f32 = 150.0;
const CARD_H: f32 = 207.0;
pub(crate) const LIST_HEADER_H: f32 = 34.0;
pub(crate) const LIST_ROW_H: f32 = 56.0;
const EMPTY_H: f32 = 206.0;
const MCP_CARD_H: f32 = 82.0;

/// What layout needs to know about the live state.
#[derive(Debug, Clone, Copy)]
pub struct HomeLayoutInput<'a> {
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub scroll_y: f32,
    pub locale: Locale,
    pub task: HomeFamily,
    pub size: HomeSize,
    /// The model button's label; ignored when `usable_agent` is false
    /// (the "Add a model provider →" link takes the slot).
    pub model_label: &'a str,
    pub usable_agent: bool,
    pub recent_count: usize,
    pub recent_view: RecentView,
}

/// Every rect the surface paints, in viewport coordinates. Zero-sized
/// rects mark absent targets.
#[derive(Debug, Clone, PartialEq)]
pub struct HomeLayout {
    /// The sidebar is a 64 px icon rail (narrow window).
    pub rail: bool,
    pub sidebar: Rect,
    pub top_bar: Rect,
    pub brand_mark: Rect,
    /// Recents, Open file…, Design systems, Scene templates, Settings.
    pub nav: [Rect; 5],
    /// The TEMPLATES caption (zero in the rail, which draws a divider).
    pub templates_label: Rect,
    pub rail_divider: Rect,
    pub mcp_card: Rect,
    /// The language button left of Open file (globe + code + chevron).
    pub locale_button: Rect,
    pub open_file: Rect,
    pub new_file: Rect,
    /// The scrolled content's visible region (below the top bar).
    pub viewport_body: Rect,
    pub column: Rect,
    pub chips: [Rect; 6],
    pub composer: Rect,
    pub input_box: Rect,
    pub toolbar: Rect,
    pub attach: Rect,
    pub size_chip: Rect,
    /// The model button, or the "Add a model provider →" link.
    pub model_chip: Rect,
    pub send: Rect,
    pub size_menu: Rect,
    pub size_rows: [Rect; 3],
    pub imports: [Rect; 3],
    pub recent_header: Rect,
    pub view_grid: Rect,
    pub view_list: Rect,
    /// The list's frame, or the empty-state panel.
    pub recent_frame: Rect,
    /// One rect per listed file: grid cards or list rows.
    pub recent_items: Vec<Rect>,
    /// The page's full height at scroll 0, for the scroll clamp.
    pub content_h: f32,
}

/// The width of a kind chip (icon + label + padding).
pub(crate) fn chip_width(locale: Locale, family: HomeFamily) -> f32 {
    12.0 + 16.0 + 8.0 + estimate_width(kind_label(locale, family), CHIP_FONT) + 12.0
}

pub(crate) fn size_chip_width(locale: Locale, size: HomeSize) -> f32 {
    8.0 + 13.0 + 6.0 + estimate_width(&size_chip_label(locale, size), SIZE_CHIP_FONT) + 8.0
}

/// The model slot's width: label + chevron, or the provider link + arrow.
pub(crate) fn model_slot_width(locale: Locale, label: &str, usable: bool) -> f32 {
    if usable {
        (estimate_width(label, MODEL_FONT) + 8.0 + 12.0 + 8.0).min(260.0)
    } else {
        estimate_width(tr(locale, "home.model.addProvider"), MODEL_FONT) + 6.0 + 12.0 + 8.0
    }
}

/// The language button's label: the locale's short code ("TR", "EN").
pub(crate) fn locale_code(locale: Locale) -> String {
    locale.code().split('-').next().unwrap_or("").to_uppercase()
}

fn locale_button_width(locale: Locale) -> f32 {
    10.0 + 16.0 + 6.0 + estimate_width(&locale_code(locale), 12.0) + 4.0 + 12.0 + 8.0
}

fn button_width(locale: Locale, key: &'static str) -> f32 {
    12.0 + 16.0 + 8.0 + estimate_width(tr(locale, key), 13.0) + 14.0
}

/// Resolve the full rect set (see `HomeLayout`).
pub fn home_layout(input: HomeLayoutInput<'_>) -> HomeLayout {
    let w = input.viewport_w.max(1.0);
    let h = input.viewport_h.max(1.0);
    let rail = w < RAIL_BREAK_W;
    let side_w = if rail { RAIL_W } else { SIDEBAR_W };
    let sidebar = Rect::xywh(0.0, 0.0, side_w, h);

    // Sidebar. The window-control cluster owns the title strip's left
    // edge on every platform, so the brand sits on its own row below it.
    let brand_mark = Rect::xywh(22.0, WINDOW_STRIP_H + 8.0, 20.0, 20.0);
    let first_nav_y = WINDOW_STRIP_H + 44.0;
    let nav_x = 12.0;
    let nav_w = side_w - 24.0;
    let nav_at = |y: f32| Rect::xywh(nav_x, y, nav_w, NAV_H);
    let recents = nav_at(first_nav_y);
    let open = nav_at(first_nav_y + NAV_H + NAV_GAP);
    let (templates_label, rail_divider, design_y) = if rail {
        let divider_y = open.origin.y + NAV_H + 6.0;
        (
            Rect::ZERO,
            Rect::xywh((side_w - 24.0) / 2.0, divider_y, 24.0, 1.0),
            divider_y + 7.0,
        )
    } else {
        let label_y = open.origin.y + NAV_H + NAV_GAP;
        (
            Rect::xywh(nav_x + 10.0, label_y, nav_w - 10.0, 43.0),
            Rect::ZERO,
            label_y + 43.0 + NAV_GAP,
        )
    };
    let design = nav_at(design_y);
    let scene = nav_at(design_y + NAV_H + NAV_GAP);
    let settings = nav_at(h - 18.0 - NAV_H);
    let mcp_card = if rail {
        Rect::xywh(nav_x, settings.origin.y - 6.0 - 40.0, nav_w, 40.0)
    } else {
        Rect::xywh(
            nav_x,
            settings.origin.y - NAV_GAP - MCP_CARD_H,
            nav_w,
            MCP_CARD_H,
        )
    };

    // Top bar.
    let main_x = side_w;
    let main_w = (w - side_w).max(1.0);
    let top_bar = Rect::xywh(main_x, 0.0, main_w, HOME_TOPBAR_H);
    let new_w = button_width(input.locale, "home.top.newFile");
    let open_w = button_width(input.locale, "home.top.openFile");
    let new_file = Rect::xywh(w - 28.0 - new_w, 13.0, new_w, 34.0);
    let open_file = Rect::xywh(new_file.origin.x - 8.0 - open_w, 13.0, open_w, 34.0);
    let locale_w = locale_button_width(input.locale);
    let locale_button = Rect::xywh(open_file.origin.x - 8.0 - locale_w, 13.0, locale_w, 34.0);

    // Column.
    let gutter = if rail { GUTTER_NARROW } else { GUTTER };
    let col_w = COLUMN_MAX_W.min(main_w - 2.0 * gutter).max(1.0);
    let col_x = main_x + (main_w - col_w) / 2.0;
    let scroll = input.scroll_y.max(0.0);
    let top = HOME_TOPBAR_H + if rail { PAD_TOP_NARROW } else { PAD_TOP };
    let mut y = top - scroll;
    let body_start = y;

    // Chips: greedy rows, each row centred.
    let mut chips = [Rect::ZERO; 6];
    let widths: Vec<f32> = HomeFamily::ALL
        .iter()
        .map(|family| chip_width(input.locale, *family))
        .collect();
    let mut index = 0;
    while index < widths.len() {
        let mut row_w = widths[index];
        let mut end = index + 1;
        while end < widths.len() && row_w + CHIP_GAP + widths[end] <= col_w {
            row_w += CHIP_GAP + widths[end];
            end += 1;
        }
        let mut x = col_x + (col_w - row_w) / 2.0;
        for (slot, width) in chips[index..end].iter_mut().zip(&widths[index..end]) {
            *slot = Rect::xywh(x, y, *width, CHIP_H);
            x += width + CHIP_GAP;
        }
        y += CHIP_H + CHIP_GAP;
        index = end;
    }
    y += 20.0 - CHIP_GAP;

    // Composer.
    let composer = Rect::xywh(col_x, y, col_w, INPUT_H + TOOLBAR_H);
    let input_box = Rect::xywh(col_x, y, col_w, INPUT_H);
    let toolbar = Rect::xywh(col_x, y + INPUT_H, col_w, TOOLBAR_H);
    let ty = toolbar.origin.y;
    let attach = Rect::xywh(col_x + 14.0, ty + 12.0, 26.0, 26.0);
    let size_chip = Rect::xywh(
        col_x + 48.0,
        ty + 13.0,
        size_chip_width(input.locale, input.size),
        24.0,
    );
    let send = Rect::xywh(col_x + col_w - 14.0 - 30.0, ty + 10.0, 30.0, 30.0);
    let model_w = model_slot_width(input.locale, input.model_label, input.usable_agent);
    let model_chip = Rect::xywh(send.origin.x - 10.0 - model_w, ty + 12.0, model_w, 26.0);

    let option_count = input.task.sizes().len();
    let size_menu = Rect::xywh(
        size_chip.origin.x,
        size_chip.origin.y + size_chip.size.y + 6.0,
        SIZE_MENU_W,
        12.0 + SIZE_MENU_HEADER_H + option_count as f32 * (SIZE_ROW_H + 2.0),
    );
    let mut size_rows = [Rect::ZERO; 3];
    for (row, slot) in size_rows.iter_mut().enumerate().take(option_count) {
        *slot = Rect::xywh(
            size_menu.origin.x + 6.0,
            size_menu.origin.y + 6.0 + SIZE_MENU_HEADER_H + row as f32 * (SIZE_ROW_H + 2.0),
            SIZE_MENU_W - 12.0,
            SIZE_ROW_H,
        );
    }
    y = composer.origin.y + composer.size.y + 44.0;

    // Import cards.
    let mut imports = [Rect::ZERO; 3];
    if col_w >= IMPORT_STACK_W {
        let card_w = (col_w - 2.0 * IMPORT_GAP) / 3.0;
        for (i, slot) in imports.iter_mut().enumerate() {
            *slot = Rect::xywh(
                col_x + i as f32 * (card_w + IMPORT_GAP),
                y,
                card_w,
                IMPORT_H,
            );
        }
        y += IMPORT_H;
    } else {
        for slot in &mut imports {
            *slot = Rect::xywh(col_x, y, col_w, IMPORT_H);
            y += IMPORT_H + 10.0;
        }
        y -= 10.0;
    }
    y += 52.0;

    // Recent.
    let recent_header = Rect::xywh(col_x, y, col_w, RECENT_HEADER_H);
    let has_files = input.recent_count > 0;
    let toggle_x = col_x + col_w - 52.0;
    let (view_grid, view_list) = if has_files {
        (
            Rect::xywh(toggle_x + 2.0, y + 2.0, 23.0, 23.0),
            Rect::xywh(toggle_x + 27.0, y + 2.0, 23.0, 23.0),
        )
    } else {
        (Rect::ZERO, Rect::ZERO)
    };
    y += RECENT_HEADER_H + 20.0;
    let count = input.recent_count.min(RECENT_MAX);
    let mut recent_items = Vec::with_capacity(count);
    let recent_frame;
    if !has_files {
        recent_frame = Rect::xywh(col_x, y, col_w, EMPTY_H);
        y += EMPTY_H;
    } else if input.recent_view == RecentView::List {
        let frame_h = LIST_HEADER_H + count as f32 * (LIST_ROW_H + 1.0);
        recent_frame = Rect::xywh(col_x, y, col_w, frame_h);
        for row in 0..count {
            recent_items.push(Rect::xywh(
                col_x,
                y + LIST_HEADER_H + 1.0 + row as f32 * (LIST_ROW_H + 1.0),
                col_w,
                LIST_ROW_H,
            ));
        }
        y += frame_h;
    } else {
        recent_frame = Rect::ZERO;
        let cols = if col_w >= 900.0 {
            4
        } else if col_w >= 640.0 {
            3
        } else {
            2
        };
        let card_w = (col_w - CARD_GAP * (cols - 1) as f32) / cols as f32;
        for item in 0..count {
            let (row, col) = (item / cols, item % cols);
            recent_items.push(Rect::xywh(
                col_x + col as f32 * (card_w + CARD_GAP),
                y + row as f32 * (CARD_H + CARD_GAP),
                card_w,
                CARD_H,
            ));
        }
        let rows = count.div_ceil(cols);
        y += rows as f32 * CARD_H + rows.saturating_sub(1) as f32 * CARD_GAP;
    }
    let content_h = y - body_start + top + PAD_BOTTOM;

    HomeLayout {
        rail,
        sidebar,
        top_bar,
        brand_mark,
        nav: [recents, open, design, scene, settings],
        templates_label,
        rail_divider,
        mcp_card,
        locale_button,
        open_file,
        new_file,
        viewport_body: Rect::xywh(main_x, HOME_TOPBAR_H, main_w, (h - HOME_TOPBAR_H).max(0.0)),
        column: Rect::xywh(col_x, top - scroll, col_w, content_h - top - PAD_BOTTOM),
        chips,
        composer,
        input_box,
        toolbar,
        attach,
        size_chip,
        model_chip,
        send,
        size_menu,
        size_rows,
        imports,
        recent_header,
        view_grid,
        view_list,
        recent_frame,
        recent_items,
        content_h,
    }
}

/// The largest scroll offset for this input: the page's height past the
/// visible body, never negative.
pub fn home_max_scroll(input: HomeLayoutInput<'_>) -> f32 {
    let layout = home_layout(HomeLayoutInput {
        scroll_y: 0.0,
        ..input
    });
    (layout.content_h - input.viewport_h.max(1.0)).max(0.0)
}
