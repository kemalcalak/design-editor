//! Geometry contract for the Home v2 frames (1440 × 900 and the 860 px
//! narrow frame).

use super::layout::*;
use crate::Rect;
use de_editor_core::{HomeFamily, HomeSize, Locale, RecentView};

fn input(w: f32, recent: usize, view: RecentView) -> HomeLayoutInput<'static> {
    HomeLayoutInput {
        viewport_w: w,
        viewport_h: 900.0,
        scroll_y: 0.0,
        locale: Locale::EnUs,
        task: HomeFamily::LandingPage,
        size: HomeSize::Desktop1440,
        model_label: "gpt-5 · Gateway",
        usable_agent: true,
        recent_count: recent,
        recent_view: view,
    }
}

fn right(rect: Rect) -> f32 {
    rect.origin.x + rect.size.x
}

fn bottom(rect: Rect) -> f32 {
    rect.origin.y + rect.size.y
}

#[test]
fn wide_frame_matches_the_approved_columns() {
    let l = home_layout(input(1440.0, 4, RecentView::Grid));
    assert!(!l.rail);
    assert_eq!(l.sidebar.size.x, SIDEBAR_W);
    // Centre column: 1000 wide, centred in the 1200 main area.
    assert_eq!(l.column.origin.x, 340.0);
    assert_eq!(l.column.size.x, 1000.0);
    assert_eq!(l.composer.size.x, 1000.0);
    assert_eq!(l.composer.size.y, 146.0);
    // The six chips sit on one centred row above the composer.
    assert!(l.chips.iter().all(|c| c.origin.y == l.chips[0].origin.y));
    let row_left = l.chips[0].origin.x;
    let row_right = right(l.chips[5]);
    assert!(((row_left - 340.0) - (1340.0 - row_right)).abs() < 1.0);
    // Four import cards side by side, four Recent cards in a row.
    assert!(l
        .imports
        .iter()
        .all(|c| c.origin.y == l.imports[0].origin.y));
    assert_eq!(l.recent_items.len(), 4);
    assert!(l
        .recent_items
        .iter()
        .all(|c| c.origin.y == l.recent_items[0].origin.y));
    assert!((right(l.recent_items[3]) - 1340.0).abs() < 0.5);
    // Top bar buttons hug the right edge, New file last.
    assert_eq!(right(l.new_file), 1412.0);
    assert!(right(l.open_file) < l.new_file.origin.x);
}

#[test]
fn sidebar_pins_the_mcp_card_and_settings_to_the_bottom() {
    let l = home_layout(input(1440.0, 0, RecentView::Grid));
    assert_eq!(bottom(l.nav[4]), 900.0 - 18.0);
    assert_eq!(bottom(l.mcp_card), l.nav[4].origin.y - 2.0);
    assert_eq!(l.mcp_card.size.y, 82.0);
    // The brand sits on its own row under the window-control strip, and
    // the first nav row clears it.
    assert!(l.brand_mark.origin.y >= WINDOW_STRIP_H);
    assert!(l.nav[0].origin.y >= bottom(l.brand_mark) + 12.0);
    // The language button sits left of Open file.
    assert!(right(l.locale_button) < l.open_file.origin.x);
}

#[test]
fn composer_toolbar_orders_attach_size_model_send() {
    let l = home_layout(input(1440.0, 0, RecentView::Grid));
    assert!(right(l.attach) < l.size_chip.origin.x);
    assert!(right(l.size_chip) < l.model_chip.origin.x);
    assert!(right(l.model_chip) < l.send.origin.x);
    assert_eq!(right(l.send), right(l.composer) - 14.0);
    // The size menu opens under its chip with one row per size.
    assert_eq!(l.size_menu.origin.x, l.size_chip.origin.x);
    assert!(l.size_menu.origin.y > bottom(l.size_chip));
    assert!(l.size_rows.iter().all(|r| r.size.x > 0.0));
}

#[test]
fn narrow_window_collapses_to_the_rail_and_three_columns() {
    let l = home_layout(input(860.0, 8, RecentView::Grid));
    assert!(l.rail);
    assert_eq!(l.sidebar.size.x, RAIL_W);
    assert_eq!(l.column.size.x, 860.0 - RAIL_W - 40.0);
    assert_eq!(l.templates_label.size.x, 0.0);
    assert!(l.rail_divider.size.x > 0.0);
    // Chips that no longer fit one row wrap to a second, centred one.
    let tight = home_layout(input(760.0, 0, RecentView::Grid));
    assert!(tight.chips[5].origin.y > tight.chips[0].origin.y);
    // Three Recent columns: eight files make three rows.
    let first_row_y = l.recent_items[0].origin.y;
    assert_eq!(
        l.recent_items
            .iter()
            .filter(|r| r.origin.y == first_row_y)
            .count(),
        3
    );
    assert!(l.recent_items[6].origin.y > l.recent_items[3].origin.y);
}

#[test]
fn very_narrow_columns_stack_the_import_cards() {
    let l = home_layout(input(700.0, 0, RecentView::Grid));
    assert!(l.imports[1].origin.y > bottom(l.imports[0]));
    assert_eq!(l.imports[0].size.x, l.column.size.x);
}

#[test]
fn empty_state_replaces_the_grid_and_hides_the_toggle() {
    let l = home_layout(input(1440.0, 0, RecentView::Grid));
    assert!(l.recent_items.is_empty());
    assert!(l.recent_frame.size.y > 0.0);
    assert_eq!(l.view_grid.size.x, 0.0);
    assert_eq!(l.view_list.size.x, 0.0);
}

#[test]
fn list_view_stacks_one_row_per_file_inside_its_frame() {
    let l = home_layout(input(1440.0, 6, RecentView::List));
    assert_eq!(l.recent_items.len(), 6);
    for pair in l.recent_items.windows(2) {
        assert_eq!(pair[1].origin.y - pair[0].origin.y, LIST_ROW_H + 1.0);
    }
    assert!((bottom(l.recent_items[5]) - bottom(l.recent_frame)).abs() < 0.5);
}

#[test]
fn recent_lists_at_most_eight_files() {
    let l = home_layout(input(1440.0, 20, RecentView::Grid));
    assert_eq!(l.recent_items.len(), RECENT_MAX);
}

#[test]
fn scrolling_moves_the_column_but_not_the_chrome() {
    let base = input(1440.0, 8, RecentView::Grid);
    let max = home_max_scroll(HomeLayoutInput {
        viewport_h: 600.0,
        ..base
    });
    assert!(max > 0.0, "eight files overflow a 600 px window");
    let still = home_layout(HomeLayoutInput {
        viewport_h: 600.0,
        ..base
    });
    let scrolled = home_layout(HomeLayoutInput {
        viewport_h: 600.0,
        scroll_y: max,
        ..base
    });
    assert_eq!(scrolled.composer.origin.y, still.composer.origin.y - max);
    assert_eq!(scrolled.new_file, still.new_file);
    assert_eq!(scrolled.mcp_card, still.mcp_card);
    let last = scrolled.recent_items.last().copied().unwrap();
    assert!(bottom(last) <= 600.0, "the last row is reachable");
}

#[test]
fn import_cards_wrap_two_by_two_on_a_mid_width_column() {
    // Wide enough not to stack, too narrow for four titled cards in a row.
    let l = home_layout(input(1100.0, 0, RecentView::Grid));
    let [a, b, c, d] = l.imports;
    assert_eq!(a.origin.y, b.origin.y);
    assert_eq!(c.origin.y, d.origin.y);
    assert!(c.origin.y > bottom(a), "second row sits under the first");
    assert_eq!(a.origin.x, c.origin.x);
    assert_eq!(a.size.x, b.size.x);
}
