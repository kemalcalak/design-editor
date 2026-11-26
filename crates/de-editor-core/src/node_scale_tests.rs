use super::*;
use serde_json::json;

fn scale_with(node: Value, factor: f64, mode: VariableMode) -> (Value, ScaleReport) {
    let resolve = |token: &str| (token == "$space").then_some(8.0);
    let scaler = PropScaler {
        factor,
        mode,
        resolve: &resolve,
    };
    let Value::Object(mut map) = node else {
        panic!("object");
    };
    let mut report = ScaleReport::default();
    scaler.scale_own(&mut map, "n", &mut report);
    (Value::Object(map), report)
}

fn scale2(node: Value) -> Value {
    scale_with(node, 2.0, VariableMode::KeepBound).0
}

#[test]
fn sizes_and_limits_scale_keywords_stay() {
    let out = scale2(json!({
        "type": "frame", "id": "n", "width": 120, "height": "fit_content",
        "minWidth": 40, "maxHeight": 300.5
    }));
    assert_eq!(out["width"], json!(240));
    assert_eq!(out["height"], json!("fit_content"));
    assert_eq!(out["minWidth"], json!(80));
    assert_eq!(out["maxHeight"], json!(601));
}

#[test]
fn auto_layout_gap_and_padding_scale() {
    let out = scale2(json!({
        "type": "frame", "id": "n", "gap": 12, "rowGap": 4, "columnGap": 6,
        "padding": [8, 16, 8, 4]
    }));
    assert_eq!(out["gap"], json!(24));
    assert_eq!(out["rowGap"], json!(8));
    assert_eq!(out["columnGap"], json!(12));
    assert_eq!(out["padding"], json!([16, 32, 16, 8]));
    assert_eq!(scale2(json!({"padding": 10}))["padding"], json!(20));
}

#[test]
fn corner_radii_scale_uniform_and_per_corner() {
    assert_eq!(
        scale2(json!({"cornerRadius": 6}))["cornerRadius"],
        json!(12)
    );
    assert_eq!(
        scale2(json!({"cornerRadius": [1, 2, 3, 4]}))["cornerRadius"],
        json!([2, 4, 6, 8])
    );
}

#[test]
fn stroke_width_sides_and_dashes_scale() {
    let out = scale2(json!({"stroke": {
        "thickness": 1.5, "dashPattern": [4, 2], "dashOffset": 1,
        "fill": [{"type": "solid", "color": "#000000"}]
    }}));
    assert_eq!(out["stroke"]["thickness"], json!(3));
    assert_eq!(out["stroke"]["dashPattern"], json!([8, 4]));
    assert_eq!(out["stroke"]["dashOffset"], json!(2));
    let sides = scale2(json!({"stroke": {"thickness": {"top": 1, "left": 2}}}));
    assert_eq!(sides["stroke"]["thickness"], json!({"top": 2, "left": 4}));
    let per = scale2(json!({"stroke": {"thickness": [1, 0, 1, 0]}}));
    assert_eq!(per["stroke"]["thickness"], json!([2, 0, 2, 0]));
}

#[test]
fn effects_scale_offsets_blur_and_spread() {
    let out = scale2(json!({"effects": [
        {"type": "shadow", "offsetX": 0, "offsetY": 4, "blur": 12, "spread": -2, "color": "#0003"},
        {"type": "blur", "radius": 5},
        {"type": "background_blur", "radius": 10},
        {"type": "shader", "sksl": "x"}
    ]}));
    let fx = &out["effects"];
    assert_eq!(fx[0]["offsetY"], json!(8));
    assert_eq!(fx[0]["blur"], json!(24));
    assert_eq!(fx[0]["spread"], json!(-4));
    assert_eq!(fx[1]["radius"], json!(10));
    assert_eq!(fx[2]["radius"], json!(20));
    assert_eq!(fx[3], json!({"type": "shader", "sksl": "x"}));
}

#[test]
fn text_metrics_scale_and_a_line_height_multiplier_stays() {
    let out = scale2(json!({
        "type": "text", "fontSize": 14, "letterSpacing": -0.5, "lineHeight": 1.4,
        "content": [{"text": "a", "fontSize": 20}, {"text": "b"}]
    }));
    assert_eq!(out["fontSize"], json!(28));
    assert_eq!(out["letterSpacing"], json!(-1));
    assert_eq!(out["lineHeight"], json!(1.4));
    assert_eq!(out["content"][0]["fontSize"], json!(40));
    assert!(out["content"][1].get("fontSize").is_none());
    let px = scale2(json!({"type": "text", "lineHeight": 24}));
    assert_eq!(px["lineHeight"], json!(48));
}

#[test]
fn path_geometry_and_line_end_scale() {
    let out = scale2(json!({
        "type": "path",
        "anchors": [{"x": 1, "y": 2, "handleOut": {"x": 3, "y": 4}}],
        "d": "M0 0 L10 5 A2 3 0 0 1 4 4 Z"
    }));
    assert_eq!(out["anchors"][0]["x"], json!(2));
    assert_eq!(out["anchors"][0]["handleOut"], json!({"x": 6, "y": 8}));
    assert_eq!(out["d"], json!("M 0 0 L 20 10 A 4 6 0 0 1 8 8 Z"));
    let line = scale2(json!({"type": "line", "x2": 30, "y2": -4}));
    assert_eq!(
        (line["x2"].clone(), line["y2"].clone()),
        (json!(60), json!(-8))
    );
}

#[test]
fn fractions_and_ratios_stay() {
    let node = json!({
        "type": "polygon", "innerRadius": 0.5, "opacity": 0.4, "rotation": 30,
        "fill": [
            {"type": "radial_gradient", "cx": 0.5, "cy": 0.5, "radius": 0.5, "stops": []},
            {"type": "image", "url": "a.png", "tileScale": 0.5,
             "originalSize": {"width": 100, "height": 50},
             "transform": {"m00": 1, "m01": 0, "m02": 0.1, "m10": 0, "m11": 1, "m12": 0}}
        ]
    });
    let out = scale2(node.clone());
    assert_eq!(out["innerRadius"], node["innerRadius"]);
    assert_eq!(out["opacity"], node["opacity"]);
    assert_eq!(out["rotation"], node["rotation"]);
    assert_eq!(out["fill"][0], node["fill"][0]);
    assert_eq!(
        out["fill"][1]["originalSize"],
        node["fill"][1]["originalSize"]
    );
    assert_eq!(out["fill"][1]["transform"], node["fill"][1]["transform"]);
    assert_eq!(out["fill"][1]["tileScale"], json!(1));
}

#[test]
fn grid_tracks_scale_px_and_keep_counts_and_fr() {
    let out = scale2(json!({"columns": [120, "1fr", "minmax(80px, 1fr)"], "rows": 3}));
    assert_eq!(out["columns"], json!([240, "1fr", "minmax(160px, 1fr)"]));
    assert_eq!(out["rows"], json!(3));
}

#[test]
fn widget_state_styles_scale() {
    let out = scale2(json!({"states": {"hover": {
        "stroke": {"thickness": 1}, "effects": [{"type": "blur", "radius": 2}]
    }}}));
    assert_eq!(out["states"]["hover"]["stroke"]["thickness"], json!(2));
    assert_eq!(out["states"]["hover"]["effects"][0]["radius"], json!(4));
}

#[test]
fn variables_stay_bound_by_default_and_are_listed() {
    let (out, report) = scale_with(
        json!({"gap": "$space", "padding": ["$space", 4]}),
        2.0,
        VariableMode::KeepBound,
    );
    assert_eq!(out["gap"], json!("$space"));
    assert_eq!(out["padding"], json!(["$space", 8]));
    let fields: Vec<_> = report.bound.iter().map(|n| n.field.as_str()).collect();
    assert_eq!(fields, ["gap", "padding/0"]);
}

#[test]
fn detach_scales_the_resolved_value() {
    let (out, report) = scale_with(
        json!({"gap": "$space", "rowGap": "$missing"}),
        2.0,
        VariableMode::Detach,
    );
    assert_eq!(out["gap"], json!(16));
    assert_eq!(out["rowGap"], json!("$missing"), "an unresolved ref stays");
    assert_eq!(report.detached.len(), 1);
    assert_eq!(report.bound.len(), 1);
}

#[test]
fn typed_bindings_keep_or_detach() {
    let node = json!({
        "type": "text", "fontSize": 16,
        "variableBindings": {"fontSize": {"variable": "body", "value": 16}}
    });
    let (kept, report) = scale_with(node.clone(), 2.0, VariableMode::KeepBound);
    assert_eq!(kept["fontSize"], json!(16));
    assert!(kept.get("variableBindings").is_some());
    assert_eq!(report.bound[0].field, "fontSize");
    let (detached, report) = scale_with(node, 2.0, VariableMode::Detach);
    assert_eq!(detached["fontSize"], json!(32));
    assert!(detached.get("variableBindings").is_none());
    assert_eq!(report.detached[0].field, "fontSize");
}

#[test]
fn variable_mode_parses_wire_spellings() {
    assert_eq!(VariableMode::parse("keep"), Some(VariableMode::KeepBound));
    assert_eq!(VariableMode::parse("Detach"), Some(VariableMode::Detach));
    assert_eq!(VariableMode::parse("drop"), None);
}
