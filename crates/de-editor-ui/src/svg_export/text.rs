//! Text nodes in SVG export, written as live text in the shape Figma's own
//! export uses when "outline text" is off: one `<text>` per node with
//! `xml:space="preserve"` and one `<tspan x y>` per line, so an importer
//! makes one editable text layer with the font family, size and weight
//! named. Styled runs become nested `<tspan>`s carrying only what differs.
//!
//! Text whose fill stack is richer than one solid (#179, the same test the
//! canvas uses, `text_fill_stack`) writes its glyphs into a `<clipPath>` and
//! paints the fill stack inside it (`fill_stack.rs`), so a photo or gradient
//! shows through the letters; that and outlined text (#282) are a `<g>` of
//! several elements.
//!
//! Lines follow the authored line breaks. The canvas also wraps
//! fixed-width text at its box edge, which needs font metrics this writer
//! does not have; such text is reported when it likely wraps.

use super::fill_stack::{emit_fill_stack, emit_fill_stack_defs};
use super::writer::Writer;
use super::{color_to_rgb, normalize_rect, svg_id, xml_escape};
use crate::layout_scene::{SceneNode, SceneTextAlign, SceneTextRun, SceneTextVerticalAlign};
use crate::text_fill_stack::text_needs_fill_stack;
use crate::Color;
use std::fmt::Write as _;

const TEXT_DEFAULT_FONT_SIZE: f32 = de_editor_core::text_defaults::FONT_SIZE;
/// Line height the writer has always used when the node sets none.
const DEFAULT_LINE_HEIGHT: f32 = 1.35;
/// Average advance of a Latin glyph in em, for the "likely wraps" warning.
const AVERAGE_ADVANCE_EM: f32 = 0.5;
const TEXT_DEFAULT_FILL: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.08,
    a: 1.0,
};

/// Write text node `n`; `attrs` are its layer attributes (id, transform,
/// opacity, …) for the one element that stands for it.
pub(super) fn emit_text(out: &mut String, n: &SceneNode, attrs: &str, w: &mut Writer) {
    let Some(text) = n.text.as_deref().filter(|t| !t.is_empty()) else {
        return;
    };
    report_text_limits(n, text, w);
    let outlined = crate::text_outline::text_outline_stroke(n).is_some();
    let stacked = text_needs_fill_stack(n);
    if !outlined && !stacked {
        emit_text_lines(out, n, text, &format!("{attrs}{}", solid_fill_attr(n)));
        return;
    }
    let _ = write!(out, r#"<g{attrs}>"#);
    super::text_outline::emit_outline_under(out, n, text);
    if stacked {
        let clip = format!("text-clip-{}", svg_id(&n.id));
        emit_fill_stack(out, n, Some(&clip), "", w);
    } else {
        emit_text_lines(out, n, text, &solid_fill_attr(n));
    }
    super::text_outline::emit_outline_over(out, n, text);
    out.push_str("</g>");
}

fn solid_fill_attr(n: &SceneNode) -> String {
    let color = n.fill.unwrap_or(TEXT_DEFAULT_FILL);
    if color.a < 0.999 {
        format!(
            r#" fill="{}" fill-opacity="{}""#,
            color_to_rgb(color),
            color.a
        )
    } else {
        format!(r#" fill="{}""#, color_to_rgb(color))
    }
}

/// Defs a text node references: the inside-outline clip, and for a fill
/// stack the glyph clip plus the layers' gradients and patterns.
pub(super) fn emit_text_fill_defs(out: &mut String, n: &SceneNode, id: &str, w: &mut Writer) {
    let Some(text) = n.text.as_deref().filter(|t| !t.is_empty()) else {
        return;
    };
    super::text_outline::emit_outline_defs(out, n, text, id);
    if !text_needs_fill_stack(n) {
        return;
    }
    let _ = write!(out, r#"<clipPath id="text-clip-{id}">"#);
    emit_text_lines(out, n, text, "");
    out.push_str("</clipPath>");
    emit_fill_stack_defs(out, n, id, w);
}

fn report_text_limits(n: &SceneNode, text: &str, w: &mut Writer) {
    if n.text_wrap && likely_wraps(n, text) {
        w.warn(
            n,
            "text wraps at its box width on the canvas; the SVG keeps only its authored line breaks",
        );
    }
    if n.text_ellipsis {
        w.warn(
            n,
            "text truncation (ellipsis) is not kept; the full text is written",
        );
    }
}

fn likely_wraps(n: &SceneNode, text: &str) -> bool {
    let size = font_size(n);
    let width = normalize_rect(n.bounds).size.x;
    text.split('\n').any(|line| {
        let glyphs = line.chars().count() as f32;
        glyphs * (size * AVERAGE_ADVANCE_EM + n.letter_spacing) > width
    })
}

fn font_size(n: &SceneNode) -> f32 {
    if n.font_size > 0.0 {
        n.font_size
    } else {
        TEXT_DEFAULT_FONT_SIZE
    }
}

/// One `<text>` with a `<tspan>` per authored line; `attrs` go on the
/// `<text>` (empty fill inside a clip).
pub(super) fn emit_text_lines(out: &mut String, n: &SceneNode, text: &str, attrs: &str) {
    emit_lines(out, n, text, attrs, text_decoration_attr(n), true);
}

/// [`emit_text_lines`] without underline / strikethrough or run colours,
/// for an outline.
pub(super) fn emit_text_outline_lines(out: &mut String, n: &SceneNode, text: &str, attrs: &str) {
    emit_lines(out, n, text, attrs, "", false);
}

fn emit_lines(
    out: &mut String,
    n: &SceneNode,
    text: &str,
    attrs: &str,
    decoration: &str,
    run_fills: bool,
) {
    let r = normalize_rect(n.bounds);
    let size = font_size(n);
    let multiplier = if n.line_height > 0.0 {
        n.line_height
    } else {
        DEFAULT_LINE_HEIGHT
    };
    let line_h = size * multiplier;
    let line_count = text.split('\n').count() as f32;
    let block_h = line_h * line_count;
    let top = match n.text_vertical_align {
        SceneTextVerticalAlign::Top => r.origin.y,
        SceneTextVerticalAlign::Middle => r.origin.y + (r.size.y - block_h) * 0.5,
        SceneTextVerticalAlign::Bottom => r.origin.y + r.size.y - block_h,
    };
    // Baseline of the first line: half the leading, then the ascent. At the
    // default 1.35 this is the `top + size + 1`-ish the export always wrote.
    let first_baseline = top + (line_h - size) * 0.5 + size * 0.8;
    let (x, anchor) = match n.text_align {
        SceneTextAlign::Left | SceneTextAlign::Justify => (r.origin.x, ""),
        SceneTextAlign::Center => (r.origin.x + r.size.x * 0.5, r#" text-anchor="middle""#),
        SceneTextAlign::Right => (r.origin.x + r.size.x, r#" text-anchor="end""#),
    };
    let family = if n.font_family.is_empty() {
        "system-ui, sans-serif"
    } else {
        &n.font_family
    };
    let _ = write!(
        out,
        r#"<text{attrs} xml:space="preserve" style="white-space: pre" font-family="{}" font-size="{size}"{}{}{anchor}{}{decoration}>"#,
        xml_escape(family),
        weight_attr(n.font_weight),
        if n.italic {
            r#" font-style="italic""#
        } else {
            ""
        },
        if n.letter_spacing != 0.0 {
            format!(r#" letter-spacing="{}""#, n.letter_spacing)
        } else {
            String::new()
        },
    );
    let mut start = 0;
    for (index, line) in text.split('\n').enumerate() {
        let y = first_baseline + line_h * index as f32;
        let _ = write!(out, r#"<tspan x="{x}" y="{y}">"#);
        emit_line_runs(out, n, start, line, run_fills);
        out.push_str("</tspan>");
        start += line.len() + 1;
    }
    out.push_str("</text>");
}

/// The text of one line (`line`, starting at byte `start` of the node's
/// text), split where the node's styled runs begin and end.
fn emit_line_runs(out: &mut String, n: &SceneNode, start: usize, line: &str, run_fills: bool) {
    let end = start + line.len();
    let mut cursor = start;
    let mut runs: Vec<&SceneTextRun> = n
        .text_runs
        .iter()
        .filter(|run| run.start < end && run.end > start && run.end > run.start)
        .collect();
    runs.sort_by_key(|run| run.start);
    for run in runs {
        let run_start = run.start.max(cursor);
        let run_end = run.end.min(end);
        if run_start >= run_end {
            continue;
        }
        push_slice(out, line, cursor - start, run_start - start);
        let attrs = run_attrs(n, run, run_fills);
        if attrs.is_empty() {
            push_slice(out, line, run_start - start, run_end - start);
        } else {
            let _ = write!(out, "<tspan{attrs}>");
            push_slice(out, line, run_start - start, run_end - start);
            out.push_str("</tspan>");
        }
        cursor = run_end;
    }
    push_slice(out, line, cursor - start, line.len());
}

fn push_slice(out: &mut String, line: &str, from: usize, to: usize) {
    if let Some(slice) = line.get(from..to) {
        out.push_str(&xml_escape(slice));
    }
}

fn run_attrs(n: &SceneNode, run: &SceneTextRun, run_fills: bool) -> String {
    let mut attrs = String::new();
    if run.font_size > 0.0 && run.font_size != n.font_size {
        let _ = write!(attrs, r#" font-size="{}""#, run.font_size);
    }
    if run.font_weight > 0 && run.font_weight != n.font_weight {
        attrs.push_str(&weight_attr(run.font_weight));
    }
    if run.italic && !n.italic {
        attrs.push_str(r#" font-style="italic""#);
    }
    if run_fills {
        if let Some(color) = run.fill {
            let _ = write!(attrs, r#" fill="{}""#, color_to_rgb(color));
            if color.a < 0.999 {
                let _ = write!(attrs, r#" fill-opacity="{}""#, color.a);
            }
        }
        let decoration = match (
            run.underline && !n.underline,
            run.strikethrough && !n.strikethrough,
        ) {
            (true, true) => r#" text-decoration="underline line-through""#,
            (true, false) => r#" text-decoration="underline""#,
            (false, true) => r#" text-decoration="line-through""#,
            (false, false) => "",
        };
        attrs.push_str(decoration);
    }
    attrs
}

fn weight_attr(weight: u16) -> String {
    if weight > 0 {
        format!(r#" font-weight="{weight}""#)
    } else {
        String::new()
    }
}

fn text_decoration_attr(n: &SceneNode) -> &'static str {
    match (n.underline, n.strikethrough) {
        (true, true) => r#" text-decoration="underline line-through""#,
        (true, false) => r#" text-decoration="underline""#,
        (false, true) => r#" text-decoration="line-through""#,
        (false, false) => "",
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
