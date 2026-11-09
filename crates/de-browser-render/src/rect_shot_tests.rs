use serde_json::json;

use super::*;
use crate::tab::screenshot_params;
use crate::{BrowserPool, PageSessions, ShotTarget, Viewport};

const VIEW: View = View {
    x: 0.0,
    y: 400.0,
    width: 800.0,
    height: 600.0,
};

fn clip(x: f64, y: f64, width: f64, height: f64) -> Clip {
    Clip {
        x,
        y,
        width,
        height,
        scale: 1.0,
    }
}

#[test]
fn a_rect_is_cut_to_the_page_and_capped() {
    assert_eq!(
        rect_clip(10.0, 20.0, 30.5, 40.0).unwrap(),
        clip(10.0, 20.0, 30.5, 40.0)
    );
    assert_eq!(
        rect_clip(-10.0, -5.0, 30.0, 40.0).unwrap(),
        clip(0.0, 0.0, 20.0, 35.0),
        "the part left of and above the page is dropped"
    );
    let huge = rect_clip(0.0, 0.0, 1e6, 1e6).unwrap();
    assert_eq!(
        (huge.width, huge.height),
        (MAX_FULL_PAGE_WIDTH, MAX_FULL_PAGE_HEIGHT)
    );
    for (x, y, w, h) in [
        (0.0, 0.0, 0.0, 10.0),
        (-50.0, 0.0, 40.0, 10.0),
        (f64::NAN, 0.0, 10.0, 10.0),
        (0.0, 0.0, f64::INFINITY, 10.0),
    ] {
        assert!(rect_clip(x, y, w, h).is_err(), "{x} {y} {w} {h}");
    }
}

#[test]
fn a_rect_on_screen_is_taken_as_it_stands() {
    assert_eq!(
        plan_rect_shot(clip(100.0, 450.0, 200.0, 100.0), VIEW),
        RectPlan::InView
    );
    assert_eq!(
        plan_rect_shot(clip(0.0, 400.0, 800.0, 600.0), VIEW),
        RectPlan::InView,
        "exactly the viewport"
    );
}

#[test]
fn a_rect_off_screen_that_fits_is_scrolled_to_the_middle() {
    assert_eq!(
        plan_rect_shot(clip(100.0, 1500.0, 200.0, 100.0), VIEW),
        RectPlan::ScrollTo { x: 0.0, y: 1250.0 }
    );
    assert_eq!(
        plan_rect_shot(clip(100.0, 300.0, 200.0, 200.0), VIEW),
        RectPlan::ScrollTo { x: 0.0, y: 100.0 },
        "partly above the viewport"
    );
}

#[test]
fn a_rect_larger_than_the_viewport_is_drawn_beyond_it() {
    assert_eq!(
        plan_rect_shot(clip(0.0, 0.0, 900.0, 100.0), VIEW),
        RectPlan::BeyondViewport
    );
}

#[test]
fn an_in_view_capture_does_not_lay_the_page_out_again() {
    let rect = clip(12.0, 1300.0, 64.0, 32.0);
    assert_eq!(
        screenshot_params(Some(rect), false),
        json!({
            "format": "png", "fromSurface": true,
            "clip": { "x": 12.0, "y": 1300.0, "width": 64.0, "height": 32.0, "scale": 1.0 },
        })
    );
    assert_eq!(
        screenshot_params(Some(rect), true)["captureBeyondViewport"],
        json!(true)
    );
    assert_eq!(
        screenshot_params(None, false),
        json!({ "format": "png", "fromSurface": true })
    );
}

#[test]
fn the_scroll_is_instant_and_waits_for_frames_only_before_the_capture() {
    let before = scroll_script(0.0, 1250.5, true);
    assert!(before.contains("left: 0, top: 1250.5, behavior: \"instant\""));
    assert!(before.contains("requestAnimationFrame(() => requestAnimationFrame(answer))"));
    assert!(before.contains(&format!("setTimeout(answer, {FRAME_WAIT_MS})")));
    let back = scroll_script(0.0, 400.0, false);
    assert!(!back.contains("requestAnimationFrame"));
    assert_eq!(
        parse_view(&json!([0, 400, 800, 600])),
        Some(VIEW),
        "integers read as px"
    );
    assert_eq!(parse_view(&json!([0, 400])), None);
}

/// A canvas below the fold, captured by rect with the installed browser:
/// `cargo test -p de-browser-render -- --ignored`.
#[test]
#[ignore = "needs an installed Chromium-based browser"]
fn a_rect_below_the_fold_is_captured_and_the_scroll_put_back() {
    let dir = std::env::temp_dir().join(format!("de-rect-shot-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let page = dir.join("page.html");
    std::fs::write(
        &page,
        "<html><body style='margin:0;height:3000px'>\
         <div style='position:absolute;left:20px;top:1500px;width:120px;height:60px;\
         background:#f00'></div></body></html>",
    )
    .unwrap();
    let sessions = PageSessions::new(BrowserPool::new());
    let viewport = Viewport {
        width: 400,
        height: 300,
        scale: 1.0,
        mobile: false,
    };
    let url = format!("file://{}", page.display());
    sessions
        .with("rect", viewport, |s| s.load(&url))
        .expect("loads");
    let shot = sessions
        .with("rect", viewport, |s| {
            s.screenshot(&ShotTarget::Rect {
                x: 20.0,
                y: 1500.0,
                w: 120.0,
                h: 60.0,
            })
        })
        .expect("captures");
    assert_eq!(u32::from_be_bytes(shot[16..20].try_into().unwrap()), 120);
    assert_eq!(u32::from_be_bytes(shot[20..24].try_into().unwrap()), 60);
    let scroll = sessions
        .with("rect", viewport, |s| s.evaluate("scrollY"))
        .unwrap();
    assert_eq!(scroll, json!(0), "the page is back where it was");
    sessions.close("rect");
    let _ = std::fs::remove_dir_all(dir);
}
