//! Outlined text in the CanvasKit bridge (#282).
//!
//! `strokeText` reuses `drawText`'s glyph placement with a stroke paint, so
//! an outline lands exactly on the glyphs the fill draws. These source
//! checks keep the outline routed through the same shaped draws, browser
//! rasters drawn as outlines, and emoji bitmaps out of it.

fn source(path: &str) -> String {
    std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("{path} is readable: {e}"))
}

#[test]
fn stroke_text_draws_the_fill_glyphs_with_the_outline_paint() {
    let bridge = source("src/op_ck_bridge.js");
    for marker in [
        "strokeText(t, family, x, y, sz, weight, italic, r, g, b, a, w) {",
        "return { paint, width: w, style: strokeStyle };",
        "textOutline = glyphOutlinePaint(r, g, b, a, w);",
        "this.drawText(t, family, x, y, sz, weight, italic, r, g, b, a);",
        // Both glyph paths pick the outline over the fill.
        "const p = textPaint(textOutline ? null : fillPaint(r, g, b, a), sz, weight);",
        "const p = textPaint(fill, sz, weight);",
        // Only an emoji bitmap merely advances the pen while outlining.
        "const browserText = (seg, cx) => textOutline && seg.emoji",
        // Browser-rastered text is rastered as outlines, keyed apart.
        "browserTextCtx.strokeText(t, left, baseline);",
        "const key = outline ? baseKey + '\\n' + outlineKey : baseKey;",
        // Complex scripts outline through the shaped path.
        "strokeShapedText(t, family, x, y, sz, weight, italic, r, g, b, a, w) {",
    ] {
        assert!(bridge.contains(marker), "the bridge must keep `{marker}`");
    }
    // The outline is cleared even when a draw throws.
    let stroke = bridge
        .split("strokeText(t, family")
        .nth(1)
        .expect("strokeText is defined");
    let body = &stroke[..stroke
        .find("drawShapedText(")
        .expect("drawShapedText follows")];
    assert!(body.contains("} finally {\n        textOutline = null;"));
}

#[test]
fn outlines_stroke_merged_glyph_paths_not_the_blob() {
    let bridge = source("src/op_ck_bridge.js");
    for marker in [
        // Overlapping contours of a variable-font glyph are merged once.
        "let path = CK.Path.MakeFromText ? CK.Path.MakeFromText(ch, 0, 0, f) : null;",
        "if (path && path.simplify && !path.simplify()) {",
        // Placed at the kerned positions the fill uses.
        "out.addPath(glyph, [1, 0, advance + xs[i], 0, 1, ys[i], 0, 0, 1]);",
        "if (entry.outline === undefined) entry.outline = kernedOutline(alias, f, sz, italic, s);",
        // A ligature falls back to stroking the blob.
        "if (word.glyphs.length !== chars.length) {",
        // Cached paths are freed with the blobs.
        "if (entry.outline && entry.outline.delete) entry.outline.delete();",
    ] {
        assert!(bridge.contains(marker), "the bridge must keep `{marker}`");
    }
}

#[test]
fn the_rust_backend_calls_the_stroke_binding() {
    let bindings = source("src/canvaskit/bindings.rs");
    assert!(bindings.contains("js_name = strokeText"));
    assert!(bindings.contains("js_name = strokeShapedText"));
    let text = source("src/canvaskit/text.rs");
    assert!(text.contains("ck.stroke_text("));
    assert!(text.contains("Some((color, width)) if complex => ck.stroke_shaped_text("));
    let backend = source("src/canvaskit/backend.rs");
    assert!(backend.contains("fn stroke_text(&mut self"));
    assert!(backend
        .contains("super::text::paint_runs(&self.ck, layout, origin, Some((color, width)));"));
}
