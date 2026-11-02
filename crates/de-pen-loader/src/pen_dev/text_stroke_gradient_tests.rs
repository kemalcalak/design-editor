//! #282: outlined text and the gradient centre come through a pen.dev
//! import instead of being dropped silently.

use serde_json::{json, Value};

use super::no_assets;
use crate::payload::load_canonical_with_assets;

/// Load a pen.dev 2.21 document; the canonical JSON and the report.
fn import(children: Value) -> (Value, super::PenDevReport) {
    let src = json!({ "version": "2.21", "children": children }).to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("pen.dev document loads");
    let report = loaded.compatibility.pen_dev.unwrap_or_default();
    (serde_json::to_value(&loaded.loaded.value).unwrap(), report)
}

fn gradient(kind: &str, extra: Value) -> Value {
    let mut fill = json!({ "type": "gradient", "gradientType": kind,
        "colors": [{ "color": "#FF0000", "position": 0 }, { "color": "#0000FF", "position": 1 }] });
    fill.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    json!({ "type": "frame", "id": kind, "width": 300, "height": 120, "fill": [fill] })
}

#[test]
fn outlined_text_keeps_its_stroke() {
    let (doc, report) = import(json!([{
        "type": "text", "id": "t", "content": "Outlined", "fill": "#111111",
        "stroke": "#FF00FF", "strokeWidth": 1, "strokeAlignment": "outer"
    }]));
    let stroke = &doc["children"][0]["stroke"];
    assert_eq!(stroke["thickness"], 1.0);
    assert_eq!(stroke["align"], "outside");
    assert_eq!(stroke["fill"][0]["color"], "#FF00FF");
    assert!(report.unsupported.is_empty(), "{:?}", report.unsupported);
}

#[test]
fn radial_and_angular_centres_become_cx_cy() {
    let center = json!({ "center": { "x": 0.3, "y": 0.7 } });
    let (doc, report) = import(json!([
        gradient("radial", center.clone()),
        gradient("angular", center),
    ]));
    for node in doc["children"].as_array().unwrap() {
        let fill = &node["fill"][0];
        assert_eq!(fill["cx"].as_f64().map(|v| (v * 10.0).round()), Some(3.0));
        assert_eq!(fill["cy"].as_f64().map(|v| (v * 10.0).round()), Some(7.0));
    }
    assert!(report.unsupported.is_empty(), "{:?}", report.unsupported);
}

#[test]
fn a_centred_default_reports_nothing() {
    let (_, report) = import(json!([
        gradient(
            "linear",
            json!({ "center": { "x": 0.5, "y": 0.5 }, "size": { "height": 1 } })
        ),
        gradient("radial", json!({ "size": { "width": 1, "height": 1 } })),
    ]));
    assert!(report.unsupported.is_empty(), "{:?}", report.unsupported);
}

#[test]
fn geometry_the_canonical_gradient_lacks_is_reported() {
    let (_, report) = import(json!([
        gradient("linear", json!({ "center": { "x": 0.2, "y": 0.5 } })),
        gradient("linear", json!({ "size": { "width": 3, "height": 0.5 } })),
        gradient("radial", json!({ "size": { "width": 0.4, "height": 1 } })),
    ]));
    assert_eq!(report.unsupported.get("linear gradient center"), Some(&1));
    assert_eq!(report.unsupported.get("gradient size"), Some(&2));
}

#[test]
fn a_linear_width_alone_is_ignored_like_pen_dev() {
    let (_, report) = import(json!([gradient(
        "linear",
        json!({ "size": { "width": 3 } })
    )]));
    assert!(report.unsupported.is_empty(), "{:?}", report.unsupported);
}
