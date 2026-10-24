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

/// The node's shader passes that sample `input`, in list order, with
/// their index in the effect list.
fn passes(
    node: &SceneNode,
    input: ShaderEffectInput,
) -> impl Iterator<Item = (usize, &ShaderEffect)> {
    node.effects
        .iter()
        .enumerate()
        .filter_map(move |(index, effect)| match effect {
            Effect::Shader(pass) if pass.input == input => Some((index, pass)),
            _ => None,
        })
}

/// Open one pass; returns the backdrop texture it captured, which the
/// caller releases once the pass closed.
fn push_pass(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    index: usize,
    pass: &ShaderEffect,
    world_rect: Rect,
    backdrop: bool,
) -> Option<u64> {
    // An object pass may also sample the backdrop and its own SDF (#165).
    let mut bound = None;
    let mut captured = None;
    if !backdrop {
        let mut uniforms = pass.uniforms.clone();
        captured = crate::widgets::canvas_shader_textures::bind_effect_textures(
            cx,
            node,
            index,
            &pass.sksl,
            world_rect,
            &mut uniforms,
        );
        if uniforms != pass.uniforms {
            bound = Some(uniforms);
        }
    }
    let uniforms = bound.as_ref().unwrap_or(&pass.uniforms);
    with_canvas_uniforms(&pass.sksl, uniforms, world_rect, |uniforms| {
        crate::widgets::canvas_shader_images::prepare_shader_images(
            cx, &pass.sksl, uniforms, world_rect,
        );
        if backdrop {
            crate::widgets::canvas_animation_regions::mark_backdrop();
        }
        cx.backend
            .push_shader_effect_layer(world_rect, &pass.sksl, uniforms, backdrop);
    });
    captured
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

/// Open the backdrop passes (none unless `enabled`). Returns how many `restore`s balance them
/// (the silhouette clip save plus one per pass; `0` when none ran).
pub(super) fn push_backdrop_shader_effects(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    zoom: f32,
    enabled: bool,
) -> usize {
    if !enabled || world_rect.size.x <= 0.0 || world_rect.size.y <= 0.0 {
        return 0;
    }
    let mut pushed = 0;
    for (index, pass) in passes(node, ShaderEffectInput::Background) {
        if pushed == 0 {
            cx.backend.save();
            clip_node_silhouette(cx, node, world_rect, zoom);
            pushed = 1;
        }
        push_pass(cx, node, index, pass, world_rect, true);
        pushed += 1;
    }
    pushed
}

/// The layers [`push_object_shader_effects`] opened: how many `restore`s
/// balance them and the backdrop textures to release after.
#[derive(Default)]
pub(super) struct ObjectPasses {
    pub(super) count: usize,
    pub(super) textures: Vec<u64>,
}

/// Open the object passes (none unless `enabled`), last listed outermost.
pub(super) fn push_object_shader_effects(
    cx: &mut PaintCx<'_>,
    node: &SceneNode,
    world_rect: Rect,
    enabled: bool,
) -> ObjectPasses {
    if !enabled || world_rect.size.x <= 0.0 || world_rect.size.y <= 0.0 {
        return ObjectPasses::default();
    }
    let object: Vec<(usize, &ShaderEffect)> = passes(node, ShaderEffectInput::Object).collect();
    let mut textures = Vec::new();
    for (index, pass) in object.iter().rev() {
        textures.extend(push_pass(cx, node, *index, pass, world_rect, false));
    }
    ObjectPasses {
        count: object.len(),
        textures,
    }
}

/// Pop `count` layers opened by the two pushes above.
pub(super) fn restore_shader_effects(cx: &mut PaintCx<'_>, count: usize) {
    for _ in 0..count {
        cx.backend.restore();
    }
}

/// Close the object passes and release their backdrop textures.
pub(super) fn restore_object_shader_effects(cx: &mut PaintCx<'_>, passes: &ObjectPasses) {
    restore_shader_effects(cx, passes.count);
    for id in &passes.textures {
        cx.backend.release_shader_texture(*id);
    }
}
