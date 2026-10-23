//! Surface-level Home tests: data gathering, hit-test order, popovers
//! and the paint pass.

use super::*;
use crate::widgets::test_capture_backend::CaptureBackend;
use de_editor_core::{HomeSize, RecentFile};

const W: f32 = 1440.0;
const H: f32 = 900.0;

fn visible_state() -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.locale = Locale::EnUs;
    state.editor_ui.home.show(false);
    state
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn hidden_home_builds_no_surface() {
    let state = EditorState::default();
    assert!(HomeSurface::for_editor(&state).is_none());
}

#[test]
fn recent_entries_drop_the_extension_and_abbreviate_the_home_folder() {
    let mut state = visible_state();
    state.editor_ui.recent_files = vec![
        RecentFile {
            path: "/Users/kemal/dev/acme/design/acme-landing.de".into(),
            modified_at: 10,
        },
        RecentFile {
            path: "C:\\Users\\kemal\\Desktop\\deck.de".into(),
            modified_at: 11,
        },
        RecentFile {
            path: "/tmp/scratch.de".into(),
            modified_at: 12,
        },
    ];
    let home = HomeSurface::for_editor(&state).unwrap();
    assert_eq!(home.recent[0].name, "acme-landing");
    assert_eq!(home.recent[0].folder, "~/dev/acme/design");
    assert_eq!(home.recent[1].name, "deck");
    assert_eq!(home.recent[1].folder, "~\\Desktop");
    assert_eq!(home.recent[2].folder, "/tmp");
    assert_eq!(home.recent[0].parent, "design");
    assert_eq!(home.recent[1].parent, "Desktop");
    assert_eq!(home.recent[2].parent, "tmp");
}

#[test]
fn a_recent_card_carries_its_installed_preview_stamp() {
    let mut state = visible_state();
    state.editor_ui.recent_files = vec![
        RecentFile {
            path: "/tmp/with.de".into(),
            modified_at: 1,
        },
        RecentFile {
            path: "/tmp/without.de".into(),
            modified_at: 2,
        },
    ];
    state
        .editor_ui
        .home
        .preview_stamps
        .insert("/tmp/with.de".into(), 42);
    let home = HomeSurface::for_editor(&state).unwrap();
    assert_eq!(home.recent[0].preview_stamp, Some(42));
    assert_eq!(home.recent[1].preview_stamp, None, "no preview: glyph");
}

#[test]
fn every_chip_hits_its_kind() {
    let state = visible_state();
    let home = HomeSurface::for_editor(&state).unwrap();
    let layout = home.layout(W, H);
    for (rect, family) in layout.chips.iter().zip(HomeFamily::ALL) {
        assert_eq!(
            home.hit_test(W, H, center(*rect)),
            Some(HomeHit::Kind(family))
        );
    }
}

#[test]
fn composer_targets_win_over_the_sheet() {
    let state = visible_state();
    let home = HomeSurface::for_editor(&state).unwrap();
    let l = home.layout(W, H);
    assert_eq!(home.hit_test(W, H, center(l.attach)), Some(HomeHit::Attach));
    assert_eq!(
        home.hit_test(W, H, center(l.size_chip)),
        Some(HomeHit::SizeChip)
    );
    assert_eq!(home.hit_test(W, H, center(l.send)), Some(HomeHit::Send));
    assert_eq!(
        home.hit_test(W, H, center(l.model_chip)),
        Some(HomeHit::AddProvider),
        "a fresh state has no usable agent"
    );
    assert_eq!(
        home.hit_test(W, H, center(l.input_box)),
        Some(HomeHit::Sheet)
    );
}

#[test]
fn the_open_size_menu_owns_presses_inside_it() {
    let mut state = visible_state();
    state.editor_ui.home.size_menu_open = true;
    let home = HomeSurface::for_editor(&state).unwrap();
    let l = home.layout(W, H);
    assert_eq!(
        home.hit_test(W, H, center(l.size_rows[2])),
        Some(HomeHit::SizeOption(HomeSize::Mobile390))
    );
    assert!(home.popover_contains(W, H, center(l.size_menu)));
    assert!(!home.popover_contains(W, H, center(l.imports[2])));
}

#[test]
fn slides_offer_only_slide_sizes() {
    let mut state = visible_state();
    state.editor_ui.home.set_task(HomeFamily::Slides);
    state.editor_ui.home.size_menu_open = true;
    let home = HomeSurface::for_editor(&state).unwrap();
    let l = home.layout(W, H);
    assert_eq!(l.size_rows[2].size.x, 0.0, "two slide sizes, two rows");
    assert_eq!(
        home.hit_test(W, H, center(l.size_rows[1])),
        Some(HomeHit::SizeOption(HomeSize::Slide43))
    );
}

#[test]
fn the_mcp_popover_sits_beside_the_card_and_maps_its_controls() {
    let mut state = visible_state();
    state.editor_ui.home.mcp_popover_open = true;
    let home = HomeSurface::for_editor(&state).unwrap();
    let l = home.layout(W, H);
    let popover = home.mcp_popover(&l, W);
    assert_eq!(popover.card.origin.x, l.sidebar.size.x + 8.0);
    assert_eq!(
        bottom(popover.card),
        bottom(l.mcp_card),
        "bottom-aligned with the card"
    );
    assert_eq!(
        home.hit_test(W, H, center(popover.copy_endpoint)),
        Some(HomeHit::McpCopyEndpoint)
    );
    assert_eq!(
        home.hit_test(W, H, center(popover.action)),
        Some(HomeHit::McpSetUp)
    );
    assert_eq!(
        home.hit_test(W, H, center(popover.tabs[1].1)),
        Some(HomeHit::McpAgent(popover.tabs[1].0))
    );
    assert_eq!(
        home.hit_test(W, H, center(popover.header)),
        Some(HomeHit::McpInside)
    );
    // The popover covers the column behind it.
    assert!(home.popover_contains(W, H, center(popover.card)));
}

#[test]
fn a_configured_agent_offers_remove() {
    let mut state = visible_state();
    state.editor_ui.home.mcp_popover_open = true;
    state.editor_ui.agent_settings.mcp_cli_enabled[0] = true;
    let home = HomeSurface::for_editor(&state).unwrap();
    let popover = home.mcp_popover(&home.layout(W, H), W);
    assert_eq!(popover.action_hit, HomeHit::McpRemove);
}

#[test]
fn the_column_under_the_top_bar_is_not_live() {
    let mut state = visible_state();
    state.editor_ui.recent_files = (0..8)
        .map(|i| RecentFile {
            path: format!("/tmp/file-{i}.de"),
            modified_at: i,
        })
        .collect();
    state.editor_ui.home.scroll_y = 120.0;
    let home = HomeSurface::for_editor(&state).unwrap();
    let l = home.layout(W, 600.0);
    let chip = l.chips[0];
    assert!(chip.origin.y < HOME_TOPBAR_H, "scrolled under the bar");
    assert_eq!(
        home.hit_test(
            W,
            600.0,
            Point2D::new(chip.origin.x + 4.0, chip.origin.y + 2.0)
        ),
        None
    );
}

#[test]
fn paint_draws_every_state_without_panicking() {
    let mut state = visible_state();
    state.editor_ui.recent_files = vec![RecentFile {
        path: "/tmp/a.de".into(),
        modified_at: 1,
    }];
    for (popover, menu, view) in [
        (false, false, RecentView::Grid),
        (true, true, RecentView::List),
    ] {
        state.editor_ui.home.mcp_popover_open = popover;
        state.editor_ui.home.mcp_agent_menu_open = menu;
        state.editor_ui.home.size_menu_open = !popover;
        state.editor_ui.home.recent_view = view;
        for w in [W, 860.0] {
            let home = HomeSurface::for_editor(&state)
                .unwrap()
                .with_wall_clock(100);
            let mut backend = CaptureBackend::default();
            let mut cx = PaintCx {
                backend: &mut backend,
            };
            home.paint(&mut cx, Rect::xywh(0.0, 0.0, w, H));
            assert!(!backend.round_fills.is_empty());
            assert!(backend.texts.iter().any(|(text, _)| text == "RECENT"));
            assert!(backend.texts.iter().any(|(text, _)| text == "Home"));
        }
    }
}

fn bottom(rect: Rect) -> f32 {
    rect.origin.y + rect.size.y
}

#[test]
fn a_long_agent_from_the_menu_never_overlaps_the_tabs() {
    use de_editor_core::agent_settings::McpCli;
    let mut state = visible_state();
    state.editor_ui.home.mcp_popover_open = true;
    state.editor_ui.home.mcp_agent = McpCli::Antigravity;
    let home = HomeSurface::for_editor(&state).unwrap();
    let popover = home.mcp_popover(&home.layout(W, H), W);
    for (_, tab) in &popover.tabs {
        assert!(tab.origin.x + tab.size.x <= popover.more_button.origin.x);
    }
    // Every agent stays reachable: a tab or a menu row, once.
    let shown = popover.tabs.len() + popover.more_rows.len();
    assert_eq!(shown, McpCli::ALL.len());
}
