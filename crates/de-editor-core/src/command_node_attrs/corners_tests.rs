use super::*;
use crate::command::EditorCommand;
use crate::test_support::{rect, state_with};
use jian_ops_schema::node::PenNode;

fn id(s: &str) -> NodeId {
    NodeId::new(s)
}

fn node_json(state: &EditorState, node: &str) -> serde_json::Value {
    serde_json::to_value(find_node(state.active_children(), &id(node)).unwrap()).unwrap()
}

fn polygon_and_path() -> Vec<PenNode> {
    vec![
        serde_json::from_str(r#"{"type":"polygon","id":"p","polygonCount":5}"#).unwrap(),
        serde_json::from_str(r#"{"type":"path","id":"q","d":"M0 0 L10 0 L10 10"}"#).unwrap(),
        serde_json::from_str(r#"{"type":"ellipse","id":"e"}"#).unwrap(),
    ]
}

#[test]
fn radii_and_smoothing_land_in_one_step() {
    let mut s = state_with(vec![rect("r", "r", 0.0, 0.0, 100.0, 60.0)]);
    assert!(s.apply(EditorCommand::SetNodeCorners {
        node_id: id("r"),
        radii: Some([4.0, 8.0, 12.0, 16.0]),
        smoothing: Some(0.6),
    }));
    let json = node_json(&s, "r");
    assert_eq!(
        json["cornerRadius"],
        serde_json::json!([4.0, 8.0, 12.0, 16.0])
    );
    assert_eq!(json["cornerSmoothing"], serde_json::json!(0.6));
    // Equal radii store one radius; smoothing 0 clears the field.
    assert!(s.apply(EditorCommand::SetNodeCorners {
        node_id: id("r"),
        radii: Some([10.0; 4]),
        smoothing: Some(0.0),
    }));
    let json = node_json(&s, "r");
    assert_eq!(json["cornerRadius"], serde_json::json!(10.0));
    assert!(json.get("cornerSmoothing").is_none(), "{json}");
    // Smoothing alone keeps the radius.
    assert!(s.apply(EditorCommand::SetNodeCorners {
        node_id: id("r"),
        radii: None,
        smoothing: Some(1.0),
    }));
    let json = node_json(&s, "r");
    assert_eq!(json["cornerRadius"], serde_json::json!(10.0));
    assert_eq!(json["cornerSmoothing"], serde_json::json!(1.0));
}

#[test]
fn polygons_and_paths_take_one_radius_and_smoothing() {
    let mut s = state_with(polygon_and_path());
    for node in ["p", "q"] {
        assert!(s.apply(EditorCommand::SetNodeCorners {
            node_id: id(node),
            radii: Some([6.0; 4]),
            smoothing: Some(0.25),
        }));
        let json = node_json(&s, node);
        assert_eq!(json["cornerRadius"], serde_json::json!(6.0), "{json}");
        assert_eq!(json["cornerSmoothing"], serde_json::json!(0.25), "{json}");
    }
    // The legacy uniform command now reaches paths too.
    assert!(s.apply(EditorCommand::SetNodeCornerRadius {
        node_id: id("q"),
        radius: 3.0,
    }));
    assert_eq!(node_json(&s, "q")["cornerRadius"], serde_json::json!(3.0));
}

#[test]
fn what_a_node_cannot_take_is_refused_without_writing() {
    let mut s = state_with(polygon_and_path());
    let before = s.doc.clone();
    let refused = [
        // A polygon has one radius.
        ("p", Some([1.0, 2.0, 3.0, 4.0]), None),
        // An ellipse has no smoothing (and nothing is half-written).
        ("e", Some([4.0; 4]), Some(0.5)),
        // Out of range / not finite.
        ("p", Some([-1.0; 4]), None),
        ("p", None, Some(1.5)),
        ("p", None, Some(f32::NAN)),
        // Nothing asked.
        ("p", None, None),
        // No such node.
        ("missing", Some([1.0; 4]), None),
    ];
    for (node, radii, smoothing) in refused {
        assert!(
            !s.apply(EditorCommand::SetNodeCorners {
                node_id: id(node),
                radii,
                smoothing,
            }),
            "{node} {radii:?} {smoothing:?}"
        );
    }
    assert_eq!(s.doc, before);
}

#[test]
fn stored_smoothing_drops_float_noise() {
    assert_eq!(stored_smoothing(0.6), Some(0.6));
    assert_eq!(stored_smoothing(0.0), None);
    assert_eq!(stored_smoothing(0.00001), None);
}
