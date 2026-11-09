//! Text lines for the auto-layout pass: wrapped paragraphs back as one text
//! node, and the line box a run takes in its parent's flow (`line_box`).
//!
//! The capture splits a bare text run the browser wrapped into one node per
//! line box (see `buildText` in the extractor): exact for an absolute
//! import, but a paragraph made of separate lines cannot re-wrap when its
//! words or its width change. Consecutive one-line runs that share their
//! styles and left edge and follow each other line by line are folded back
//! into one run, which the layout then gives its parent's width. A line that
//! ends well short of the box's width ended at a `<br>` (or the paragraph
//! ended), so the next line starts a new run and the hard break survives.

use std::borrow::Cow;

use serde_json::{Map, Value};

use super::super::parse_px;
use super::{style, Rect};

/// A line ending this close to the content box's right edge was wrapped.
const WRAPPED_FILL: f64 = 0.7;

/// `children` of a box `width` wide, with every wrapped paragraph folded
/// into one text run. Children pass through borrowed; only a folded
/// paragraph is new.
pub(in crate::snapshot) fn merge_wrapped_lines(
    children: &[Value],
    width: f64,
) -> Vec<Cow<'_, Value>> {
    let mut out: Vec<Cow<'_, Value>> = Vec::with_capacity(children.len());
    let mut run: Vec<&Value> = Vec::new();
    for child in children {
        if !child.as_object().is_some_and(is_plain_line) {
            flush(&mut run, &mut out);
            out.push(Cow::Borrowed(child));
            continue;
        }
        if let (Some(previous), Some(line)) = (
            run.last().and_then(|previous| previous.as_object()),
            child.as_object(),
        ) {
            if !continues(previous, line, width) {
                flush(&mut run, &mut out);
            }
        }
        run.push(child);
    }
    flush(&mut run, &mut out);
    out
}

fn flush<'a>(run: &mut Vec<&'a Value>, out: &mut Vec<Cow<'a, Value>>) {
    match run.len() {
        0 => {}
        1 => out.push(Cow::Borrowed(run[0])),
        _ => {
            let lines: Vec<&Map<String, Value>> =
                run.iter().filter_map(|line| line.as_object()).collect();
            out.push(Cow::Owned(Value::Object(fold(&lines))));
        }
    }
    run.clear();
}

/// A one-line text run without per-segment styling.
fn is_plain_line(object: &Map<String, Value>) -> bool {
    object.get("kind").and_then(Value::as_str) == Some("text")
        && object.get("lines").and_then(Value::as_u64).unwrap_or(1) <= 1
        && object.get("segments").is_none()
        && Rect::from_node(object).is_some()
}

/// Is `next` the line right below `previous` in the same paragraph?
fn continues(previous: &Map<String, Value>, next: &Map<String, Value>, width: f64) -> bool {
    let (Some(a), Some(b)) = (Rect::from_node(previous), Rect::from_node(next)) else {
        return false;
    };
    previous.get("styles") == next.get("styles")
        && a.w >= width * WRAPPED_FILL
        && (a.x - b.x).abs() <= 1.0
        && b.y >= a.y + a.h * 0.5
        && b.y <= a.y + a.h * 2.5
}

fn fold(run: &[&Map<String, Value>]) -> Map<String, Value> {
    let mut text = String::new();
    let (mut left, mut top) = (f64::INFINITY, f64::INFINITY);
    let (mut right, mut bottom) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for line in run {
        let piece = line
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !text.is_empty() && !text.ends_with('-') && !piece.is_empty() {
            text.push(' ');
        }
        text.push_str(piece);
        if let Some(rect) = Rect::from_node(line) {
            left = left.min(rect.x);
            top = top.min(rect.y);
            right = right.max(rect.x + rect.w);
            bottom = bottom.max(rect.y + rect.h);
        }
    }
    let mut merged = run[0].clone();
    merged.insert("text".into(), Value::String(text));
    merged.insert("lines".into(), Value::from(run.len() as u64));
    merged.insert(
        "rect".into(),
        serde_json::json!({ "x": left, "y": top, "w": right - left, "h": bottom - top }),
    );
    merged
}

/// Text line box: the run's line-height times its lines (the height the
/// editor gives the text node), centred on the glyph box the capture
/// measured. A tight line-height (`leading-none` on a large heading) makes
/// the line box shorter than the glyphs; the page lays the run out by its
/// line box all the same, so the glyphs' overhang must not count as the
/// run reaching past its parent's edge.
pub(in crate::snapshot) fn line_box(
    object: &Map<String, Value>,
    rect: Rect,
    default_font: f64,
) -> Rect {
    let font_size = style(object, "font-size")
        .and_then(parse_px)
        .unwrap_or(default_font);
    let line_height = style(object, "line-height").and_then(parse_px);
    let lines = object.get("lines").and_then(Value::as_u64);
    let height = match (line_height, lines) {
        (Some(line_height), Some(lines)) => line_height * lines.max(1) as f64,
        // `line-height: normal` (the line box is the font's own, which is
        // what the glyph box measured) or a capture without a line count:
        // the estimate only ever grows the measured box.
        (line_height, lines) => {
            (line_height.unwrap_or(font_size * 1.2) * lines.unwrap_or(1).max(1) as f64).max(rect.h)
        }
    };
    Rect {
        y: rect.y - (height - rect.h) / 2.0,
        h: height,
        ..rect
    }
}
