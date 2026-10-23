//! The tab row's design layout (#30): the tab on show is a labelled
//! pill, every other tab is its glyph alone, across the rail's drag
//! range and with or without the optional tabs.
//!
//! Sibling of `slides_panel_tests.rs`, split out at the 800-line cap.

use super::*;

const ALL_TABS: [(LeftPanelTab, SlidesPanelTarget); 5] = [
    (LeftPanelTab::Chat, SlidesPanelTarget::ChatTab),
    (LeftPanelTab::Layers, SlidesPanelTarget::LayersTab),
    (LeftPanelTab::Components, SlidesPanelTarget::ComponentsTab),
    (LeftPanelTab::Images, SlidesPanelTarget::ImagesTab),
    (LeftPanelTab::Slides, SlidesPanelTarget::SlidesTab),
];

/// Every tab hit-tests where it paints, in row order, at both ends of
/// the drag range, and the row never overhangs the rail.
#[test]
fn five_tabs_hit_test_across_the_drag_range() {
    for width in [240.0, 440.0] {
        let tabs = tabs_at(width, LeftPanelTab::Chat, &EN_ROW);
        let mid_y = tabs.row.origin.y + tabs.row.size.y / 2.0;
        let mut previous_right = 0.0_f32;
        for (tab, target) in ALL_TABS {
            let rect = tabs.rect_of(tab);
            assert!(rect.size.x > 0.0, "{tab:?} exists at {width}: {rect:?}");
            assert!(rect.origin.x >= previous_right, "row order at {width}");
            let inside = Point2D::new(rect.origin.x + rect.size.x / 2.0, mid_y);
            assert_eq!(tabs.hit(inside), Some(target), "at {width}");
            previous_right = rect.origin.x + rect.size.x;
        }
        assert!(previous_right <= width, "the tabs fit the rail at {width}");
    }
}

/// The labelled pill follows whichever tab is active; the others are the
/// same glyph-sized squares wherever they sit.
#[test]
fn the_labelled_pill_follows_whichever_tab_is_active() {
    for (active, _) in ALL_TABS {
        let tabs = tabs_at(320.0, active, &EN_ROW);
        let active_w = tabs.rect_of(active).size.x;
        for (tab, _) in ALL_TABS.into_iter().filter(|(t, _)| *t != active) {
            assert!(
                active_w > tabs.rect_of(tab).size.x,
                "{active:?}'s pill is wider than {tab:?}'s glyph"
            );
        }
    }
    let on_chat = tabs_at(320.0, LeftPanelTab::Chat, &EN_ROW);
    let on_images = tabs_at(320.0, LeftPanelTab::Images, &EN_ROW);
    assert!(
        (on_chat.rect_of(LeftPanelTab::Layers).size.x
            - on_images.rect_of(LeftPanelTab::Slides).size.x)
            .abs()
            < 0.01
    );
}

/// A document with no boards drops the slides tab, not the row; a host
/// that owns the chat drops the Agent tab.
#[test]
fn optional_tabs_drop_out_of_the_row() {
    let row = SlidesTabRow {
        slides_available: false,
        chat_available: false,
        ..EN_ROW
    };
    let tabs = SlidesPanelTabs::new(PANEL, LeftPanelTab::Layers, &row);
    assert_eq!(tabs.rect_of(LeftPanelTab::Slides), Rect::ZERO);
    assert_eq!(tabs.rect_of(LeftPanelTab::Chat), Rect::ZERO);
    let first = tabs.rect_of(LeftPanelTab::Layers);
    assert!(first.size.x > 0.0, "Layers leads when Agent is absent");
    for tab in [LeftPanelTab::Components, LeftPanelTab::Images] {
        assert!(tabs.rect_of(tab).origin.x > first.origin.x);
    }
}

/// Even a very narrow rail never paints a tab past its edge: each tab is
/// capped at what its predecessors left.
#[test]
fn a_narrow_rail_caps_the_tabs_inside_the_row() {
    let tabs = tabs_at(120.0, LeftPanelTab::Images, &VI_ROW);
    for (tab, _) in ALL_TABS {
        let rect = tabs.rect_of(tab);
        assert!(
            rect.origin.x + rect.size.x <= 120.0 + 0.01,
            "{tab:?}: {rect:?}"
        );
    }
}
