//! Value parsing for the live-snapshot import: a node's captured styles
//! read back as a computed style, pixels, colours and rotation. Split off
//! `snapshot.rs` at the 800-line cap.

use std::collections::BTreeMap;

use jian_ops_schema::node::text::TextAlign;
use jian_ops_schema::style::{PenFill, SolidFillBody};
use serde_json::{Map, Value};

use crate::color::parse_css_color;
use crate::css::cascade::ComputedStyle;

pub(super) fn style_map(object: &Map<String, Value>) -> BTreeMap<String, String> {
    object
        .get("styles")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|styles| styles.iter())
        .filter_map(|(name, value)| {
            value
                .as_str()
                .map(|value| (name.clone(), value.to_string()))
        })
        .collect()
}

pub(super) fn computed_style(
    styles: &BTreeMap<String, String>,
    default_font_size: f64,
) -> ComputedStyle {
    let mut props = styles.clone();
    if let Some(border) = styles.get("border") {
        if let Some(width) = border
            .split_whitespace()
            .find(|part| parse_px(part).is_some())
        {
            props.insert("border-width".into(), width.to_string());
        }
        if let Some(style) = border.split_whitespace().find(|part| {
            matches!(
                part.to_ascii_lowercase().as_str(),
                "none"
                    | "hidden"
                    | "dotted"
                    | "dashed"
                    | "solid"
                    | "double"
                    | "groove"
                    | "ridge"
                    | "inset"
                    | "outset"
            )
        }) {
            props.insert("border-style".into(), style.to_string());
        }
        if let Some(color) = color_from_border(border) {
            props.insert("border-color".into(), color);
        }
    }
    let font_size = props
        .get("font-size")
        .and_then(|value| parse_px(value))
        .unwrap_or(default_font_size);
    ComputedStyle { props, font_size }
}

pub(super) fn color_from_border(border: &str) -> Option<String> {
    for prefix in ["rgba(", "rgb(", "hsla(", "hsl("] {
        if let Some(start) = border.find(prefix) {
            let end = border[start..].find(')')? + start + 1;
            return parse_css_color(&border[start..end]);
        }
    }
    border.split_whitespace().find_map(parse_css_color)
}

pub(super) fn parse_px(value: &str) -> Option<f64> {
    value
        .trim()
        .strip_suffix("px")
        .unwrap_or(value.trim())
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

pub(super) fn parse_text_align(value: &str) -> Option<TextAlign> {
    match value.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" | "end" => Some(TextAlign::Right),
        "justify" => Some(TextAlign::Justify),
        _ => None,
    }
}

pub(super) fn matrix_rotation(value: &str) -> Option<f64> {
    let body = value.trim().strip_prefix("matrix(")?.strip_suffix(')')?;
    let values: Vec<f64> = body
        .split(',')
        .map(|part| part.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    if values.len() != 6 || !values.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some(values[1].atan2(values[0]).to_degrees())
}

pub(super) fn solid_fill(color: String) -> PenFill {
    PenFill::Solid(SolidFillBody {
        color,
        explain: None,
        opacity: None,
        blend_mode: None,
    })
}
