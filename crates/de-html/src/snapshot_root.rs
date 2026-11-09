//! The box a whole-page capture is imported into.
//!
//! The extractor roots a whole-page capture at `document.body`, whose box
//! covers only the content laid out in the body's flow. A page whose content
//! is all `position: fixed` (a full-screen `inset: 0` app shell, a
//! modal-only page) reports a body 0 px tall, and so does every empty
//! wrapper between the body and the overlay. Imported as is, the page lands
//! in a 0-tall frame. In the browser such a page fills the viewport, and
//! absolutely placed content below it still scrolls into view, so the root
//! is grown to cover both before anything is mapped.

use serde_json::{Map, Value};

use super::Rect;

/// Grow a whole-page root (`tag: "body"`) on an axis where its box is
/// empty: to the viewport and, for the height, to the bottom of its
/// non-fixed descendants. Then give the single-child chain of empty
/// wrappers above a fixed overlay the same box. A body with content keeps
/// its own box: `innerWidth` counts a classic scrollbar, the body margin is
/// not the page's, a short page hugs its content in an auto-layout import,
/// and what overflows it stays overflow, as before. A subtree capture is
/// left alone: its box is the picked element's, which the viewport says
/// nothing about.
pub(super) fn grow_page_root(root: &mut Map<String, Value>, viewport: Option<(f64, f64)>) {
    if root.get("tag").and_then(Value::as_str) != Some("body") {
        return;
    }
    let Some(rect) = Rect::from_node(root) else {
        return;
    };
    let mut right = rect.x + rect.w;
    let mut bottom = rect.y + rect.h;
    if let Some((width, height)) = viewport {
        if rect.w <= 0.0 {
            right = right.max(width);
        }
        if rect.h <= 0.0 {
            bottom = bottom.max(height);
        }
    }
    if rect.h <= 0.0 {
        if let Some(children) = root.get("children").and_then(Value::as_array) {
            for child in children {
                bottom = bottom.max(reach_bottom(child));
            }
        }
    }
    let grown = Rect {
        w: right - rect.x,
        h: bottom - rect.y,
        ..rect
    };
    set_rect(root, grown);
    grow_empty_wrappers(root, grown);
}

/// The lowest edge `node` and its descendants reach, leaving out
/// `position: fixed` subtrees (pinned to the viewport, which the root
/// already covers: an off-screen drawer parked below it must not stretch
/// the page) and whatever a clipping box hides.
fn reach_bottom(node: &Value) -> f64 {
    let Some(object) = node.as_object() else {
        return f64::NEG_INFINITY;
    };
    if is_fixed(object) {
        return f64::NEG_INFINITY;
    }
    let own = Rect::from_node(object).map_or(f64::NEG_INFINITY, |rect| rect.y + rect.h);
    if clips(object) {
        // A clipping box (`overflow: hidden`, a scroll container) keeps what
        // overflows it out of the page; only its own box counts.
        return own;
    }
    object
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(reach_bottom)
        .fold(own, f64::max)
}

/// Walk down from the root while each box holds exactly one element that
/// is itself empty in one axis and holds a fixed overlay (`#app`, then
/// `#root`, then the overlay), and give it the parent's extent. Only a lone child: a
/// 0-tall box beside siblings takes no room in the flow, and growing it
/// would push them in an auto-layout import. A clipping one (a collapsed
/// accordion) stays shut, and an empty portal mount stays empty.
fn grow_empty_wrappers(parent: &mut Map<String, Value>, parent_rect: Rect) {
    let Some([child]) = parent
        .get_mut("children")
        .and_then(Value::as_array_mut)
        .map(Vec::as_mut_slice)
    else {
        return;
    };
    let Some(object) = child.as_object_mut() else {
        return;
    };
    if object.get("kind").and_then(Value::as_str) != Some("element")
        || is_fixed(object)
        || clips(object)
        || !holds_fixed(object)
    {
        return;
    }
    let Some(rect) = Rect::from_node(object) else {
        return;
    };
    if rect.w > 0.0 && rect.h > 0.0 {
        return;
    }
    let grown = Rect {
        w: rect.w.max(parent_rect.x + parent_rect.w - rect.x),
        h: rect.h.max(parent_rect.y + parent_rect.h - rect.y),
        ..rect
    };
    set_rect(object, grown);
    grow_empty_wrappers(object, grown);
}

/// Whether a `position: fixed` box sits anywhere under `object`.
fn holds_fixed(object: &Map<String, Value>) -> bool {
    object
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .any(|child| is_fixed(child) || holds_fixed(child))
}

/// Any `overflow` other than `visible` (the snapshot records only the
/// shorthand). Unlike `is_scroll_container`, a plain `hidden` counts too:
/// both keep their overflow out of the page.
fn clips(object: &Map<String, Value>) -> bool {
    object
        .get("styles")
        .and_then(|styles| styles.get("overflow"))
        .and_then(Value::as_str)
        .is_some_and(|overflow| overflow.split_whitespace().any(|axis| axis != "visible"))
}

fn is_fixed(object: &Map<String, Value>) -> bool {
    object
        .get("styles")
        .and_then(|styles| styles.get("position"))
        .and_then(Value::as_str)
        == Some("fixed")
}

fn set_rect(object: &mut Map<String, Value>, rect: Rect) {
    object.insert(
        "rect".into(),
        serde_json::json!({ "x": rect.x, "y": rect.y, "w": rect.w, "h": rect.h }),
    );
}

/// The capture's `viewport` (`{ width, height }`), when it is usable.
pub(super) fn snapshot_viewport(snapshot: &Map<String, Value>) -> Option<(f64, f64)> {
    let viewport = snapshot.get("viewport")?.as_object()?;
    let width = viewport.get("width")?.as_f64()?;
    let height = viewport.get("height")?.as_f64()?;
    (width.is_finite() && height.is_finite() && width >= 0.0 && height >= 0.0)
        .then_some((width, height))
}
