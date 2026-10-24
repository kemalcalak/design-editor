//! CanvasKit text measures and paints the same kerned advances (#197).
//!
//! Native shapes text word by word through `jian_skia::FontResolver`; the
//! CanvasKit bridge mirrors it in `op_ck_bridge.js`. These source checks
//! keep both halves of the bridge (measure and draw) on the shaped path, so
//! wrapping and carets never run on advances the painter does not use.

fn bridge_source() -> String {
    std::fs::read_to_string(format!(
        "{}/src/op_ck_bridge.js",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("CanvasKit bridge source is readable")
}

#[test]
fn registered_faces_measure_and_paint_through_the_word_shaper() {
    let source = bridge_source();
    for marker in [
        "const shapeWord = (alias, sz, token) =>",
        "const kernedWidth = (tf, f, sz, s) =>",
        "const drawKerned = (tf, f, sz, italic, s, x, y, p) =>",
        // Family-aware draw and measure.
        "cx += drawKerned(familyEntry.tf, f, sz, italic, seg.text, cx, y, p);",
        "w += kernedWidth(familyEntry.tf, f, sz, seg.text);",
        // Script-segmented draw and measure.
        "cx += drawKerned(tf, f, sz, skewed, seg.text, cx, y, p);",
        "w += kernedWidth(tf, f, sz, seg.text);",
        // Every registry hands its bytes to the shaper.
        "registerShapeFace(tf, bytes);",
        // A registry change drops shaped words and blobs.
        "clearShapedCaches();",
    ] {
        assert!(
            source.contains(marker),
            "CanvasKit kerned text must keep `{marker}`"
        );
    }
    assert_eq!(
        source.matches("registerShapeFace(tf, bytes);").count(),
        3,
        "system, imported and bundled faces all register for shaping"
    );
}

#[test]
fn only_the_unshaped_fallback_draws_plain_text() {
    let source = bridge_source();
    // `drawKerned` keeps one plain `drawText` for faces without bytes (the
    // CanvasKit default); no other text path may bypass it.
    assert_eq!(
        source.matches("canvas.drawText(").count(),
        1,
        "segment paint must go through drawKerned"
    );
}
