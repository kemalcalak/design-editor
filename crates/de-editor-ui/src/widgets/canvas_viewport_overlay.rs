//! Selection overlay + fill node-paint helpers split out of
//! `canvas_viewport.rs` to keep that file under the 800-line ceiling. The
//! stroke helpers live in `canvas_viewport_stroke_paint.rs`; the
//! `paint_node_stroke` re-export keeps their old path.

use crate::layout_scene::{SceneGradient, SceneNode, SceneShader, SceneStrokeAlign};
use crate::theme::Theme;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};

// The legacy `paint_pen_rubber_band` (node-stroke-colored straight
// line) was retired in favor of `canvas_path_overlay.rs`, which ports
// the TS pen preview verbatim (dashed selection-blue rubber band +
// anchor/handle dots).

/// Paint a 1 px primary-tinted outline around `world_rect` plus
/// 8 handle anchors. Container selections paint with a softened
/// outline alpha so child content stays visually dominant. When
/// `show_handles` is false the outline paints without the 8 grab
/// dots — used for multi-select where only one bounding box per
/// node is shown (Figma parity; handles only on single-select).
pub fn paint_selection_overlay(
    cx: &mut PaintCx<'_>,
    world_rect: Rect,
    theme: &Theme,
    is_container: bool,
    show_handles: bool,
) {
    // Outline — draw as 4 individual stroke_line calls so the
    // skia AA path runs (stroke_rect goes through jian which
    // doesn't enable AA by default).
    let left = world_rect.origin.x;
    let right = world_rect.origin.x + world_rect.size.x;
    let top = world_rect.origin.y;
    let bottom = world_rect.origin.y + world_rect.size.y;
    let stroke_w = 1.5;
    let outline_color = if is_container {
        crate::Color {
            r: theme.primary.r,
            g: theme.primary.g,
            b: theme.primary.b,
            a: theme.primary.a * 0.55,
        }
    } else {
        theme.primary
    };
    cx.backend.stroke_line(
        Point2D::new(left, top),
        Point2D::new(right, top),
        outline_color,
        stroke_w,
    );
    cx.backend.stroke_line(
        Point2D::new(right, top),
        Point2D::new(right, bottom),
        outline_color,
        stroke_w,
    );
    cx.backend.stroke_line(
        Point2D::new(right, bottom),
        Point2D::new(left, bottom),
        outline_color,
        stroke_w,
    );
    cx.backend.stroke_line(
        Point2D::new(left, bottom),
        Point2D::new(left, top),
        outline_color,
        stroke_w,
    );

    if !show_handles {
        return;
    }

    // Corner handles only (#30). The edge midpoints keep their resize hit
    // zones (`canvas_viewport/hit_test.rs`) — a press on an edge still
    // resizes it; they just don't paint a square.
    let anchors = [(left, top), (right, top), (right, bottom), (left, bottom)];

    let handle_size = 8.0;
    let half = handle_size / 2.0;
    let radius = 1.5;
    let bg = crate::Color::WHITE;
    for (x, y) in anchors {
        let rect = Rect {
            origin: Point2D::new(x - half, y - half),
            size: Point2D::new(handle_size, handle_size),
        };
        cx.backend.fill_round_rect(rect, radius, bg);
        cx.backend
            .stroke_round_rect(rect, radius, theme.primary, 1.5);
    }
}

/// Paint a node's fill rect followed by its stroke rect. Stroke
/// width is scaled by `zoom` so it stays visually constant under
/// canvas zoom. Effective `fill` is passed explicitly — the scene's
/// `SceneNode.fill` is already `$ref`-resolved by the layout-scene
/// builder, so the caller forwards it straight through.
pub fn paint_fill_then_stroke(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    fill: Option<crate::Color>,
) {
    paint_node_fill(cx, node, world_rect, zoom, fill);
    paint_node_stroke(cx, node, world_rect, zoom);
}

/// Paint only a node's fill. Recursing containers use this before their
/// children, then overlay the outline after the child clip is restored.
pub(crate) fn paint_node_fill(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    fill: Option<crate::Color>,
) {
    let r = node.corner_radius * zoom;
    let use_round = r > 0.5;
    let per_corner = scaled_non_uniform_corner_radii(node, zoom);
    // A native SkSL shader fill wins over everything (gradient / solid)
    // when present. Gradients in turn win over solid `fill`. The scene
    // builder leaves the first stop's / fallback colour in `fill` for
    // paint paths that don't grok the richer body, so the shader / gradient
    // body is the more faithful representation here.
    if let Some(shader) = node.shader.as_ref() {
        paint_shader_rect(
            cx,
            shader,
            world_rect,
            if use_round { r } else { 0.0 },
            per_corner,
        );
    } else if let Some(gradient) = node.gradient.as_ref() {
        paint_gradient_rect(
            cx,
            gradient,
            world_rect,
            if use_round { r } else { 0.0 },
            per_corner,
        );
    } else if let Some(fill) = fill {
        if use_round {
            if let Some(radii) = per_corner {
                cx.backend
                    .fill_round_rect_per_corner(world_rect, radii, fill);
            } else {
                cx.backend.fill_round_rect(world_rect, r, fill);
            }
        } else {
            cx.backend.fill_rect(world_rect, fill);
        }
    }
}

pub(crate) use super::canvas_viewport_stroke_paint::paint_node_stroke;

pub(crate) fn scaled_non_uniform_corner_radii(node: &SceneNode, zoom: f32) -> Option<[f32; 4]> {
    node.corner_radii
        .map(|radii| radii.map(|value| value * zoom))
        .filter(|radii| {
            radii
                .iter()
                .skip(1)
                .any(|radius| (*radius - radii[0]).abs() > f32::EPSILON)
        })
}

/// Backends stroke centered on the rect path; INSIDE / OUTSIDE
/// alignment shifts the path by half a width so the painted band
/// lands inside / outside the node edge (Figma defaults to INSIDE).
/// The corner radius follows the shifted path.
pub fn align_stroke_rect(
    rect: Rect,
    radius: f32,
    width: f32,
    align: SceneStrokeAlign,
) -> (Rect, f32) {
    let half = width / 2.0;
    match align {
        SceneStrokeAlign::Center => (rect, radius),
        SceneStrokeAlign::Inside => (
            Rect::xywh(
                rect.origin.x + half,
                rect.origin.y + half,
                (rect.size.x - width).max(0.0),
                (rect.size.y - width).max(0.0),
            ),
            (radius - half).max(0.0),
        ),
        SceneStrokeAlign::Outside => (
            Rect::xywh(
                rect.origin.x - half,
                rect.origin.y - half,
                rect.size.x + width,
                rect.size.y + width,
            ),
            if radius > 0.0 { radius + half } else { 0.0 },
        ),
    }
}

/// Dispatch a [`SceneGradient`] onto the right `RenderBackend`
/// gradient method. Stops are flattened into `(offset, Color)` pairs
/// because the trait stays free of scene-specific types.
pub(crate) fn paint_gradient_rect(
    cx: &mut PaintCx<'_>,
    gradient: &SceneGradient,
    rect: Rect,
    corner_radius: f32,
    corner_radii: Option<[f32; 4]>,
) {
    match gradient {
        SceneGradient::Linear {
            angle_deg,
            opacity,
            stops,
        } => {
            let flat: Vec<(f32, Color)> = stops.iter().map(|s| (s.offset, s.color)).collect();
            if let Some(radii) = corner_radii {
                cx.backend.fill_round_rect_linear_gradient_per_corner(
                    rect, radii, &flat, *angle_deg, *opacity,
                );
            } else {
                cx.backend.fill_round_rect_linear_gradient(
                    rect,
                    corner_radius,
                    &flat,
                    *angle_deg,
                    *opacity,
                );
            }
        }
        SceneGradient::Radial {
            cx: gx,
            cy,
            radius,
            opacity,
            stops,
        } => {
            let flat: Vec<(f32, Color)> = stops.iter().map(|s| (s.offset, s.color)).collect();
            if let Some(radii) = corner_radii {
                cx.backend.fill_round_rect_radial_gradient_per_corner(
                    rect, radii, &flat, *gx, *cy, *radius, *opacity,
                );
            } else {
                cx.backend.fill_round_rect_radial_gradient(
                    rect,
                    corner_radius,
                    &flat,
                    *gx,
                    *cy,
                    *radius,
                    *opacity,
                );
            }
        }
        SceneGradient::Angular {
            cx: gx,
            cy,
            rotation_deg,
            opacity,
            stops,
        } => {
            let flat: Vec<(f32, Color)> = stops.iter().map(|s| (s.offset, s.color)).collect();
            if let Some(radii) = corner_radii {
                cx.backend.fill_round_rect_angular_gradient_per_corner(
                    rect,
                    radii,
                    &flat,
                    *gx,
                    *cy,
                    *rotation_deg,
                    *opacity,
                );
            } else {
                cx.backend.fill_round_rect_angular_gradient(
                    rect,
                    corner_radius,
                    &flat,
                    *gx,
                    *cy,
                    *rotation_deg,
                    *opacity,
                );
            }
        }
        SceneGradient::Mesh {
            rows,
            cols,
            colors,
            opacity,
        } => {
            if let Some(radii) = corner_radii {
                cx.backend.fill_round_rect_mesh_gradient_per_corner(
                    rect, radii, *rows, *cols, colors, *opacity,
                );
            } else {
                cx.backend.fill_round_rect_mesh_gradient(
                    rect,
                    corner_radius,
                    *rows,
                    *cols,
                    colors,
                    *opacity,
                );
            }
        }
    }
}

/// Dispatch a [`SceneShader`] onto the `RenderBackend` shader method.
/// Uniforms are flattened into `(name, values)` slices because the
/// backend trait stays free of scene-specific types. Native Skia and the
/// CanvasKit web host run the program; other backends paint `fallback` solid.
/// A shader that declares `uniform float time;` also gets the canvas clock
/// (see [`super::canvas_shader_clock`]), and one that declares
/// `uniform float2 mouse;` the node-local pointer (see
/// [`super::canvas_shader_pointer`]); both override an authored value of the
/// same name the way the loader overrides `size`. Images bound to
/// `uniform shader` children are made resident first
/// (see [`super::canvas_shader_images`]).
pub(crate) fn paint_shader_rect(
    cx: &mut PaintCx<'_>,
    shader: &SceneShader,
    rect: Rect,
    corner_radius: f32,
    corner_radii: Option<[f32; 4]>,
) {
    with_canvas_uniforms(&shader.sksl, &shader.uniforms, rect, |uniforms| {
        super::canvas_shader_images::prepare_shader_images(cx, &shader.sksl, uniforms, rect);
        if let Some(radii) = corner_radii {
            cx.backend.fill_round_rect_shader_per_corner(
                rect,
                radii,
                &shader.sksl,
                uniforms,
                shader.opacity,
                shader.fallback,
            );
        } else {
            cx.backend.fill_round_rect_shader(
                rect,
                corner_radius,
                &shader.sksl,
                uniforms,
                shader.opacity,
                shader.fallback,
            );
        }
    });
}

/// Run `f` with the bindings a program painted into `rect` receives: the
/// resolved `uniforms` plus the canvas clock (`time`) and node-local pointer
/// (`mouse`) when `sksl` declares them, each overriding an authored value of
/// the same name. Shared by shader fills and shader effects (#6).
pub(crate) fn with_canvas_uniforms<R>(
    sksl: &str,
    uniforms: &[crate::layout_scene::SceneShaderUniform],
    rect: Rect,
    f: impl FnOnce(&[(&str, &[f32])]) -> R,
) -> R {
    use super::canvas_shader_clock::{time_uniform_value, TIME_UNIFORM};
    use super::canvas_shader_pointer::{mouse_uniform_value, MOUSE_UNIFORM};
    let time = time_uniform_value(sksl).map(|seconds| [seconds]);
    let mut bound: Vec<(&str, &[f32])> = uniforms
        .iter()
        .filter(|u| time.is_none() || u.name != TIME_UNIFORM)
        .map(|u| (u.name.as_str(), u.values.as_slice()))
        .collect();
    if let Some(time) = time.as_ref() {
        bound.push((TIME_UNIFORM, time.as_slice()));
    }
    let mouse = mouse_uniform_value(sksl, rect, &bound);
    if let Some(mouse) = mouse.as_ref() {
        bound.retain(|(name, _)| *name != MOUSE_UNIFORM);
        bound.push((MOUSE_UNIFORM, mouse.as_slice()));
    }
    f(&bound)
}
