//! SkSL shader-effect layers (#6) around one node's paint.
//!
//! Backdrop passes (`uniform shader background;`) open inside the node
//! silhouette before the node paints, in list order, so each later pass
//! reads the earlier one's output as its backdrop. Object passes
//! (`uniform shader object;`) capture the node's whole output; they are
//! pushed last-first so the first listed pass is innermost and runs first.
//! Both are skipped during gestures, like the blur layers.

use crate::layout_scene::{Effect, NodeKind, SceneNode, ShaderEffect, ShaderEffectInput};
use crate::widgets::canvas_viewport_overlay::scaled_non_uniform_corner_radii;
use crate::widgets::canvas_viewport_overlay::with_canvas_uniforms;
use crate::widgets::PaintCx;
use crate::Rect;

/// The node's shader passes that sample `input`, in list order.
fn passes(node: &SceneNode, input: ShaderEffectInput) -> impl Iterator<Item = &ShaderEffect> {
    node.effects.iter().filter_map(move |effect| match effect {
        Effect::Shader(pass) if pass.input == input => Some(pass),
        _ => None,
    })
}

fn push_pass(cx: &mut PaintCx<'_>, pass: &ShaderEffect, world_rect: Rect, backdrop: bool) {
    with_canvas_uniforms(&pass.sksl, &pass.uniforms, world_rect, |uniforms| {
        cx.backend
            .push_shader_effect_layer(world_rect, &pass.sksl, uniforms, backdrop);
    });
}

/// Clip to the node silhouette the way background blur does (ellipse,
/// per-corner or uniform rounded rect, else the box).
pub(super) fn clip_node_silhouette(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
) {
    let radius = if node.kind == NodeKind::Ellipse {
        world_rect.size.x.min(world_rect.size.y) / 2.0
    } else {
        node.corner_radius * zoom
    };
    if let Some(radii) = scaled_non_uniform_corner_radii(node, zoom) {
        cx.backend.clip_round_rect_per_corner(world_rect, radii);
    } else if radius > 0.5 {
        cx.backend.clip_round_rect(world_rect, radius);
    } else {
        cx.backend.clip_rect(world_rect);
    }
}

/// Open the backdrop passes. Returns how many `restore`s balance them
/// (the silhouette clip save plus one per pass; `0` when none ran).
pub(super) fn push_backdrop_shader_effects(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
) -> usize {
    if world_rect.size.x <= 0.0 || world_rect.size.y <= 0.0 {
        return 0;
    }
    let mut pushed = 0;
    for pass in passes(node, ShaderEffectInput::Background) {
        if pushed == 0 {
            cx.backend.save();
            clip_node_silhouette(cx, node, world_rect, zoom);
            pushed = 1;
        }
        push_pass(cx, pass, world_rect, true);
        pushed += 1;
    }
    pushed
}

/// Open the object passes, last listed outermost. Returns how many
/// `restore`s balance them.
pub(super) fn push_object_shader_effects(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
) -> usize {
    if world_rect.size.x <= 0.0 || world_rect.size.y <= 0.0 {
        return 0;
    }
    let object: Vec<&ShaderEffect> = passes(node, ShaderEffectInput::Object).collect();
    for pass in object.iter().rev() {
        push_pass(cx, pass, world_rect, false);
    }
    object.len()
}

/// Pop `count` layers opened by the two pushes above.
pub(super) fn restore_shader_effects(cx: &mut PaintCx<'_>, count: usize) {
    for _ in 0..count {
        cx.backend.restore();
    }
}
