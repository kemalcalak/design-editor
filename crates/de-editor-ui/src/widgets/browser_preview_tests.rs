use super::*;
use crate::widgets::browser_textures::{self, lock_for_tests};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect {
        origin: Point2D::new(x, y),
        size: Point2D::new(w, h),
    }
}

#[test]
fn a_preview_frame_notes_its_pages_inside_the_clip() {
    let _lock = lock_for_tests();
    begin_frame();
    with_clip(rect(0.0, 0.0, 100.0, 100.0), || {
        assert!(painting().is_some());
        note("bp-in", rect(50.0, 50.0, 100.0, 100.0), true);
        note("bp-out", rect(200.0, 0.0, 10.0, 10.0), false);
    });
    assert!(painting().is_none(), "the clip ends with its pass");
    end_frame();
    let visible = browser_textures::take_visible().unwrap_or_default();
    assert!(visible.iter().any(|id| id == "bp-in"));
    assert!(!visible.iter().any(|id| id == "bp-out"), "clipped away");
    let hit = page_at(Point2D::new(60.0, 60.0)).expect("on the page");
    assert_eq!(hit.node_id, "bp-in");
    assert_eq!(hit.shown, rect(50.0, 50.0, 50.0, 50.0));
    assert!(hit.live);
    assert!(
        page_at(Point2D::new(120.0, 120.0)).is_none(),
        "outside the clip the page is hidden"
    );
    assert_eq!(
        page("bp-in").map(|p| p.rect),
        Some(rect(50.0, 50.0, 100.0, 100.0))
    );
    clear();
    assert!(page_at(Point2D::new(60.0, 60.0)).is_none());
}

#[test]
fn painting_outside_a_host_frame_notes_nothing() {
    let _lock = lock_for_tests();
    with_clip(rect(0.0, 0.0, 100.0, 100.0), || {
        note("bp-headless", rect(0.0, 0.0, 10.0, 10.0), false);
    });
    assert!(page("bp-headless").is_none());
}

#[test]
fn the_topmost_page_wins_and_a_new_frame_replaces_the_record() {
    let _lock = lock_for_tests();
    begin_frame();
    with_clip(rect(0.0, 0.0, 100.0, 100.0), || {
        note("bp-under", rect(0.0, 0.0, 50.0, 50.0), true);
        note("bp-over", rect(10.0, 10.0, 20.0, 20.0), false);
    });
    end_frame();
    assert_eq!(
        page_at(Point2D::new(15.0, 15.0)).map(|p| p.node_id),
        Some("bp-over".to_string())
    );
    begin_frame();
    end_frame();
    assert!(page_at(Point2D::new(15.0, 15.0)).is_none());
    browser_textures::take_visible();
}
