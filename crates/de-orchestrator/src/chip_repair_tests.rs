//! Adopted corner badges and notification dots must keep their pinned x/y
//! inside an auto-layout parent, which jian only honours for overlays.

use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use super::{adopt_corner_badges, adopt_notification_dots};

fn node(v: Value) -> PenNode {
    serde_json::from_value(v).expect("fixture parses")
}

fn child(root: &PenNode, path: &[usize]) -> Value {
    let mut v = serde_json::to_value(root).expect("serialize");
    for &i in path {
        v = v["children"][i].take();
    }
    v
}

#[test]
fn adopted_notification_dot_is_an_overlay() {
    let mut button = node(json!({
        "type": "frame", "id": "btn", "width": 40, "height": 40,
        "layout": "horizontal", "alignItems": "center",
        "children": [
            {"type": "icon_font", "id": "bell", "iconFontName": "bell", "width": 20, "height": 20},
            {"type": "frame", "id": "dot", "width": 8, "height": 8,
             "fill": [{"type": "solid", "color": "#22C55E"}]}
        ]
    }));
    assert!(adopt_notification_dots(&mut button));
    let dot = child(&button, &[1]);
    assert_eq!(
        (dot["x"].as_f64(), dot["y"].as_f64()),
        (Some(16.0), Some(0.0))
    );
    assert_eq!(dot["role"], "overlay");
}

#[test]
fn adopted_corner_badge_is_an_overlay() {
    let mut row = node(json!({
        "type": "frame", "id": "row", "width": 300, "height": 200, "layout": "horizontal",
        "children": [
            {"type": "frame", "id": "media", "width": 200, "height": 200, "layout": "vertical",
             "children": [{"type": "image", "id": "img", "src": "", "width": 200, "height": 200}]},
            {"type": "frame", "id": "badge", "fill": [{"type": "solid", "color": "#EF4444"}],
             "children": [{"type": "text", "id": "pct", "content": "-35%"}]}
        ]
    }));
    assert!(adopt_corner_badges(&mut row));
    let badge = child(&row, &[0, 1]);
    assert_eq!(
        (badge["x"].as_f64(), badge["y"].as_f64()),
        (Some(8.0), Some(8.0))
    );
    assert_eq!(badge["role"], "overlay");
}
