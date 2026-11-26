//! Argument parsing for `set_node_stroke_style` (#89, #389), split out of
//! `node_attr_tools.rs` at the 800-line cap.

use std::collections::BTreeMap;

use de_editor_core::fills::MAX_MITER_LIMIT;
use jian_ops_schema::style::StrokeEndCap;

use crate::{ToolErrorCode, ToolOutcome};

/// The names `stroke_start` / `stroke_end` take, in the schema's order.
pub(crate) const END_NAMES: [(&str, StrokeEndCap); 8] = [
    ("none", StrokeEndCap::None),
    ("round", StrokeEndCap::Round),
    ("square", StrokeEndCap::Square),
    ("line_arrow", StrokeEndCap::LineArrow),
    ("triangle_arrow", StrokeEndCap::TriangleArrow),
    ("reversed_triangle", StrokeEndCap::ReversedTriangle),
    ("circle", StrokeEndCap::Circle),
    ("diamond", StrokeEndCap::Diamond),
];

#[allow(clippy::result_large_err)]
fn parse_end(
    args: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<StrokeEndCap>, ToolOutcome> {
    let Some(raw) = args.get(key) else {
        return Ok(None);
    };
    let wanted = raw.trim().to_ascii_lowercase().replace('-', "_");
    let wanted = if wanted == "butt" {
        "none".to_owned()
    } else {
        wanted
    };
    END_NAMES
        .iter()
        .find(|(name, _)| *name == wanted)
        .map(|(_, cap)| Some(*cap))
        .ok_or_else(|| {
            let names: Vec<&str> = END_NAMES.iter().map(|(name, _)| *name).collect();
            invalid(format!(
                "{key} must be one of {}, got {raw:?}",
                names.join(" / ")
            ))
        })
}

fn invalid(message: String) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message)
}

// `ToolOutcome` is the shared MCP outcome type, as in `parse_opt_bool`.
#[allow(clippy::result_large_err)]
pub(crate) fn parse_stroke_style_args(
    args: &BTreeMap<String, String>,
) -> Result<de_editor_core::fills::StrokeStyleWrite, ToolOutcome> {
    use jian_ops_schema::style::{StrokeCap, StrokeJoin};
    let mut style = de_editor_core::fills::StrokeStyleWrite::default();
    if let Some(raw) = args.get("cap") {
        style.cap = Some(match raw.trim().to_ascii_lowercase().as_str() {
            // `butt` is the SVG / CSS spelling of the schema's `none`.
            "none" | "butt" => StrokeCap::None,
            "round" => StrokeCap::Round,
            "square" => StrokeCap::Square,
            _ => {
                return Err(invalid(format!(
                    "cap must be one of none (butt) / round / square, got {raw:?}"
                )))
            }
        });
    }
    if let Some(raw) = args.get("join") {
        style.join = Some(match raw.trim().to_ascii_lowercase().as_str() {
            "miter" => StrokeJoin::Miter,
            "round" => StrokeJoin::Round,
            "bevel" => StrokeJoin::Bevel,
            _ => {
                return Err(invalid(format!(
                    "join must be one of miter / round / bevel, got {raw:?}"
                )))
            }
        });
    }
    if let Some(raw) = args.get("dash_pattern") {
        style.dash_pattern = Some(parse_dash_pattern(raw)?);
    }
    if let Some(raw) = args.get("dash_offset") {
        match raw.trim().parse::<f32>() {
            Ok(v) if v.is_finite() => style.dash_offset = Some(v),
            _ => {
                return Err(invalid(format!(
                    "dash_offset must be a finite number, got {raw:?}"
                )))
            }
        }
    }
    style.stroke_start = parse_end(args, "stroke_start")?;
    style.stroke_end = parse_end(args, "stroke_end")?;
    if let Some(raw) = args.get("miter_limit") {
        match raw.trim().parse::<f32>() {
            Ok(v) if v.is_finite() && (1.0..=MAX_MITER_LIMIT).contains(&v) => {
                style.miter_limit = Some(v)
            }
            _ => {
                return Err(invalid(format!(
                    "miter_limit must be a number from 1 to {MAX_MITER_LIMIT}, got {raw:?}"
                )))
            }
        }
    }
    if style.is_empty() {
        return Err(ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            "set at least one of cap / join / dash_pattern / dash_offset / stroke_start / stroke_end / miter_limit".into(),
        ));
    }
    if !style.is_valid() {
        return Err(invalid(
            "dash_pattern lengths must be finite, non-negative and not all zero".into(),
        ));
    }
    Ok(style)
}

/// `"4 2"`, `"4,2"` or `"[4, 2]"`; `""`, `"none"` or `"solid"` clears.
#[allow(clippy::result_large_err)]
fn parse_dash_pattern(raw: &str) -> Result<Vec<f32>, ToolOutcome> {
    let trimmed = raw
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    if trimmed.is_empty() || matches!(trimmed.to_ascii_lowercase().as_str(), "none" | "solid") {
        return Ok(Vec::new());
    }
    trimmed
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f32>().map_err(|_| {
                invalid(format!(
                    "dash_pattern must be a list of numbers, got {raw:?}"
                ))
            })
        })
        .collect()
}
