//! Shader textures (#165) over the CanvasKit bridge: the `RenderBackend`
//! items, expanded inside the impl in `backend.rs` (one impl block at the
//! 800-line cap). The bridge keeps the textures (`op_ck_bridge.js`,
//! `frameTextures`); ids cross as two u32 halves, as image ids do.

/// `id` as the `(lo, hi)` halves the bridge keys textures by.
pub(super) fn halves(id: u64) -> (u32, u32) {
    (id as u32, (id >> 32) as u32)
}

macro_rules! shader_texture_items {
    () => {
        fn push_shader_effect_layer(
            &mut self,
            rect: Rect,
            sksl: &str,
            uniforms: &[(&str, &[f32])],
            backdrop: bool,
        ) {
            ops::push_shader_effect_layer(&self.ck, rect, sksl, uniforms, backdrop);
        }
        fn supports_shader_textures(&self) -> bool {
            true
        }
        fn capture_shader_backdrop(&mut self, id: u64, rect: Rect) {
            let (lo, hi) = super::shader_textures::halves(id);
            self.ck.capture_shader_backdrop(
                lo,
                hi,
                rect.origin.x,
                rect.origin.y,
                rect.size.x,
                rect.size.y,
            );
        }
        fn begin_shader_texture(&mut self, id: u64, rect: Rect) {
            let (lo, hi) = super::shader_textures::halves(id);
            self.ck.begin_shader_texture(
                lo,
                hi,
                rect.origin.x,
                rect.origin.y,
                rect.size.x,
                rect.size.y,
            );
        }
        fn end_shader_texture(&mut self, sdf: bool, sdf_scale: f32) {
            self.ck.end_shader_texture(sdf, sdf_scale);
        }
        fn release_shader_texture(&mut self, id: u64) {
            let (lo, hi) = super::shader_textures::halves(id);
            self.ck.release_shader_texture(lo, hi);
        }
        fn shader_texture_ready(&self, id: u64) -> bool {
            let (lo, hi) = super::shader_textures::halves(id);
            self.ck.has_shader_texture(lo, hi)
        }
    };
}
pub(super) use shader_texture_items;
