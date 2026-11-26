//! `set_interaction`'s overlay actions (#372): `open_overlay`,
//! `swap_overlay`, `close_overlay`.
//!
//! The object an agent sends is validated here and stored in the shape
//! `de_editor_core::overlay_action` defines, so the panel, Preview and the
//! HTML export read exactly what MCP wrote. A target is a frame (or an
//! instance) on any page, named by id or by layer name.

use de_editor_core::overlay_action::{
    OverlayAction, OverlayAnimation, OverlayPosition, OverlaySpec, DEFAULT_OVERLAY_DURATION_MS,
};
use de_editor_core::PenNodeExt;
use jian_ops_schema::events::Action;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{Map, Value};

/// Build one overlay action from an MCP action object; `Ok(None)` when
/// `kind` is not an overlay action.
pub(crate) fn overlay_action(
    doc: &PenDocument,
    kind: &str,
    object: &Map<String, Value>,
) -> Result<Option<(Action, Value)>, String> {
    let text = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let action = match kind {
        "open_overlay" => {
            let target = target(doc, text("target"))?;
            let position = match text("position") {
                Some(raw) => OverlayPosition::from_wire(raw).ok_or_else(|| {
                    format!(
                        "position must be center, top_left, top_center, top_right, bottom_left, bottom_center, bottom_right or manual, got {raw}"
                    )
                })?,
                None => OverlayPosition::Center,
            };
            let offset = object.get("offset");
            let number = |value: Option<&Value>, name: &str| -> Result<f32, String> {
                match value {
                    None | Some(Value::Null) => Ok(0.0),
                    Some(v) => v
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .map(|n| n as f32)
                        .ok_or_else(|| format!("{name} must be a number")),
                }
            };
            let close_on_outside_click = match object.get("closeOnOutsideClick") {
                None | Some(Value::Null) => true,
                Some(Value::Bool(on)) => *on,
                Some(_) => return Err("closeOnOutsideClick must be true or false".into()),
            };
            let (animation, duration_ms) = animation(object)?;
            OverlayAction::Open(OverlaySpec {
                target,
                position,
                offset: (
                    number(offset.and_then(|o| o.get("x")), "offset.x")?,
                    number(offset.and_then(|o| o.get("y")), "offset.y")?,
                ),
                close_on_outside_click,
                background: background(text("background"))?,
                animation,
                duration_ms,
            })
        }
        "swap_overlay" => {
            let (animation, duration_ms) = animation(object)?;
            OverlayAction::Swap {
                target: target(doc, text("target"))?,
                animation,
                duration_ms,
            }
        }
        "close_overlay" => OverlayAction::Close,
        _ => return Ok(None),
    };
    let mut echo = action.body();
    match echo.as_object_mut() {
        Some(map) => {
            map.insert("type".into(), Value::from(kind));
            if let Some(name) = action.target().and_then(|id| target_name(doc, id)) {
                map.insert("targetName".into(), Value::from(name));
            }
        }
        None => echo = serde_json::json!({ "type": kind }),
    }
    Ok(Some((action.to_action(), echo)))
}

/// `animation` / `direction` / `duration`.
fn animation(object: &Map<String, Value>) -> Result<(OverlayAnimation, u32), String> {
    let direction = object.get("direction").and_then(Value::as_str);
    if let Some(raw) = direction {
        if de_editor_core::overlay_action::OverlayEdge::from_wire(raw).is_none() {
            return Err(format!(
                "direction must be top, bottom, left or right, got {raw}"
            ));
        }
    }
    let animation = match object.get("animation").and_then(Value::as_str) {
        Some(raw) => OverlayAnimation::from_wire(raw, direction)
            .ok_or_else(|| format!("animation must be instant, dissolve or move_in, got {raw}"))?,
        None => OverlayAnimation::Instant,
    };
    let duration_ms = match object.get("duration") {
        None | Some(Value::Null) => DEFAULT_OVERLAY_DURATION_MS,
        Some(value) => value
            .as_f64()
            .filter(|ms| ms.is_finite() && (0.0..=10_000.0).contains(ms))
            .ok_or("duration must be milliseconds between 0 and 10000")?
            as u32,
    };
    Ok((animation, duration_ms))
}

/// A `#RRGGBBAA` dim (any hex form is normalized), or none.
fn background(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if matches!(raw.to_ascii_lowercase().as_str(), "none" | "transparent") {
        return Ok(None);
    }
    let [r, g, b, a] =
        de_util::hex_color::parse_hex_rgba8(raw, de_util::hex_color::HexOptions::LENIENT)
            .ok_or_else(|| {
                format!("background must be a hex colour such as #00000066 or none, got {raw}")
            })?;
    Ok(Some(format!("#{r:02X}{g:02X}{b:02X}{a:02X}")))
}

/// The overlay's frame: an id on any page, else a top-level frame or
/// instance with that layer name (unique, case-insensitive).
fn target(doc: &PenDocument, raw: Option<&str>) -> Result<String, String> {
    let raw = raw.ok_or("needs target: the overlay frame's id or name")?;
    if de_editor_core::overlay_edit::find_overlay_target(doc, raw).is_some() {
        return Ok(raw.to_owned());
    }
    let wanted = raw.to_lowercase();
    let named: Vec<String> = top_level(doc)
        .filter(|node| {
            node.base()
                .name
                .as_deref()
                .is_some_and(|name| name.trim().to_lowercase() == wanted)
        })
        .map(|node| node.id_str().to_owned())
        .collect();
    match named.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(format!(
            "no frame {raw:?} for the overlay; give a frame id or a top-level frame name"
        )),
        _ => Err(format!(
            "{} frames are named {raw:?}; give the overlay frame's id",
            named.len()
        )),
    }
}

fn target_name(doc: &PenDocument, id: &str) -> Option<String> {
    let (_, node) = de_editor_core::overlay_edit::find_overlay_target(doc, id)?;
    node.base().name.clone().filter(|n| !n.trim().is_empty())
}

fn top_level(doc: &PenDocument) -> impl Iterator<Item = &PenNode> {
    let pages: Vec<&[PenNode]> = match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages.iter().map(|p| p.children.as_slice()).collect(),
        _ => vec![doc.children.as_slice()],
    };
    pages.into_iter().flatten()
}
