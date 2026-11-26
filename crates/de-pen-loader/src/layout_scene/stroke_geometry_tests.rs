use super::*;
use jian_ops_schema::node::PenNode;
use std::collections::BTreeMap;

/// The payload the adapter builds for `json` laid out at `rect`.
fn payload(json: &str, rect: [f32; 4]) -> NodePayload {
    let node: PenNode = serde_json::from_str(json).expect("node");
    let rects = BTreeMap::from([("l".to_owned(), rect), ("p".to_owned(), rect)]);
    crate::adapter::node_to_payload(&node, &rects)
}

const STROKE: &str = r##""stroke":{"thickness":2,"cap":"round","strokeEnd":"triangle_arrow","fill":[{"type":"solid","color":"#000000"}]}"##;

#[test]
fn a_line_without_end_caps_has_no_geometry() {
    let n = payload(
        r#"{"type":"line","id":"l","x":50,"y":20,"x2":100,"y2":0,"stroke":{"thickness":2}}"#,
        [50.0, 20.0, 100.0, 0.0],
    );
    assert!(scene_stroke_geometry(&n).is_none());
}

#[test]
fn a_line_arrow_is_local_to_the_box_and_falls_back_to_cap_at_the_start() {
    let n = payload(
        &format!(r#"{{"type":"line","id":"l","x":50,"y":20,"x2":100,"y2":0,{STROKE}}}"#),
        [50.0, 20.0, 100.0, 0.0],
    );
    let g = scene_stroke_geometry(&n).expect("geometry");
    assert!(g.fill_d.contains("M 100 0"), "{}", g.fill_d);
    // The start falls back to the round cap: a half circle behind 0,0.
    assert!(g.fill_d.contains("M 0 -1"), "{}", g.fill_d);
    assert!(g.stroke_d.starts_with("M 0 0 L 90.5 0"), "{}", g.stroke_d);
}

#[test]
fn svg_paths_are_fitted_to_the_node_box() {
    let mut n = payload(
        &format!(
            r#"{{"type":"path","id":"p","d":"M10 10 L110 60","width":200,"height":100,{STROKE}}}"#
        ),
        [0.0, 0.0, 200.0, 100.0],
    );
    n.w = 200.0;
    n.h = 100.0;
    let g = scene_stroke_geometry(&n).expect("geometry");
    // 100 × 50 of path data stretched to 200 × 100: the end lands at 200,100.
    assert!(g.fill_d.contains("M 200 100"), "{}", g.fill_d);
    assert!(g.stroke_d.starts_with("M 0 0"), "{}", g.stroke_d);
}

#[test]
fn closed_outlines_have_no_ends() {
    let n = payload(
        &format!(
            r#"{{"type":"path","id":"p","d":"M0 0 L40 0 L40 40 Z","width":40,"height":40,{STROKE}}}"#
        ),
        [0.0, 0.0, 40.0, 40.0],
    );
    assert!(scene_stroke_geometry(&n).is_none());
}
