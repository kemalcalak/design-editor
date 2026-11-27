use super::*;
use jian_ops_schema::node::PenNode;
use std::collections::BTreeMap;

/// The payload the adapter builds for `json` laid out at `rect`.
fn payload(json: &str, rect: [f32; 4]) -> NodePayload {
    let node: PenNode = serde_json::from_str(json).expect("node");
    let rects = BTreeMap::from([("n".to_owned(), rect)]);
    let mut p = crate::adapter::node_to_payload(&node, &rects);
    p.w = rect[2];
    p.h = rect[3];
    p
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn circular_rect_corners_keep_the_rounded_rect_primitive() {
    let n = payload(
        r#"{"type":"rectangle","id":"n","width":100,"height":60,"cornerRadius":12}"#,
        [0.0, 0.0, 100.0, 60.0],
    );
    assert_eq!(n.corner_smoothing, 0.0);
    assert!(scene_corner_path(&n).is_none());
    let flat = payload(
        r#"{"type":"rectangle","id":"n","width":100,"height":60,"cornerSmoothing":0.6}"#,
        [0.0, 0.0, 100.0, 60.0],
    );
    assert!(
        scene_corner_path(&flat).is_none(),
        "no radius, nothing to smooth"
    );
}

#[test]
fn smoothed_rect_outlines_span_the_node_box() {
    let n = payload(
        r#"{"type":"frame","id":"n","width":100,"height":60,"cornerRadius":[4,12,20,0],"cornerSmoothing":0.6}"#,
        [0.0, 0.0, 100.0, 60.0],
    );
    assert!(close(n.corner_smoothing, 0.6));
    let outline = scene_corner_path(&n).expect("outline");
    assert!(
        outline.d.starts_with("M ") && outline.d.ends_with(" Z"),
        "{}",
        outline.d
    );
    let b = outline.bounds;
    assert!(close(b.origin.x, 0.0) && close(b.origin.y, 0.0), "{b:?}");
    assert!(close(b.size.x, 100.0) && close(b.size.y, 60.0), "{b:?}");
    assert!(outline.stroke_d.is_empty());
    // The hit ring closes back on its start.
    assert_eq!(outline.hit_ring.first(), outline.hit_ring.last());
    assert!(outline.hit_ring.len() > 8);
}

#[test]
fn aligned_strokes_get_their_own_centre_line() {
    let inside = payload(
        r##"{"type":"rectangle","id":"n","width":100,"height":60,"cornerRadius":12,"cornerSmoothing":1,"stroke":{"thickness":4,"align":"inside","fill":[{"type":"solid","color":"#000000"}]}}"##,
        [0.0, 0.0, 100.0, 60.0],
    );
    let outline = scene_corner_path(&inside).expect("outline");
    let b = outline.stroke_bounds;
    assert!(close(b.origin.x, 2.0) && close(b.size.x, 96.0), "{b:?}");
    assert!(close(b.origin.y, 2.0) && close(b.size.y, 56.0), "{b:?}");
    let outside = payload(
        r##"{"type":"rectangle","id":"n","width":100,"height":60,"cornerRadius":12,"cornerSmoothing":1,"stroke":{"thickness":4,"align":"outside","fill":[{"type":"solid","color":"#000000"}]}}"##,
        [0.0, 0.0, 100.0, 60.0],
    );
    let b = scene_corner_path(&outside).expect("outline").stroke_bounds;
    assert!(close(b.origin.x, -2.0) && close(b.size.x, 104.0), "{b:?}");
}

#[test]
fn rounded_polygons_and_stars_get_an_outline_even_without_smoothing() {
    let n = payload(
        r#"{"type":"polygon","id":"n","width":80,"height":80,"polygonCount":5,"cornerRadius":6}"#,
        [0.0, 0.0, 80.0, 80.0],
    );
    let outline = scene_corner_path(&n).expect("outline");
    assert!(outline.d.contains(" C "), "{}", outline.d);
    // Rounding pulls the outline inside the sharp polygon's box.
    assert!(outline.bounds.origin.y > 0.0, "{:?}", outline.bounds);
    let star = payload(
        r#"{"type":"polygon","id":"n","width":80,"height":80,"polygonCount":5,"innerRadius":0.4,"cornerRadius":3,"cornerSmoothing":0.5}"#,
        [0.0, 0.0, 80.0, 80.0],
    );
    let outline = scene_corner_path(&star).expect("star outline");
    // Ten rounded vertices, each a transition, an arc and a transition.
    assert!(outline.d.matches(" C ").count() >= 30, "{}", outline.d);
    let sharp = payload(
        r#"{"type":"polygon","id":"n","width":80,"height":80,"polygonCount":5}"#,
        [0.0, 0.0, 80.0, 80.0],
    );
    assert!(scene_corner_path(&sharp).is_none());
}

#[test]
fn paths_round_their_sharp_vertices_and_their_ends_follow() {
    let n = payload(
        r##"{"type":"path","id":"n","d":"M0 0 L100 0 L100 50","width":100,"height":50,"vertexRadius":10,"stroke":{"thickness":2,"strokeEnd":"triangle_arrow","fill":[{"type":"solid","color":"#000000"}]}}"##,
        [0.0, 0.0, 100.0, 50.0],
    );
    let outline = scene_corner_path(&n).expect("outline");
    assert!(outline.d.starts_with("M 0 0 L 90 0 C "), "{}", outline.d);
    assert!(!outline.d.ends_with('Z'));
    let ends = super::super::stroke_geometry::scene_stroke_geometry(&n).expect("ends");
    assert!(ends.stroke_d.contains(" C "), "{}", ends.stroke_d);
    // A path with no straight vertex has nothing to round.
    let curve = payload(
        r#"{"type":"path","id":"n","d":"M0 0 C 50 -40 50 90 100 50","width":100,"height":50,"vertexRadius":10}"#,
        [0.0, 0.0, 100.0, 50.0],
    );
    assert!(scene_corner_path(&curve).is_none());
    // An older file's path `cornerRadius` (its outline already rounded)
    // stays ignored.
    let legacy = payload(
        r#"{"type":"path","id":"n","d":"M0 0 L100 0 L100 50","width":100,"height":50,"cornerRadius":10}"#,
        [0.0, 0.0, 100.0, 50.0],
    );
    assert!(scene_corner_path(&legacy).is_none());
}
