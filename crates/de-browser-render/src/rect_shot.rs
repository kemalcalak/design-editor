//! [`ShotTarget::Rect`](super::ShotTarget::Rect): one rectangle of the page
//! (document CSS px) at 1×, as the browser drew it. A WebGL canvas made
//! without `preserveDrawingBuffer` reads back blank through its own
//! `toDataURL`, but the page's screenshot still shows it (#323).
//!
//! A rect that fits the viewport is taken from what the viewport shows:
//! when it is not wholly on screen the page is scrolled (instantly) to
//! centre it, two animation frames pass so a page that stops drawing
//! off-screen canvases draws again, and the scroll is put back afterwards.
//! Only a rect larger than the viewport uses `captureBeyondViewport`, which
//! lays the page out at full size for the capture; a page that redraws its
//! canvases on resize can show them cleared there.

use serde_json::Value;

use super::{PageSession, MAX_FULL_PAGE_WIDTH, SCRIPT_TIMEOUT};
use crate::tab::{Clip, MAX_FULL_PAGE_HEIGHT};
use crate::BrowserRenderError;

/// Slack when asking whether a rect is on screen, in CSS px (rects are
/// rounded to 1/100 px).
const ON_SCREEN_SLACK: f64 = 0.5;
/// Longest wait for the two animation frames after a scroll, in ms; a tab
/// that is not drawing frames must not hold the capture.
const FRAME_WAIT_MS: u32 = 250;

/// Where the viewport is in the page: scroll offset and size, CSS px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct View {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// How a rect is captured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RectPlan {
    /// Wholly on screen: capture it as it stands.
    InView,
    /// Fits the viewport: scroll the page to this offset first.
    ScrollTo { x: f64, y: f64 },
    /// Larger than the viewport.
    BeyondViewport,
}

/// The capture region for a rect at `x, y` sized `w × h`. A rect reaching
/// past the page's top or left edge, or past the full-page capture caps, is
/// refused rather than cropped: the importer would stretch a cropped
/// picture over the whole node.
pub(super) fn rect_clip(x: f64, y: f64, w: f64, h: f64) -> Result<Clip, BrowserRenderError> {
    let finite = [x, y, w, h].iter().all(|value| value.is_finite());
    let inside = x >= 0.0 && y >= 0.0 && w <= MAX_FULL_PAGE_WIDTH && h <= MAX_FULL_PAGE_HEIGHT;
    if !finite || !inside || w < 1.0 || h < 1.0 {
        return Err(BrowserRenderError::Protocol {
            method: "ShotTarget::Rect".into(),
            message: format!("the rect {x}, {y}, {w} × {h} is empty or not wholly inside the page"),
        });
    }
    Ok(Clip {
        x,
        y,
        width: w,
        height: h,
        scale: 1.0,
    })
}

/// How to capture `clip` with the viewport at `view`.
pub(super) fn plan_rect_shot(clip: Clip, view: View) -> RectPlan {
    let on_screen = clip.x >= view.x - ON_SCREEN_SLACK
        && clip.y >= view.y - ON_SCREEN_SLACK
        && clip.x + clip.width <= view.x + view.width + ON_SCREEN_SLACK
        && clip.y + clip.height <= view.y + view.height + ON_SCREEN_SLACK;
    if on_screen {
        RectPlan::InView
    } else if clip.width <= view.width && clip.height <= view.height {
        RectPlan::ScrollTo {
            x: (clip.x + clip.width / 2.0 - view.width / 2.0).max(0.0),
            y: (clip.y + clip.height / 2.0 - view.height / 2.0).max(0.0),
        }
    } else {
        RectPlan::BeyondViewport
    }
}

/// The page script that scrolls to `x, y` without smooth scrolling and
/// answers the view: with `wait_frames` once two animation frames passed
/// (or the wait ran out), otherwise at once.
pub(super) fn scroll_script(x: f64, y: f64, wait_frames: bool) -> String {
    let wait = if wait_frames {
        format!(
            "setTimeout(answer, {FRAME_WAIT_MS}); \
             requestAnimationFrame(() => requestAnimationFrame(answer));"
        )
    } else {
        "answer();".to_string()
    };
    format!(
        "new Promise((done) => {{ \
           scrollTo({{ left: {x}, top: {y}, behavior: \"instant\" }}); \
           const answer = () => done([scrollX, scrollY, innerWidth, innerHeight]); \
           {wait} }})"
    )
}

/// Answers the view as it is.
const VIEW_SCRIPT: &str = "[scrollX, scrollY, innerWidth, innerHeight]";

fn parse_view(value: &Value) -> Option<View> {
    let numbers: Vec<f64> = value.as_array()?.iter().filter_map(Value::as_f64).collect();
    match numbers[..] {
        [x, y, width, height] => Some(View {
            x,
            y,
            width,
            height,
        }),
        _ => None,
    }
}

impl PageSession {
    pub(super) fn rect_shot(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Result<Vec<u8>, BrowserRenderError> {
        let clip = rect_clip(x, y, w, h)?;
        let Some(view) = self.run_view_script(VIEW_SCRIPT)? else {
            return self.tab.screenshot(&mut self.client, Some(clip));
        };
        match plan_rect_shot(clip, view) {
            RectPlan::InView => self.tab.screenshot_in_view(&mut self.client, clip),
            RectPlan::BeyondViewport => self.tab.screenshot(&mut self.client, Some(clip)),
            RectPlan::ScrollTo { x, y } => {
                let shot = match self.run_view_script(&scroll_script(x, y, true)) {
                    Ok(Some(now)) if plan_rect_shot(clip, now) == RectPlan::InView => {
                        self.tab.screenshot_in_view(&mut self.client, clip)
                    }
                    // The page would not scroll there (an `overflow:
                    // hidden` root): draw it beyond the viewport instead.
                    Ok(_) => self.tab.screenshot(&mut self.client, Some(clip)),
                    Err(error) => Err(error),
                };
                // Best effort: the capture is what was asked for.
                let _ = self.run_view_script(&scroll_script(view.x, view.y, false));
                shot
            }
        }
    }

    /// Run a script that answers the view (and may scroll the page first).
    fn run_view_script(&mut self, script: &str) -> Result<Option<View>, BrowserRenderError> {
        let value = self
            .tab
            .evaluate(&mut self.client, script, SCRIPT_TIMEOUT)?;
        Ok(parse_view(&value))
    }
}

#[cfg(test)]
#[path = "rect_shot_tests.rs"]
mod tests;
