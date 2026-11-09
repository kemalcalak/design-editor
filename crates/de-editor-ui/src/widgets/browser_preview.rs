//! Browser nodes in Preview (▶) (#324): what a preview frame showed of
//! them.
//!
//! On the editor canvas a browser node is canvas-only and plays live only
//! while selected. In Preview it behaves like an embedded page: the
//! preview painter shows its page (the live frame, else the still) and
//! every page on screen streams and takes the pointer and keys directly.
//!
//! A host opens each preview frame with [`begin_frame`] and closes it with
//! [`end_frame`]; the preview painter runs each pass that may show pages
//! inside [`with_clip`], naming the screen rect the pass is clipped to (the
//! canvas, the device screen, a pinned strip). `canvas_browser` then paints
//! the page box and calls [`note`], which hands the node to the snapshot
//! and stream queues (`browser_textures::note_visible`) and keeps its box
//! on screen for input ([`page_at`]). Painters outside a host's preview
//! frame (MCP `capture_preview`, tests) still show the pages but note
//! nothing, so they never move the editor's visible set.
//!
//! The record is per thread: hosts paint and route input on one UI thread.

use std::cell::RefCell;

use crate::{Point2D, Rect};

use super::browser_textures;

/// One browser node a preview frame showed.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewPage {
    pub node_id: String,
    /// The node box on screen (logical px), where the page is laid out.
    pub rect: Rect,
    /// The part of `rect` the frame showed (inside its pass's clip).
    pub shown: Rect,
    /// A stream's frame was on screen: the page takes input.
    pub live: bool,
}

#[derive(Default)]
struct Record {
    /// A host's preview frame is being painted.
    framed: bool,
    /// The clip of the preview pass painting now.
    clip: Option<Rect>,
    /// The pages of the last preview frame, in paint order.
    pages: Vec<PreviewPage>,
}

thread_local! {
    static RECORD: RefCell<Record> = RefCell::new(Record::default());
}

/// Host side: a preview frame starts. What it paints replaces the visible
/// browser nodes and the pages input can reach.
pub fn begin_frame() {
    browser_textures::begin_frame(false);
    RECORD.with(|record| {
        let mut record = record.borrow_mut();
        record.framed = true;
        record.pages.clear();
    });
}

/// Host side: the preview frame is painted.
pub fn end_frame() {
    RECORD.with(|record| record.borrow_mut().framed = false);
}

/// Host side: the preview closed; no page takes input any more.
pub fn clear() {
    RECORD.with(|record| record.borrow_mut().pages.clear());
}

/// Preview painter: run `paint`, whose browser nodes show their pages,
/// clipped on screen to `clip`.
pub fn with_clip<R>(clip: Rect, paint: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rect>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RECORD.with(|record| record.borrow_mut().clip = self.0);
        }
    }
    let previous = RECORD.with(|record| record.borrow_mut().clip.replace(clip));
    let _restore = Restore(previous);
    paint()
}

/// The clip of the preview pass painting now; `None` outside the preview
/// painter.
pub(crate) fn painting() -> Option<Rect> {
    RECORD.with(|record| record.borrow().clip)
}

/// Paint side: the preview painted `node_id`'s page in `rect` (screen
/// px); `live` when a stream's frame showed. Noted only inside a host's
/// preview frame and only when some of it is inside the pass's clip.
pub(crate) fn note(node_id: &str, rect: Rect, live: bool) {
    let noted = RECORD.with(|record| {
        let mut record = record.borrow_mut();
        if !record.framed {
            return false;
        }
        let Some(shown) = record.clip.and_then(|clip| intersect(rect, clip)) else {
            return false;
        };
        record.pages.push(PreviewPage {
            node_id: node_id.to_string(),
            rect,
            shown,
            live,
        });
        true
    });
    if noted {
        browser_textures::note_visible(node_id, [rect.size.x, rect.size.y]);
    }
}

/// The topmost page the last preview frame showed under screen `point`.
pub fn page_at(point: Point2D) -> Option<PreviewPage> {
    RECORD.with(|record| {
        record
            .borrow()
            .pages
            .iter()
            .rev()
            .find(|page| page.shown.contains(point))
            .cloned()
    })
}

/// Where the last preview frame showed `node_id`'s page (its topmost box).
pub fn page(node_id: &str) -> Option<PreviewPage> {
    RECORD.with(|record| {
        record
            .borrow()
            .pages
            .iter()
            .rev()
            .find(|page| page.node_id == node_id)
            .cloned()
    })
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.origin.x.max(b.origin.x);
    let y0 = a.origin.y.max(b.origin.y);
    let x1 = (a.origin.x + a.size.x).min(b.origin.x + b.size.x);
    let y1 = (a.origin.y + a.size.y).min(b.origin.y + b.size.y);
    (x1 > x0 && y1 > y0).then(|| Rect {
        origin: Point2D::new(x0, y0),
        size: Point2D::new(x1 - x0, y1 - y0),
    })
}

#[cfg(test)]
#[path = "browser_preview_tests.rs"]
mod tests;
