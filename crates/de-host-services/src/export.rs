//! Compatibility surface over the standalone render export core.
//!
//! Headless screenshot capture remains a host service because it adds MCP
//! base64 transport semantics. All file rendering lives in
//! [`de_render_export`] so desktop and server share one renderer.

pub use de_render_export::*;

// `capture_scene` / `CaptureSpec` / `ScreenshotPng` back both the optional
// debug MCP tool and the always-on vision-validation provider.
pub mod screenshot;

#[cfg(test)]
pub mod test_support {
    use de_editor_ui::layout_scene::NodeKind;
    use de_editor_ui::layout_scene::{LayoutScene, SceneNode, ScenePage};
    use de_editor_ui::{Color, Rect};

    pub fn scene_with(children: Vec<SceneNode>) -> LayoutScene {
        LayoutScene {
            pages: vec![ScenePage {
                id: "p1".into(),
                name: "Page 1".into(),
                children,
            }],
            active_page_index: 0,
        }
    }

    pub fn filled_rect(id: &str, x: f32, y: f32, w: f32, h: f32, fill: Color) -> SceneNode {
        let mut node = SceneNode::leaf(id, NodeKind::Rect);
        node.bounds = Rect::xywh(x, y, w, h);
        node.fill = Some(fill);
        node
    }

    /// An 8x8 rect whose shader paints `fract(time)` into red, so captures at
    /// different shader times differ (#8 `timeMs` tests).
    pub fn time_shader_rect(id: &str) -> SceneNode {
        use de_editor_ui::layout_scene::{SceneFillType, SceneShader};
        let mut node = filled_rect(id, 0.0, 0.0, 8.0, 8.0, Color::WHITE);
        node.fill_type = SceneFillType::Shader;
        node.shader = Some(SceneShader {
            sksl:
                "uniform float time; half4 main(float2 p){ return half4(fract(time), 0.0, 0.0, 1.0); }"
                    .to_string(),
            uniforms: Vec::new(),
            opacity: 1.0,
            fallback: Color::WHITE,
        });
        node
    }
}
