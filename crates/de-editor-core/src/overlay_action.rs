//! Prototype overlays (#372): the `open_overlay` / `swap_overlay` /
//! `close_overlay` actions, the one shape every surface reads them in.
//!
//! An overlay shows another frame (an off-screen board, a frame on an
//! "Overlays" page, a component instance) on top of the current screen,
//! the way Figma models modals, menus, sheets and toasts. The actions are
//! ordinary `events` entries (`jian_ops_schema::events::Action`, an opaque
//! one-key map), so the document schema needs nothing new:
//!
//! ```json
//! {"open_overlay": {"target": "n50", "position": "center",
//!   "closeOnOutsideClick": true, "background": "#00000066",
//!   "animation": "move_in", "direction": "bottom", "duration": 300}}
//! {"swap_overlay": {"target": "n51", "animation": "dissolve"}}
//! {"close_overlay": null}
//! ```
//!
//! `position` is `center` (default) or one of the six edge anchors, or
//! `manual`: then the overlay's top-left sits `offset` from the trigger's
//! bottom-left corner, like a dropdown. `background` is a `#RRGGBBAA`
//! dim, absent for none. The panel, MCP, the flow graph, Preview and the
//! HTML export all parse through [`OverlayAction::parse`], so they cannot
//! disagree on defaults.

use std::collections::BTreeMap;

use jian_ops_schema::events::Action;
use serde_json::{json, Map, Value};

/// The action key that opens an overlay.
pub const OPEN_OVERLAY: &str = "open_overlay";
/// The action key that replaces the top overlay.
pub const SWAP_OVERLAY: &str = "swap_overlay";
/// The action key that closes the top overlay.
pub const CLOSE_OVERLAY: &str = "close_overlay";

/// How long an animated overlay takes to arrive, unless authored.
pub const DEFAULT_OVERLAY_DURATION_MS: u32 = 300;
/// The dim a new "background" starts at: black at 40%.
pub const DEFAULT_OVERLAY_DIM: &str = "#00000066";

/// Where an overlay sits in the screen it opens over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OverlayPosition {
    #[default]
    Center,
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    /// `offset` from the trigger's bottom-left corner.
    Manual,
}

impl OverlayPosition {
    pub const ALL: [OverlayPosition; 8] = [
        OverlayPosition::Center,
        OverlayPosition::TopLeft,
        OverlayPosition::TopCenter,
        OverlayPosition::TopRight,
        OverlayPosition::BottomLeft,
        OverlayPosition::BottomCenter,
        OverlayPosition::BottomRight,
        OverlayPosition::Manual,
    ];

    /// The stored (and MCP) spelling.
    pub fn wire_name(self) -> &'static str {
        match self {
            OverlayPosition::Center => "center",
            OverlayPosition::TopLeft => "top_left",
            OverlayPosition::TopCenter => "top_center",
            OverlayPosition::TopRight => "top_right",
            OverlayPosition::BottomLeft => "bottom_left",
            OverlayPosition::BottomCenter => "bottom_center",
            OverlayPosition::BottomRight => "bottom_right",
            OverlayPosition::Manual => "manual",
        }
    }

    /// Parse the stored spelling; a camelCase or hyphenated variant is
    /// accepted too (`topLeft`, `top-left`).
    pub fn from_wire(name: &str) -> Option<Self> {
        let folded: String = name
            .trim()
            .chars()
            .filter(|c| !matches!(c, '_' | '-' | ' '))
            .flat_map(char::to_lowercase)
            .collect();
        Self::ALL
            .into_iter()
            .find(|p| p.wire_name().replace('_', "") == folded)
    }

    /// The overlay's anchor inside the viewport as fractions
    /// `(x, y)`: `0` the start edge, `0.5` the middle, `1` the end edge.
    /// `None` for [`OverlayPosition::Manual`].
    pub fn anchor(self) -> Option<(f32, f32)> {
        Some(match self {
            OverlayPosition::Center => (0.5, 0.5),
            OverlayPosition::TopLeft => (0.0, 0.0),
            OverlayPosition::TopCenter => (0.5, 0.0),
            OverlayPosition::TopRight => (1.0, 0.0),
            OverlayPosition::BottomLeft => (0.0, 1.0),
            OverlayPosition::BottomCenter => (0.5, 1.0),
            OverlayPosition::BottomRight => (1.0, 1.0),
            OverlayPosition::Manual => return None,
        })
    }

    /// Where an overlay of `size` lands inside `viewport` (`x, y, w, h`),
    /// or at `trigger_bottom_left + offset` for a manual one (the viewport
    /// origin when the trigger is unknown).
    pub fn place(
        self,
        viewport: (f32, f32, f32, f32),
        size: (f32, f32),
        trigger_bottom_left: Option<(f32, f32)>,
        offset: (f32, f32),
    ) -> (f32, f32) {
        let (vx, vy, vw, vh) = viewport;
        match self.anchor() {
            Some((fx, fy)) => (vx + (vw - size.0) * fx, vy + (vh - size.1) * fy),
            None => {
                let (tx, ty) = trigger_bottom_left.unwrap_or((vx, vy));
                (tx + offset.0, ty + offset.1)
            }
        }
    }
}

/// The edge a "move in" overlay slides in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OverlayEdge {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl OverlayEdge {
    pub const ALL: [OverlayEdge; 4] = [
        OverlayEdge::Top,
        OverlayEdge::Bottom,
        OverlayEdge::Left,
        OverlayEdge::Right,
    ];

    pub fn wire_name(self) -> &'static str {
        match self {
            OverlayEdge::Top => "top",
            OverlayEdge::Bottom => "bottom",
            OverlayEdge::Left => "left",
            OverlayEdge::Right => "right",
        }
    }

    pub fn from_wire(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|e| e.wire_name() == name)
    }
}

/// How an overlay arrives: the transition presets Preview already plays
/// (instant, the Fade cross-dissolve, a Slide from an edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OverlayAnimation {
    #[default]
    Instant,
    Dissolve,
    MoveIn(OverlayEdge),
}

impl OverlayAnimation {
    /// The `animation` spelling (the edge rides in `direction`).
    pub fn wire_name(self) -> &'static str {
        match self {
            OverlayAnimation::Instant => "instant",
            OverlayAnimation::Dissolve => "dissolve",
            OverlayAnimation::MoveIn(_) => "move_in",
        }
    }

    /// Parse `animation` + `direction`. `fade` and `slide` (the navigate
    /// transition names) are accepted as `dissolve` and `move_in`.
    pub fn from_wire(name: &str, direction: Option<&str>) -> Option<Self> {
        let edge = || direction.and_then(OverlayEdge::from_wire).unwrap_or_default();
        match name.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "instant" | "none" => Some(OverlayAnimation::Instant),
            "dissolve" | "fade" => Some(OverlayAnimation::Dissolve),
            "move_in" | "movein" | "slide" | "slide_in" => Some(OverlayAnimation::MoveIn(edge())),
            _ => None,
        }
    }
}

/// Everything `open_overlay` says.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySpec {
    /// The overlay frame's id (any page), or an instance's id.
    pub target: String,
    pub position: OverlayPosition,
    /// Manual position only: from the trigger's bottom-left corner.
    pub offset: (f32, f32),
    /// A click outside the overlay closes it; otherwise it is blocked.
    pub close_on_outside_click: bool,
    /// The dim behind the overlay, as authored (`#RRGGBBAA`); `None` for
    /// no background.
    pub background: Option<String>,
    pub animation: OverlayAnimation,
    pub duration_ms: u32,
}

impl OverlaySpec {
    /// A centred overlay of `target` with the defaults.
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            position: OverlayPosition::Center,
            offset: (0.0, 0.0),
            close_on_outside_click: true,
            background: None,
            animation: OverlayAnimation::Instant,
            duration_ms: DEFAULT_OVERLAY_DURATION_MS,
        }
    }

    /// The dim as normalized RGBA, when there is a parseable one.
    pub fn background_rgba(&self) -> Option<[f32; 4]> {
        let hex = self.background.as_deref()?;
        de_util::hex_color::parse_hex_rgba_f32(hex, de_util::hex_color::HexOptions::LENIENT)
    }
}

/// One parsed overlay action.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayAction {
    Open(OverlaySpec),
    /// Replace the top overlay with `target`, keeping its place, dim and
    /// outside-click rule (opens one when none is open).
    Swap {
        target: String,
        animation: OverlayAnimation,
        duration_ms: u32,
    },
    Close,
}

impl OverlayAction {
    /// Whether `key` is one of the three overlay action keys.
    pub fn is_overlay_key(key: &str) -> bool {
        matches!(key, OPEN_OVERLAY | SWAP_OVERLAY | CLOSE_OVERLAY)
    }

    /// Parse one `events` action; `None` for any other action, or an open
    /// / swap without a target.
    pub fn parse(action: &Action) -> Option<Self> {
        let (key, body) = action.single()?;
        Self::parse_body(key, body)
    }

    /// Parse an action key and its body.
    pub fn parse_body(key: &str, body: &Value) -> Option<Self> {
        let text = |name: &str| {
            body.get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        let animation = || {
            text("animation")
                .and_then(|name| OverlayAnimation::from_wire(name, text("direction")))
                .unwrap_or_default()
        };
        let duration = || {
            body.get("duration")
                .and_then(Value::as_f64)
                .filter(|ms| ms.is_finite() && *ms >= 0.0)
                .map_or(DEFAULT_OVERLAY_DURATION_MS, |ms| ms.min(10_000.0) as u32)
        };
        match key {
            OPEN_OVERLAY => {
                let target = text("target")?.to_owned();
                let number = |v: Option<&Value>| v.and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let offset = body.get("offset");
                Some(OverlayAction::Open(OverlaySpec {
                    target,
                    position: text("position")
                        .and_then(OverlayPosition::from_wire)
                        .unwrap_or_default(),
                    offset: (
                        number(offset.and_then(|o| o.get("x"))),
                        number(offset.and_then(|o| o.get("y"))),
                    ),
                    close_on_outside_click: body
                        .get("closeOnOutsideClick")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    background: text("background").map(str::to_owned),
                    animation: animation(),
                    duration_ms: duration(),
                }))
            }
            SWAP_OVERLAY => Some(OverlayAction::Swap {
                target: text("target")?.to_owned(),
                animation: animation(),
                duration_ms: duration(),
            }),
            CLOSE_OVERLAY => Some(OverlayAction::Close),
            _ => None,
        }
    }

    /// The target frame an open / swap shows.
    pub fn target(&self) -> Option<&str> {
        match self {
            OverlayAction::Open(spec) => Some(&spec.target),
            OverlayAction::Swap { target, .. } => Some(target),
            OverlayAction::Close => None,
        }
    }

    /// The action key.
    pub fn key(&self) -> &'static str {
        match self {
            OverlayAction::Open(_) => OPEN_OVERLAY,
            OverlayAction::Swap { .. } => SWAP_OVERLAY,
            OverlayAction::Close => CLOSE_OVERLAY,
        }
    }

    /// The action body as stored: defaults are left out, except `target`,
    /// `position` and `closeOnOutsideClick`, which an open always names.
    pub fn body(&self) -> Value {
        let animate = |map: &mut Map<String, Value>, animation: OverlayAnimation, ms: u32| {
            if animation == OverlayAnimation::Instant {
                return;
            }
            map.insert("animation".into(), json!(animation.wire_name()));
            if let OverlayAnimation::MoveIn(edge) = animation {
                map.insert("direction".into(), json!(edge.wire_name()));
            }
            if ms != DEFAULT_OVERLAY_DURATION_MS {
                map.insert("duration".into(), json!(ms));
            }
        };
        match self {
            OverlayAction::Open(spec) => {
                let mut map = Map::new();
                map.insert("target".into(), json!(spec.target));
                map.insert("position".into(), json!(spec.position.wire_name()));
                if spec.position == OverlayPosition::Manual {
                    map.insert(
                        "offset".into(),
                        json!({ "x": spec.offset.0, "y": spec.offset.1 }),
                    );
                }
                map.insert(
                    "closeOnOutsideClick".into(),
                    json!(spec.close_on_outside_click),
                );
                if let Some(background) = &spec.background {
                    map.insert("background".into(), json!(background));
                }
                animate(&mut map, spec.animation, spec.duration_ms);
                Value::Object(map)
            }
            OverlayAction::Swap {
                target,
                animation,
                duration_ms,
            } => {
                let mut map = Map::new();
                map.insert("target".into(), json!(target));
                animate(&mut map, *animation, *duration_ms);
                Value::Object(map)
            }
            OverlayAction::Close => Value::Null,
        }
    }

    /// The `events` entry.
    pub fn to_action(&self) -> Action {
        let mut map = BTreeMap::new();
        map.insert(self.key().to_owned(), self.body());
        Action(map)
    }
}

#[cfg(test)]
#[path = "overlay_action_tests.rs"]
mod tests;
