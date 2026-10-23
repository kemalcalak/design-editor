//! The borderless window's title strip.
//!
//! The editor has no title bar (#30), so the window chrome lives in a
//! 40 px strip across the top of the left panel — the same strip Home
//! reserves above its sidebar. It carries the macOS traffic lights (or
//! the custom close / minimise / maximise dots on Windows / Linux) and
//! is where a press drags the window. When the left panel is closed the
//! strip's controls sit over the canvas top-left instead, and the file
//! pill moves right to clear them (see `host_canvas_geometry`).

use crate::{Color, Point2D, Rect, RenderBackend};

/// Height of the window strip. The native traffic lights are centred on
/// it (`de-host-desktop`'s `reposition_traffic_lights` inset), so it is a
/// shared constant rather than a per-surface choice.
pub const WINDOW_STRIP_H: f32 = 40.0;

/// Whether this build owns a window at all. The browser host has none:
/// no controls, no strip reservation, nothing to drag.
pub const WINDOW_CONTROLS_AVAILABLE: bool = !cfg!(target_arch = "wasm32");

/// Left padding of the custom dot cluster.
const PAD: f32 = 12.0;
/// Diameter of a macOS-style window-control dot.
const TRAFFIC_DOT: f32 = 12.0;
/// Centre-to-centre spacing of the 3 window-control dots.
const TRAFFIC_STEP: f32 = 20.0;
/// Horizontal span the window controls reserve at the window's left
/// edge. macOS uses the *native* traffic-light buttons — they end
/// ~x=70, so reserve enough that the app chrome clears them. Windows /
/// Linux paint the custom dots from `PAD`, so the reservation is the
/// dot cluster plus a small gap.
pub const TRAFFIC_CLUSTER_W: f32 = if cfg!(target_os = "macos") {
    66.0
} else {
    TRAFFIC_STEP * 2.0 + TRAFFIC_DOT + 16.0
};

/// A window-control dot. Resolved by [`window_control_at`]; the desktop
/// runner maps each onto the matching winit `Window` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControl {
    /// Red dot — close the window / quit.
    Close,
    /// Yellow dot — minimise the window.
    Minimize,
    /// Green dot — toggle maximised.
    Maximize,
}

/// Width the window controls take from the window's left edge. `0` on
/// the web (no window) and in macOS fullscreen, where the native traffic
/// lights hide and the gap would be dead space.
pub fn window_controls_inset(fullscreen: bool) -> f32 {
    if !WINDOW_CONTROLS_AVAILABLE || (fullscreen && cfg!(target_os = "macos")) {
        0.0
    } else {
        TRAFFIC_CLUSTER_W
    }
}

/// Bounds of the 3-dot cluster at the window's top-left — the cursor-move
/// handler uses this to drive `window_control_hover` (the glyph reveal).
pub fn window_controls_rect() -> Rect {
    Rect::xywh(PAD, 0.0, TRAFFIC_STEP * 2.0 + TRAFFIC_DOT, WINDOW_STRIP_H)
}

/// Resolve a press on the custom window-control dots. `None` anywhere
/// else, and always on macOS (native traffic lights) and the web.
pub fn window_control_at(point: Point2D) -> Option<WindowControl> {
    if !WINDOW_CONTROLS_AVAILABLE || cfg!(target_os = "macos") {
        return None;
    }
    if point.y < 0.0 || point.y >= WINDOW_STRIP_H {
        return None;
    }
    let first_cx = PAD + TRAFFIC_DOT / 2.0;
    [
        WindowControl::Close,
        WindowControl::Minimize,
        WindowControl::Maximize,
    ]
    .into_iter()
    .enumerate()
    .find_map(|(i, ctl)| {
        let dot_cx = first_cx + i as f32 * TRAFFIC_STEP;
        // Square slop around the dot — adjacent zones tile without
        // overlap (±TRAFFIC_STEP/2 in x).
        ((point.x - dot_cx).abs() <= TRAFFIC_STEP / 2.0).then_some(ctl)
    })
}

/// Paint the Windows / Linux window-control dots at `bar`'s left edge
/// (macOS keeps the native traffic lights, so this paints nothing
/// there). Shared by the editor's window strip and Home, which both sit under
/// the borderless window's title strip; `hover` reveals each dot's glyph.
pub fn paint_window_control_dots(backend: &mut dyn RenderBackend, bar: Rect, hover: bool) {
    let rect = bar;
    let center_y = rect.origin.y + rect.size.y / 2.0;
    if !cfg!(target_os = "macos") {
        let traffic = [
            Color {
                r: 1.0,
                g: 0.373,
                b: 0.341,
                a: 1.0,
            }, // #FF5F57
            Color {
                r: 0.996,
                g: 0.737,
                b: 0.18,
                a: 1.0,
            }, // #FEBC2E
            Color {
                r: 0.157,
                g: 0.784,
                b: 0.251,
                a: 1.0,
            }, // #28C840
        ];
        let first_dot_cx = rect.origin.x + PAD + TRAFFIC_DOT / 2.0;
        for (i, color) in traffic.into_iter().enumerate() {
            let dot_cx = first_dot_cx + i as f32 * TRAFFIC_STEP;
            backend.fill_oval(
                Rect {
                    origin: Point2D::new(dot_cx - TRAFFIC_DOT / 2.0, center_y - TRAFFIC_DOT / 2.0),
                    size: Point2D::new(TRAFFIC_DOT, TRAFFIC_DOT),
                },
                color,
            );
            // Hovering the cluster reveals each dot's glyph
            // (macOS-style): ✕ close, − minimise, + maximise.
            if hover {
                let glyph = Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.55,
                };
                let r = 3.0_f32;
                let lw = 1.3_f32;
                match i {
                    0 => {
                        backend.stroke_line(
                            Point2D::new(dot_cx - r, center_y - r),
                            Point2D::new(dot_cx + r, center_y + r),
                            glyph,
                            lw,
                        );
                        backend.stroke_line(
                            Point2D::new(dot_cx - r, center_y + r),
                            Point2D::new(dot_cx + r, center_y - r),
                            glyph,
                            lw,
                        );
                    }
                    1 => {
                        backend.stroke_line(
                            Point2D::new(dot_cx - r, center_y),
                            Point2D::new(dot_cx + r, center_y),
                            glyph,
                            lw,
                        );
                    }
                    _ => {
                        backend.stroke_line(
                            Point2D::new(dot_cx - r, center_y),
                            Point2D::new(dot_cx + r, center_y),
                            glyph,
                            lw,
                        );
                        backend.stroke_line(
                            Point2D::new(dot_cx, center_y - r),
                            Point2D::new(dot_cx, center_y + r),
                            glyph,
                            lw,
                        );
                    }
                }
            }
        }
    }
}
