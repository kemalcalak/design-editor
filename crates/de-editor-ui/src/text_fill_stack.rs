//! Whether a text node paints its glyphs with the full canonical fill
//! stack instead of one solid ink colour (#179). Shared by the canvas
//! painter (raster + PNG export) and the SVG writer so both agree on
//! which nodes take the clipped-fill path.

use crate::layout_scene::{SceneFillLayer, SceneNode};
use crate::ImageBlendMode;

/// `true` when the node's fill stack cannot be expressed as the single
/// solid `SceneNode::fill` the plain glyph path paints with: more than one
/// layer, any image / gradient / shader layer, or a non-Normal blend.
/// A one-layer Normal solid (and a legacy scene with no layers) stays on
/// the fast path, where `fill` already carries that colour.
pub(crate) fn text_needs_fill_stack(node: &SceneNode) -> bool {
    match node.fill_layers.as_slice() {
        [] => false,
        [SceneFillLayer::Solid { blend_mode, .. }] => *blend_mode != ImageBlendMode::Normal,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_scene::{NodeKind, SceneGradient, SceneGradientStop};
    use crate::Color;

    fn solid(blend_mode: ImageBlendMode) -> SceneFillLayer {
        SceneFillLayer::Solid {
            color: Color::BLACK,
            blend_mode,
        }
    }

    #[test]
    fn plain_solid_and_legacy_text_stay_on_the_ink_path() {
        let mut node = SceneNode::leaf("t", NodeKind::Text);
        assert!(!text_needs_fill_stack(&node));
        node.fill_layers = vec![solid(ImageBlendMode::Normal)];
        assert!(!text_needs_fill_stack(&node));
    }

    #[test]
    fn rich_layers_blends_and_stacks_take_the_fill_stack_path() {
        let mut node = SceneNode::leaf("t", NodeKind::Text);
        node.fill_layers = vec![solid(ImageBlendMode::Multiply)];
        assert!(text_needs_fill_stack(&node));
        node.fill_layers = vec![solid(ImageBlendMode::Normal), solid(ImageBlendMode::Normal)];
        assert!(text_needs_fill_stack(&node));
        node.fill_layers = vec![SceneFillLayer::Gradient {
            gradient: SceneGradient::Linear {
                angle_deg: 0.0,
                opacity: 1.0,
                stops: vec![SceneGradientStop {
                    offset: 0.0,
                    color: Color::WHITE,
                }],
            },
            blend_mode: ImageBlendMode::Normal,
        }];
        assert!(text_needs_fill_stack(&node));
    }
}
