//! Live browser frames (#324) into the CanvasKit image cache: the bitmaps
//! `widget_host::browser_live_stream` decoded go in under each node's
//! stable texture id, replacing the last frame in place, and come out once
//! the node shows no live picture.

use wasm_bindgen::prelude::*;

use super::bindings::OpCk;
use crate::widget_host::browser_live_stream;

#[wasm_bindgen(module = "/src/op_ck_bridge.js")]
extern "C" {
    /// Install a decoded `ImageBitmap` as the live image `id`; the cache
    /// owns (and closes) it. False when CanvasKit could not take it.
    #[wasm_bindgen(method, js_name = installLiveImage)]
    fn install_live_image(
        this: &OpCk,
        image_id_lo: u32,
        image_id_hi: u32,
        bitmap: &JsValue,
    ) -> bool;
    #[wasm_bindgen(method, js_name = dropLiveImage)]
    fn drop_live_image(this: &OpCk, image_id_lo: u32, image_id_hi: u32);
}

/// Before a paint: install the frames decoded since the last one.
pub(super) fn install_landed(ck: &OpCk) {
    browser_live_stream::install_landed(|texture, bitmap| {
        ck.install_live_image(texture as u32, (texture >> 32) as u32, bitmap)
    });
}

/// After the stream pump: free the live images no node shows any more.
pub(super) fn release(ck: &OpCk) {
    for texture in browser_live_stream::textures_to_release() {
        ck.drop_live_image(texture as u32, (texture >> 32) as u32);
    }
}
