use serde_json::{json, Value};

use super::{convert, no_assets, sniff_pen_dev_version};
use crate::payload::load_canonical_with_assets;

fn pen(children: Value) -> Value {
    json!({ "version": "2.21", "children": children })
}

fn converted(children: Value) -> (Value, super::PenDevReport) {
    let mut doc = pen(children);
    let report = convert(&mut doc, &no_assets);
    (doc, report)
}

fn png_reader(relative: &str) -> Option<Vec<u8>> {
    (relative == "images/a.png").then(|| vec![0x89, b'P', b'N', b'G'])
}

#[test]
fn version_sniff_takes_pen_dev_2_9_and_later_only() {
    assert_eq!(
        sniff_pen_dev_version("{\n  \"version\": \"2.21\",\n  \"children\": []}"),
        Some("2.21".to_owned())
    );
    assert_eq!(
        sniff_pen_dev_version(r#"{"version":"2.9"}"#),
        Some("2.9".to_owned())
    );
    // The TS-era 2.8 documents keep their historical load path.
    assert_eq!(sniff_pen_dev_version(r#"{"version":"2.8"}"#), None);
    assert_eq!(sniff_pen_dev_version(r#"{"version":"1.0"}"#), None);
    assert_eq!(sniff_pen_dev_version(r#"{"children":[]}"#), None);
}

#[test]
fn format_version_marks_a_canonical_document() {
    assert_eq!(
        sniff_pen_dev_version(r#"{"version":"2.21","formatVersion":"1.0","children":[]}"#),
        None
    );
}

#[test]
fn only_a_sniffed_source_is_converted_on_load() {
    // A canonical document whose `version` reads 2.21 but carries
    // `formatVersion` loads as canonical, without the pen.dev defaults.
    let src = r#"{"version":"2.21","formatVersion":"1.0","children":[{"type":"frame","id":"f"}]}"#;
    let loaded = load_canonical_with_assets(src, &no_assets);
    if let Ok(loaded) = loaded {
        assert!(loaded.compatibility.pen_dev.is_none());
    }
}

#[test]
fn legacy_2_8_documents_are_left_untouched() {
    let mut doc = json!({ "version": "2.8", "children": [{ "type": "frame", "id": "f" }] });
    let before = doc.clone();
    let report = convert(&mut doc, &no_assets);
    assert_eq!(doc, before);
    assert_eq!(report, super::PenDevReport::default());
}

#[test]
fn top_level_keys_become_canonical() {
    let mut doc = json!({ "version": "2.21", "fileToken": "t", "imports": { "kit": "kit.pen" }, "children": [] });
    let report = convert(&mut doc, &no_assets);
    assert_eq!(doc, json!({ "version": "1.0", "children": [] }));
    assert_eq!(report.source_version, "2.21");
    assert_eq!(report.unsupported.get("imported .pen libraries"), Some(&1));
}

#[test]
fn frames_default_to_a_hugging_horizontal_layout() {
    let (doc, _) = converted(json!([
        { "type": "frame", "id": "row" },
        { "type": "frame", "id": "free", "layout": "none" },
        { "type": "frame", "id": "fixed", "layout": "vertical", "width": 200, "height": "fit_content(40)" }
    ]));
    let kids = &doc["children"];
    assert_eq!(kids[0]["layout"], "horizontal");
    assert_eq!(kids[0]["width"], "fit_content");
    assert_eq!(kids[0]["height"], "fit_content");
    assert!(kids[1].get("width").is_none(), "free stack keeps no size");
    assert_eq!(kids[2]["width"], 200);
    assert_eq!(kids[2]["height"], "fit_content", "fallback size is dropped");
}

#[test]
fn absolute_position_and_disabled_nodes_map_to_canonical_fields() {
    let (doc, _) = converted(json!([{ "type": "frame", "id": "f", "children": [
        { "type": "ellipse", "id": "dot", "layoutPosition": "absolute", "x": 4 },
        { "type": "text", "id": "t", "content": "hi", "enabled": false, "placeholder": true }
    ]}]));
    // The absolute dot pen.dev paints first (behind) ends up last.
    let kids = &doc["children"][0]["children"];
    assert_eq!(kids[1]["role"], "overlay");
    assert!(kids[1].get("layoutPosition").is_none());
    assert_eq!(kids[0]["visible"], false);
    assert!(kids[0].get("enabled").is_none());
    assert!(kids[0].get("placeholder").is_none());
}

#[test]
fn a_frame_keeps_its_placeholder_flag() {
    let (doc, _) = converted(json!([{ "type": "frame", "id": "f", "placeholder": true }]));
    assert_eq!(doc["children"][0]["placeholder"], true);
}

#[test]
fn unsupported_nodes_are_removed_and_reported() {
    let (doc, report) = converted(json!([
        { "type": "note", "id": "n", "content": "todo" },
        { "type": "frame", "id": "f", "children": [
            { "type": "browser", "id": "b", "url": "https://example.com" },
            { "type": "script", "id": "s" },
            { "type": "text", "id": "t", "content": "kept" }
        ]}
    ]));
    let roots = doc["children"].as_array().unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0]["children"].as_array().unwrap().len(), 1);
    assert_eq!(report.unsupported.get("note"), Some(&1));
    assert_eq!(report.unsupported.get("browser node"), Some(&1));
    assert_eq!(report.unsupported.get("script node"), Some(&1));
}

#[test]
fn mixed_fill_lists_resolve_images_and_drop_unreadable_shaders() {
    let mut doc = pen(json!([{ "type": "frame", "id": "f", "fill": [
        "#141519",
        { "type": "image", "enabled": true, "url": "images/a.png", "mode": "contain" },
        { "type": "image", "url": "images/gone.png" },
        { "type": "image", "url": "../outside.png" },
        { "type": "shader", "url": "flow.glsl" },
        { "type": "color", "color": "#fff", "enabled": false }
    ]}]));
    let report = convert(&mut doc, &png_reader);
    let fills = doc["children"][0]["fill"].as_array().unwrap();
    assert_eq!(fills.len(), 2);
    // pen.dev paints the last fill on top; the canonical stack is top-first.
    assert_eq!(fills[0]["mode"], "fit");
    assert_eq!(fills[0]["url"], "op-image:pen-dev-0");
    assert!(fills[0].get("enabled").is_none());
    assert_eq!(fills[1], json!({ "type": "solid", "color": "#141519" }));
    assert_eq!(doc["images"]["pen-dev-0"], "data:image/png;base64,iVBORw==");
    assert_eq!(
        report.missing_images,
        vec!["images/gone.png", "../outside.png"]
    );
    // No `flow.glsl` next to the document: dropped and reported.
    assert_eq!(
        report.unsupported.get("shader fill (GLSL file missing)"),
        Some(&1)
    );
}

#[test]
fn stroke_width_and_caps_fold_into_one_stroke() {
    let (doc, _) = converted(json!([
        { "type": "path", "id": "p", "geometry": "M0 0L10 10", "stroke": "$border",
          "strokeWidth": 1.5, "strokeLinecap": "butt", "strokeLinejoin": "round" },
        { "type": "frame", "id": "f", "stroke": ["#E2E3E8"], "strokeWidth": { "bottom": 1 },
          "strokeAlignment": "inner" }
    ]));
    let kids = &doc["children"];
    assert_eq!(
        kids[0]["stroke"],
        json!({ "thickness": 1.5, "fill": [{ "type": "solid", "color": "$border" }],
                "cap": "none", "join": "round" })
    );
    assert!(kids[0].get("strokeWidth").is_none());
    assert_eq!(kids[1]["stroke"]["thickness"], json!({ "bottom": 1 }));
    assert_eq!(kids[1]["stroke"]["align"], "inside");
}

#[test]
fn shadows_and_blurs_become_canonical_effects() {
    let (doc, report) = converted(json!([{ "type": "frame", "id": "f", "effect": [
        { "type": "shadow", "shadowType": "outer", "color": "#3E6DF526",
          "offset": { "x": 0, "y": 8 }, "blur": 40 },
        { "type": "shadow", "shadowType": "inner", "enabled": false },
        { "type": "background_blur", "radius": 12 },
        { "type": "shader", "url": "fx.glsl" }
    ]}]));
    let effects = doc["children"][0]["effects"].as_array().unwrap();
    assert_eq!(
        effects[0],
        json!({ "type": "shadow", "offsetX": 0.0, "offsetY": 8.0, "blur": 40.0,
                "spread": 0, "color": "#3E6DF526" })
    );
    assert_eq!(
        effects[1],
        json!({ "type": "background_blur", "radius": 12.0 })
    );
    assert_eq!(effects.len(), 2);
    assert!(doc["children"][0].get("effect").is_none());
    assert_eq!(
        report.unsupported.get("shader effect (GLSL file missing)"),
        Some(&1)
    );
}

#[test]
fn blend_modes_map_to_snake_case_or_are_reported() {
    let (doc, report) = converted(json!([
        { "type": "rectangle", "id": "a", "blendMode": "softLight" },
        { "type": "rectangle", "id": "b", "blendMode": "linearBurn" },
        { "type": "rectangle", "id": "c", "blendMode": "linearDodge" },
        { "type": "rectangle", "id": "d", "blendMode": "plusDarker" }
    ]));
    assert_eq!(doc["children"][0]["blendMode"], "soft_light");
    assert_eq!(doc["children"][1]["blendMode"], "linear_burn");
    assert_eq!(doc["children"][2]["blendMode"], "linear_dodge");
    assert!(doc["children"][3].get("blendMode").is_none());
    assert_eq!(report.unsupported.get("blend mode"), Some(&1));
}

#[test]
fn a_cache_busting_query_still_reads_the_file() {
    let mut doc = pen(json!([
        { "type": "frame", "id": "a", "fill": { "type": "image", "url": "images/a.png?v=2" } },
        { "type": "frame", "id": "b", "fill": { "type": "image", "url": "images/a.png#top" } }
    ]));
    let report = convert(&mut doc, &png_reader);
    assert!(
        report.missing_images.is_empty(),
        "{:?}",
        report.missing_images
    );
    assert!(doc["children"][0]["fill"][0]["url"]
        .as_str()
        .is_some_and(|url| url.starts_with("op-image:")));
}

#[test]
fn a_bitmap_used_by_many_fills_is_stored_once() {
    let mut doc = pen(json!([
        { "type": "frame", "id": "a", "fill": { "type": "image", "url": "images/a.png" } },
        { "type": "frame", "id": "b", "fill": { "type": "image", "url": "./images/a.png" } },
        { "type": "frame", "id": "c", "fill": { "type": "image", "url": "images/a.png" } }
    ]));
    convert(&mut doc, &png_reader);
    assert_eq!(
        doc["images"].as_object().unwrap().len(),
        2,
        "one per distinct url"
    );
    assert_eq!(
        doc["children"][0]["fill"][0]["url"],
        doc["children"][2]["fill"][0]["url"]
    );
}

#[test]
fn instance_overrides_use_canonical_spellings() {
    let (doc, _) = converted(
        json!([{ "type": "ref", "id": "i", "ref": "btn", "descendants": {
            "label": { "fill": "#FFFFFF" },
            "icon": { "type": "frame", "id": "swap" }
        }}]),
    );
    let overrides = &doc["children"][0]["descendants"];
    assert_eq!(
        overrides["label"]["fill"],
        json!([{ "type": "solid", "color": "#FFFFFF" }])
    );
    assert_eq!(overrides["icon"]["layout"], "horizontal");
}

#[test]
fn instance_overrides_the_canonical_merge_cannot_express_are_reported() {
    let (doc, report) = converted(
        json!([{ "type": "ref", "id": "i", "ref": "btn", "descendants": {
            "swap": { "type": "text", "id": "t", "content": "new" },
            "width-only": { "strokeWidth": 3 },
            "paint-only": { "stroke": "#f00" },
            "both": { "stroke": "#f00", "strokeWidth": 2 },
            "gone": { "type": "note", "id": "n" }
        }}]),
    );
    let overrides = doc["children"][0]["descendants"].as_object().unwrap();
    assert!(
        !overrides.contains_key("gone"),
        "dropped kinds leave overrides too"
    );
    assert!(overrides["width-only"].get("strokeWidth").is_none());
    assert_eq!(overrides["both"]["stroke"]["thickness"], 2);
    // A typed override is a replacement the canonical resolver honours
    // (#166): kept whole, converted as a node, not reported.
    assert_eq!(overrides["swap"]["type"], "text");
    assert_eq!(overrides["swap"]["content"], "new");
    assert!(!report
        .unsupported
        .keys()
        .any(|feature| feature.contains("replacement")));
    assert_eq!(report.unsupported.get("partial stroke override"), Some(&2));
    assert_eq!(report.unsupported.get("note"), Some(&1));
}

#[test]
fn an_absolute_path_inside_auto_layout_still_honours_its_view_box() {
    let (doc, report) = converted(json!([{ "type": "frame", "id": "row", "children": [
        { "type": "path", "id": "p", "layoutPosition": "absolute", "x": 0, "y": 0,
          "width": 50, "height": 50, "viewBox": [0, 0, 100, 100],
          "geometry": "M50 0L100 0L100 100L50 100Z" }
    ]}]));
    let path = &doc["children"][0]["children"][0];
    assert_eq!(path["x"], 25.0);
    assert_eq!(path["width"], 25.0);
    assert!(report.unsupported.is_empty());
}

#[test]
fn path_view_box_offsets_a_free_path_to_its_geometry() {
    // The geometry covers the right half of a 0..100 viewBox drawn at 50×50.
    let (doc, report) = converted(
        json!([{ "type": "frame", "id": "f", "layout": "none", "children": [
            { "type": "path", "id": "p", "x": 10, "y": 10, "width": 50, "height": 50,
              "viewBox": [0, 0, 100, 100], "geometry": "M50 0L100 0L100 100L50 100Z" }
        ]}]),
    );
    let path = &doc["children"][0]["children"][0];
    assert_eq!(path["x"], 35.0);
    assert_eq!(path["y"], 10.0);
    assert_eq!(path["width"], 25.0);
    assert_eq!(path["height"], 50.0);
    assert!(path.get("viewBox").is_none());
    assert!(report.unsupported.is_empty());
}

#[test]
fn a_pen_dev_document_loads_into_the_typed_schema() {
    let src = json!({
        "version": "2.21",
        "fileToken": "6d11",
        "themes": { "mode": ["light", "dark"] },
        "variables": { "bg": { "type": "color", "value": [
            { "value": "#F6F6F8", "theme": { "mode": "light" } },
            { "value": "#141519", "theme": { "mode": "dark" } }
        ]}},
        "children": [{
            "type": "frame", "id": "home", "name": "Home", "x": 0, "y": 0,
            "width": 1440, "height": 900, "clip": true, "theme": { "mode": "dark" },
            "fill": ["$bg", { "type": "image", "enabled": true, "url": "images/a.png", "mode": "cover" }],
            "stroke": "#2B2D34", "strokeWidth": { "right": 1 },
            "effect": { "type": "shadow", "shadowType": "outer", "color": "#00000066",
                        "offset": { "x": 0, "y": 8 }, "blur": 24 },
            "children": [
                { "type": "icon", "id": "i", "icon": "plus", "library": "lucide",
                  "width": 16, "height": 16, "fill": "#FFFFFF", "weight": 700 },
                { "type": "text", "id": "t", "content": "Hi", "fontFamily": "Manrope",
                  "fontWeight": "700", "textGrowth": "fixed-width", "width": "fill_container",
                  "lineHeight": 1.5, "fill": "#FFFFFF" },
                { "type": "note", "id": "n", "content": "remember" }
            ]
        }]
    })
    .to_string();
    let loaded = load_canonical_with_assets(&src, &png_reader).expect("pen.dev document loads");
    let report = loaded
        .compatibility
        .pen_dev
        .clone()
        .expect("converted as pen.dev");
    assert_eq!(report.unsupported.get("note"), Some(&1));
    assert!(
        !loaded.compatibility.needs_schema_upgrade(),
        "never rewritten in place"
    );

    let doc = loaded.loaded.value;
    let jian_ops_schema::node::PenNode::Frame(home) = &doc.children[0] else {
        panic!("root is a frame");
    };
    assert_eq!(
        home.container.layout,
        Some(jian_ops_schema::node::container::LayoutMode::Horizontal)
    );
    assert_eq!(home.container.clip_content, Some(true));
    assert_eq!(home.container.fill.as_ref().map(Vec::len), Some(2));
    assert!(home.container.stroke.is_some());
    assert_eq!(home.container.effects.as_ref().map(Vec::len), Some(1));
    let kids = home.children.as_ref().expect("children");
    assert_eq!(kids.len(), 2, "the note is dropped");
    let jian_ops_schema::node::PenNode::IconFont(icon) = &kids[0] else {
        panic!("the icon converts to icon_font");
    };
    assert_eq!(
        icon.weight,
        Some(jian_ops_schema::node::FontWeight::Number(700)),
        "the icon weight survives the import"
    );
}

/// Converts a real pen.dev file. Not run in CI: set `PEN_DEV_SAMPLE` to a
/// local `.pen` path (e.g. one under `~/.pencil/documents/`) and run with
/// `--ignored`.
#[test]
#[ignore = "needs a local pen.dev document in PEN_DEV_SAMPLE"]
fn a_local_pen_dev_document_loads() {
    let Ok(path) = std::env::var("PEN_DEV_SAMPLE") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let src = std::fs::read_to_string(&path).expect("read sample");
    let folder = path.parent().expect("sample folder").to_path_buf();
    let reader = move |relative: &str| std::fs::read(folder.join(relative)).ok();
    let loaded = load_canonical_with_assets(&src, &reader).expect("sample loads");
    let report = loaded.compatibility.pen_dev.expect("converted as pen.dev");
    eprintln!(
        "{} roots; losses: {}",
        loaded.loaded.value.children.len(),
        report.summary()
    );
    assert!(!loaded.loaded.value.children.is_empty());
}

#[test]
fn the_short_summary_counts_missing_images() {
    let mut report = super::PenDevReport::default();
    report.note("shader fill");
    report.note_missing_image("images/a.png");
    report.note_missing_image("images/b.png");
    assert_eq!(
        report.summary(),
        "shader fill ×1; missing images: images/a.png, images/b.png"
    );
    assert_eq!(report.short_summary(), "shader fill ×1; 2 missing images");
}

#[test]
fn gradient_rotation_turns_clockwise_like_css() {
    let gradient = |rotation: f64| {
        json!({ "type": "gradient", "gradientType": "linear", "rotation": rotation,
                "colors": [{ "color": "#000000", "position": 0 }, { "color": "#FFFFFF", "position": 1 }] })
    };
    let src = json!({ "version": "2.21", "children": [
        { "type": "rectangle", "id": "down", "width": 10, "height": 10, "fill": gradient(180.0) },
        { "type": "rectangle", "id": "right", "width": 10, "height": 10, "fill": gradient(270.0) },
        { "type": "rectangle", "id": "left", "width": 10, "height": 10, "fill": gradient(90.0) },
        { "type": "rectangle", "id": "up", "width": 10, "height": 10, "fill": gradient(0.0) }
    ]})
    .to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("loads");
    let doc = serde_json::to_value(&loaded.loaded.value).unwrap();
    let angles: Vec<f64> = doc["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["fill"][0]["angle"].as_f64().unwrap())
        .collect();
    // pen.dev: 0° up, counter-clockwise. Canonical: CSS, clockwise.
    assert_eq!(angles, vec![180.0, 90.0, 270.0, 0.0]);

    let mut doc = pen(json!([{ "type": "rectangle", "id": "sweep", "fill": {
        "type": "gradient", "gradientType": "angular", "rotation": 90,
        "colors": [{ "color": "#000000", "position": 0 }, { "color": "#FFFFFF", "position": 1 }]
    }}]));
    convert(&mut doc, &no_assets);
    assert_eq!(doc["children"][0]["fill"][0]["rotation"], 270.0);
}

/// A PNG header of `width` × `height` (enough for the size reader).
fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut png = vec![0; 24];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[8..12].copy_from_slice(&13_u32.to_be_bytes());
    png[12..16].copy_from_slice(b"IHDR");
    png[16..20].copy_from_slice(&width.to_be_bytes());
    png[20..24].copy_from_slice(&height.to_be_bytes());
    png
}

#[test]
fn an_image_crop_becomes_a_canonical_crop_transform() {
    let reader = |relative: &str| (relative == "images/wide.png").then(|| png_header(200, 100));
    let fill = |mode: &str| {
        json!({ "type": "image", "url": "images/wide.png", "mode": mode,
                "transform": [2, 0, 0, 1, -1, 0] })
    };
    let src = json!({ "version": "2.21", "children": [
        { "type": "rectangle", "id": "cover", "width": 200, "height": 100, "fill": fill("cover") },
        { "type": "rectangle", "id": "hug", "width": "fill_container", "height": 100, "fill": fill("cover") },
        { "type": "rectangle", "id": "broken", "width": 10, "height": 10, "fill": {
            "type": "image", "url": "images/wide.png", "transform": [0, 0, 0, 0, 0, 0] } }
    ]})
    .to_string();
    let loaded = load_canonical_with_assets(&src, &reader).expect("loads");
    let report = loaded.compatibility.pen_dev.expect("converted");
    let doc = serde_json::to_value(&loaded.loaded.value).unwrap();
    let cover = &doc["children"][0]["fill"][0];
    assert_eq!(cover["mode"], "crop");
    // The right half (a 100×100 crop) covering a 200×100 box: its middle
    // half vertically.
    let m = &cover["transform"];
    assert_eq!(
        (m["m00"].as_f64(), m["m02"].as_f64()),
        (Some(0.5), Some(0.5))
    );
    assert_eq!(
        (m["m11"].as_f64(), m["m12"].as_f64()),
        (Some(0.5), Some(0.25))
    );
    assert_eq!(
        report.unsupported.get("image crop (fitted as stretch)"),
        Some(&1)
    );
    assert_eq!(
        report.unsupported.get("image crop (invalid transform)"),
        Some(&1)
    );
    assert!(doc["children"][2]["fill"][0].get("transform").is_none());
    assert!(!report.unsupported.contains_key("image crop"));
}

#[test]
fn a_text_link_opens_its_url_on_tap() {
    let src = json!({ "version": "2.21", "children": [
        { "type": "text", "id": "link", "content": "Docs", "href": "https://example.com/it's" },
        { "type": "text", "id": "own", "content": "Own", "href": "https://example.com",
          "events": { "onTap": [{ "navigate": { "to": "home" } }] } }
    ]})
    .to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("loads");
    let report = loaded.compatibility.pen_dev.expect("converted");
    assert!(!report.has_losses(), "{}", report.summary());
    let doc = serde_json::to_value(&loaded.loaded.value).unwrap();
    let link = &doc["children"][0];
    assert!(link.get("href").is_none());
    assert_eq!(
        link["events"]["onTap"],
        json!([{ "open_url": { "url": "'https://example.com/it\\'s'" } }])
    );
    assert_eq!(link["semantics"]["role"], "link");
    // An authored tap handler is kept.
    assert_eq!(
        doc["children"][1]["events"]["onTap"][0]["navigate"]["to"],
        "home"
    );
}

#[test]
fn a_shadow_blend_mode_reaches_the_scene() {
    let src = json!({ "version": "2.21", "children": [
        { "type": "rectangle", "id": "card", "width": 10, "height": 10, "fill": "#FFFFFF",
          "effect": { "type": "shadow", "color": "#0000FF", "offset": { "x": 4, "y": 4 },
                      "blur": 0, "blendMode": "multiply" } }
    ]})
    .to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("loads");
    let doc = serde_json::to_value(&loaded.loaded.value).unwrap();
    assert_eq!(doc["children"][0]["effects"][0]["blendMode"], "multiply");
    let shadows = crate::effects::shadows_from_canonical(&loaded.loaded.value.children[0]);
    let scene = crate::effects::effects_from_payload(shadows);
    let jian_scene::layout_scene::Effect::DropShadow(shadow) = scene[0] else {
        panic!("a drop shadow");
    };
    assert_eq!(
        shadow.blend_mode,
        de_editor_core::render_backend::ImageBlendMode::Multiply
    );
    let back = crate::effects::effects_to_payload(&scene);
    assert_eq!(
        back[0].blend_mode,
        Some(jian_ops_schema::style::BlendMode::Multiply),
        "round trip"
    );
}
