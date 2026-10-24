//! Variable references in a pen.dev document (#145), against a reduced copy
//! of a real file's shapes: themed number and string variables used from
//! typed fields, boards with their own `theme`, an instance patch.

use serde_json::{json, Value};

use super::{convert, no_assets};
use crate::payload::load_canonical_with_assets;

const FIXTURE: &str = include_str!("../../tests/fixtures/pen-dev-variables.pen");

fn converted() -> (Value, super::PenDevReport) {
    let mut doc: Value = serde_json::from_str(FIXTURE).expect("fixture is JSON");
    let report = convert(&mut doc, &no_assets);
    (doc, report)
}

fn child<'a>(node: &'a Value, id: &str) -> &'a Value {
    node["children"]
        .as_array()
        .and_then(|kids| kids.iter().find(|kid| kid["id"] == id))
        .unwrap_or_else(|| panic!("no child {id}"))
}

#[test]
fn the_fixture_loads_instead_of_failing_on_a_variable_font_size() {
    let loaded = load_canonical_with_assets(FIXTURE, &no_assets).expect("fixture loads");
    let report = loaded.compatibility.pen_dev.expect("converted as pen.dev");
    assert!(report.has_losses(), "fixed values are reported");
    assert_eq!(loaded.loaded.value.children.len(), 3);
}

#[test]
fn typed_fields_take_the_value_for_the_boards_theme() {
    let (doc, _) = converted();
    let desktop = child(&doc, "desktop");
    let mobile = child(&doc, "mobile");
    assert_eq!(child(desktop, "desktop-data")["fontSize"], json!(13));
    assert_eq!(child(mobile, "mobile-data")["fontSize"], json!(12));
    assert_eq!(child(mobile, "mobile-data")["fontFamily"], json!("Inter"));
    assert_eq!(desktop["padding"], json!([48, 80, 72, 80]));
    assert_eq!(mobile["padding"], json!([48, 20, 72, 20]));
    // The icon name follows the mode axis.
    assert_eq!(child(desktop, "desktop-toggle")["icon"], json!("moon"));
    assert_eq!(child(mobile, "mobile-toggle")["icon"], json!("sun"));
    // A component outside any board uses the document's default theme.
    let chip = child(&doc, "chip");
    assert_eq!(chip["cornerRadius"], json!(999));
    assert_eq!(chip["padding"], json!([8, 80]));
}

#[test]
fn references_the_canonical_resolver_reads_stay_bound() {
    let (doc, _) = converted();
    let mobile = child(&doc, "mobile");
    assert_eq!(mobile["gap"], json!("$space-m"));
    assert_eq!(
        child(mobile, "mobile-data")["fill"],
        json!([{ "type": "solid", "color": "$ink" }])
    );
}

#[test]
fn an_instance_patch_resolves_with_the_instances_theme() {
    let (doc, _) = converted();
    let instance = child(child(&doc, "mobile"), "mobile-chip");
    assert_eq!(instance["descendants"]["chip-label"]["fontSize"], json!(12));
    // The chip's own padding reads a device variable: the mobile copy gets
    // the mobile value.
    assert_eq!(instance["descendants"]["chip"]["padding"], json!([8, 20]));
}

#[test]
fn an_unknown_variable_in_a_number_field_is_dropped_and_reported() {
    let (doc, report) = converted();
    let text = child(child(&doc, "desktop"), "desktop-data");
    assert!(text.get("letterSpacing").is_none());
    assert_eq!(report.unsupported.get("unresolved variable"), Some(&1));
}

#[test]
fn typed_field_references_stay_bound_and_are_not_reported_fixed() {
    let (doc, report) = converted();
    for label in [
        "font size variable (fixed value)",
        "font family variable (fixed value)",
        "spacing variable (fixed value)",
        "corner radius variable (fixed value)",
        "icon variable (fixed value)",
        "paint variable (fixed value)",
    ] {
        assert!(
            !report.unsupported.contains_key(label),
            "{}",
            report.summary()
        );
    }
    assert_eq!(
        report.unsupported.get("shader fill (GLSL file missing)"),
        Some(&1)
    );
    let chip = child(&doc, "chip");
    assert_eq!(
        chip["variableBindings"]["cornerRadius"]["value"],
        json!(999)
    );
}

#[test]
fn a_reference_that_cannot_fit_its_field_never_fails_the_load() {
    let src = json!({
        "version": "2.21",
        "variables": {
            "ink": { "type": "color", "value": "#111111" },
            "grow": { "type": "string", "value": "fill_container" },
            "count": { "type": "number", "value": 3 },
            "loop": { "type": "string", "value": "$loop" }
        },
        "children": [{
            "type": "frame", "id": "f", "layout": "vertical", "width": "$grow",
            "children": [
                { "type": "text", "id": "t", "content": "$count", "fontSize": "$ink",
                  "textAlign": "$missing", "fontFamily": "$loop",
                  "fill": [{ "type": "solid", "color": "$ink", "opacity": "$missing" }] }
            ]
        }]
    })
    .to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("document loads");
    let report = loaded.compatibility.pen_dev.expect("converted as pen.dev");
    // fontSize (a colour), textAlign, fontFamily (a cycle), fill opacity.
    assert_eq!(report.unsupported.get("unresolved variable"), Some(&4));

    let (doc, _) = {
        let mut doc: Value = serde_json::from_str(&src).expect("JSON");
        let report = convert(&mut doc, &no_assets);
        (doc, report)
    };
    let frame = child(&doc, "f");
    assert_eq!(frame["width"], json!("fill_container"));
    let text = child(frame, "t");
    assert_eq!(text["content"], json!("3"));
    assert!(text.get("fontSize").is_none() && text.get("textAlign").is_none());
    assert_eq!(text["fill"], json!([{ "type": "solid", "color": "$ink" }]));
}
