use super::*;
use crate::command::EditorCommand;
use crate::pen_node_ext::PenNodeExt;
use crate::test_support::{rect, state_with};
use serde_json::{json, Value};

fn node(value: Value) -> PenNode {
    serde_json::from_value(value).expect("node")
}

/// Typed `f64` fields serialize as `400.0`; compare them like the
/// hand-written `400`.
fn whole(value: Value) -> Value {
    match value {
        Value::Number(n) => crate::node_scale::num(n.as_f64().unwrap_or(0.0)),
        Value::Array(items) => Value::Array(items.into_iter().map(whole).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, whole(v))).collect()),
        other => other,
    }
}

fn json_of(state: &EditorState, id: &str) -> Value {
    let found = find_node(state.active_children(), &NodeId::new(id)).expect("node");
    whole(serde_json::to_value(found).expect("json"))
}

/// A card: a vertical auto-layout frame with a title, a stroked
/// rectangle and a free-positioned badge.
fn card() -> PenNode {
    node(json!({
        "type": "frame", "id": "card", "x": 100, "y": 50, "width": 200, "height": 120,
        "layout": "vertical", "gap": 8, "padding": 16, "cornerRadius": 12,
        "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": "#000000"}]},
        "effects": [{"type": "shadow", "offsetX": 0, "offsetY": 2, "blur": 6, "spread": 0, "color": "#00000033"}],
        "children": [
            {"type": "text", "id": "title", "content": "Hi", "fontSize": 14, "letterSpacing": 0.5},
            {"type": "rectangle", "id": "bar", "width": 100, "height": 4},
            {"type": "frame", "id": "badge", "role": "overlay", "x": 150, "y": 10, "width": 30, "height": 30,
             "constraints": {"h": "right", "v": "top"}}
        ]
    }))
}

fn request(id: &str, pivot: Option<(f64, f64)>, factor: f64) -> ScaleRequest {
    ScaleRequest {
        targets: vec![ScaleTarget {
            node_id: NodeId::new(id),
            pivot,
        }],
        factor,
        variables: VariableMode::KeepBound,
    }
}

#[test]
fn scaling_a_card_scales_everything_inside() {
    let mut s = state_with(vec![card()]);
    s.scale_nodes(&request("card", Some((100.0, 50.0)), 2.0))
        .expect("scaled");
    let card = json_of(&s, "card");
    assert_eq!(
        (card["x"].as_f64(), card["y"].as_f64()),
        (Some(100.0), Some(50.0))
    );
    assert_eq!(card["width"], json!(400));
    assert_eq!(card["height"], json!(240));
    assert_eq!(card["gap"], json!(16));
    assert_eq!(card["padding"], json!(32));
    assert_eq!(card["cornerRadius"], json!(24));
    assert_eq!(card["stroke"]["thickness"], json!(2));
    assert_eq!(card["effects"][0]["blur"], json!(12));
    let title = json_of(&s, "title");
    assert_eq!(title["fontSize"], json!(28));
    assert_eq!(title["letterSpacing"], json!(1));
    assert!(title.get("x").is_none(), "a flow child stays in the flow");
    assert_eq!(json_of(&s, "bar")["width"], json!(200));
    let badge = json_of(&s, "badge");
    assert_eq!(
        (badge["x"].clone(), badge["y"].clone()),
        (json!(300), json!(20))
    );
}

#[test]
fn the_pivot_stays_put() {
    let mut s = state_with(vec![rect("a", "A", 10.0, 20.0, 40.0, 40.0)]);
    let centre = ScaleAnchor::Center.point_in(DocRect {
        x: 10.0,
        y: 20.0,
        w: 40.0,
        h: 40.0,
    });
    s.scale_nodes(&request("a", Some(centre), 0.5))
        .expect("scaled");
    let a = json_of(&s, "a");
    assert_eq!((a["x"].as_f64(), a["y"].as_f64()), (Some(20.0), Some(30.0)));
    assert_eq!(a["width"], json!(20));
}

#[test]
fn a_scale_is_one_undo_step() {
    let mut s = state_with(vec![card()]);
    let before = json_of(&s, "card");
    scale_with_history(&mut s, &request("card", Some((100.0, 50.0)), 1.5)).expect("scaled");
    assert_ne!(json_of(&s, "card"), before);
    assert!(s.undo());
    assert_eq!(json_of(&s, "card"), before);
    assert!(!s.undo());
}

#[test]
fn bad_requests_are_refused_before_any_write() {
    let mut locked = rect("l", "L", 0.0, 0.0, 10.0, 10.0);
    locked.base_mut().locked = Some(true);
    let mut s = state_with(vec![card(), locked]);
    let before = s.doc.clone();
    let cases = [
        (
            request("card", None, 0.0),
            ScaleError::FactorOutOfRange(0.0),
        ),
        (request("card", None, 1.0), ScaleError::Unchanged),
        (
            request("card", None, f64::NAN),
            ScaleError::FactorOutOfRange(f64::NAN),
        ),
        (
            request("nope", None, 2.0),
            ScaleError::NodeNotFound("nope".into()),
        ),
        (request("l", None, 2.0), ScaleError::NotEditable("l".into())),
    ];
    for (req, want) in cases {
        let got = s.scale_nodes(&req).expect_err("refused");
        assert_eq!(got.to_string(), want.to_string());
    }
    let mut empty = request("card", None, 2.0);
    empty.targets.clear();
    assert_eq!(s.scale_nodes(&empty), Err(ScaleError::NoTargets));
    assert_eq!(s.doc, before);
}

#[test]
fn a_target_inside_another_scales_once() {
    let mut s = state_with(vec![card()]);
    let mut req = request("card", Some((100.0, 50.0)), 2.0);
    req.targets.push(ScaleTarget {
        node_id: NodeId::new("bar"),
        pivot: None,
    });
    s.scale_nodes(&req).expect("scaled");
    assert_eq!(json_of(&s, "bar")["width"], json!(200));
}

#[test]
fn the_command_applies_like_the_api() {
    let mut s = state_with(vec![card()]);
    assert!(s.apply(EditorCommand::Scale(request(
        "card",
        Some((100.0, 50.0)),
        2.0
    ))));
    assert_eq!(json_of(&s, "card")["width"], json!(400));
    assert!(!s.apply(EditorCommand::Scale(request("card", None, 500.0))));
    assert!(s.undo(), "the MCP write is one undo step");
    assert_eq!(json_of(&s, "card")["width"], json!(200));
}

#[test]
fn from_resolved_converts_the_pivot_into_parent_space() {
    // A child at x=150 in a parent whose page origin is (100, 50).
    let t = ScaleTarget::from_resolved(
        NodeId::new("badge"),
        (250.0, 60.0),
        (250.0, 60.0),
        (Some(150.0), Some(10.0)),
    );
    assert_eq!(t.pivot, Some((150.0, 10.0)));
}

#[test]
fn anchors_parse_and_place() {
    assert_eq!(
        ScaleAnchor::parse("bottom_right"),
        Some(ScaleAnchor::BottomRight)
    );
    assert_eq!(ScaleAnchor::parse("Centre"), Some(ScaleAnchor::Center));
    assert_eq!(ScaleAnchor::parse("middle"), None);
    let r = DocRect {
        x: 0.0,
        y: 0.0,
        w: 10.0,
        h: 20.0,
    };
    assert_eq!(ScaleAnchor::BottomRight.point_in(r), (10.0, 20.0));
    assert_eq!(ScaleAnchor::Top.point_in(r), (5.0, 0.0));
    assert!(factor_for_size(0.0, 10.0).is_err());
    assert_eq!(factor_for_size(50.0, 100.0), Ok(2.0));
}

/// A master button and an instance of it with one existing override.
fn instance_state() -> EditorState {
    state_with(vec![
        node(json!({
            "type": "frame", "id": "btn", "reusable": true, "x": 0, "y": 0,
            "width": 100, "height": 40, "padding": [8, 12], "cornerRadius": 6,
            "children": [{"type": "text", "id": "label", "content": "Go", "fontSize": 14}]
        })),
        node(json!({
            "type": "ref", "id": "inst", "ref": "btn", "x": 200, "y": 0,
            "descendants": {"label": {"content": "Buy"}}
        })),
    ])
}

#[test]
fn an_instance_scales_through_overrides() {
    let mut s = instance_state();
    s.scale_nodes(&request("inst", Some((200.0, 0.0)), 2.0))
        .expect("scaled");
    let inst = json_of(&s, "inst");
    assert_eq!(inst["x"], json!(200));
    let d = &inst["descendants"];
    assert_eq!(d["btn"]["width"], json!(200));
    assert_eq!(d["btn"]["height"], json!(80));
    assert_eq!(d["btn"]["padding"], json!([16, 24]));
    assert_eq!(d["btn"]["cornerRadius"], json!(12));
    assert!(
        d["btn"].get("x").is_none(),
        "the instance owns its position"
    );
    assert_eq!(d["label"]["fontSize"], json!(28));
    assert_eq!(
        d["label"]["content"],
        json!("Buy"),
        "existing overrides stay"
    );
    let master = json_of(&s, "btn");
    assert_eq!(master["width"], json!(100), "the master is untouched");
}

#[test]
fn a_scaled_instance_renders_like_a_scaled_master() {
    let mut s = instance_state();
    s.scale_nodes(&request("inst", Some((200.0, 0.0)), 2.0))
        .expect("scaled");
    let resolved = crate::ref_resolve::resolve_refs_for_canvas_roots(s.active_children(), &s.doc);
    let shown = resolved
        .iter()
        .find(|n| n.id_str() == "inst")
        .expect("expanded");
    let shown = whole(serde_json::to_value(shown).expect("json"));
    assert_eq!(shown["width"], json!(200));
    assert_eq!(shown["children"][0]["fontSize"], json!(28));
}

#[test]
fn nested_instances_get_path_overrides() {
    let mut s = state_with(vec![
        node(json!({
            "type": "frame", "id": "icon", "reusable": true, "width": 16, "height": 16,
            "children": [{"type": "rectangle", "id": "glyph", "width": 10, "height": 10}]
        })),
        node(json!({
            "type": "frame", "id": "row", "reusable": true, "width": 100, "height": 20,
            "children": [{"type": "ref", "id": "ic", "ref": "icon", "x": 2, "y": 2}]
        })),
        node(json!({"type": "ref", "id": "r1", "ref": "row", "x": 0, "y": 100})),
    ]);
    s.scale_nodes(&request("r1", None, 2.0)).expect("scaled");
    let d = json_of(&s, "r1")["descendants"].clone();
    assert_eq!(d["row"]["width"], json!(200));
    assert_eq!(d["ic"]["x"], json!(4));
    assert_eq!(d["ic"]["width"], json!(32));
    assert_eq!(d["ic/glyph"]["width"], json!(20));
    let resolved = crate::ref_resolve::resolve_refs_for_canvas_roots(s.active_children(), &s.doc);
    let shown = whole(
        serde_json::to_value(resolved.iter().find(|n| n.id_str() == "r1").unwrap()).expect("json"),
    );
    let icon = &shown["children"][0];
    assert_eq!(icon["width"], json!(32));
    assert_eq!(icon["x"], json!(4));
    assert_eq!(icon["children"][0]["width"], json!(20));
}

#[test]
fn an_instance_child_is_refused() {
    let mut s = instance_state();
    let err = s
        .scale_nodes(&request("inst__label", None, 2.0))
        .expect_err("refused");
    assert_eq!(err, ScaleError::InstanceChild("inst__label".into()));
}

#[test]
fn an_unresolved_instance_only_moves() {
    let mut s = state_with(vec![node(json!({
        "type": "ref", "id": "lost", "ref": "gone", "x": 10, "y": 10
    }))]);
    let report = s
        .scale_nodes(&request("lost", Some((0.0, 0.0)), 2.0))
        .expect("scaled");
    assert_eq!(report.unresolved_instances, vec!["lost".to_string()]);
    assert_eq!(json_of(&s, "lost")["x"], json!(20));
}

#[test]
fn detach_inside_an_instance_keeps_typed_bindings_and_says_so() {
    let mut s = state_with(vec![
        node(json!({
            "type": "frame", "id": "btn", "reusable": true, "width": 100, "height": 40,
            "children": [{"type": "text", "id": "label", "content": "Go", "fontSize": 14,
                "variableBindings": {"fontSize": {"variable": "body", "value": 14}}}]
        })),
        node(json!({"type": "ref", "id": "inst", "ref": "btn", "x": 200, "y": 0})),
    ]);
    let mut req = request("inst", Some((200.0, 0.0)), 2.0);
    req.variables = VariableMode::Detach;
    let report = s.scale_nodes(&req).expect("scaled");
    assert!(report.detached.is_empty(), "nothing was really unbound");
    assert_eq!(report.bound.len(), 1);
    assert_eq!(report.bound[0].field, "fontSize");
    let inst = json_of(&s, "inst");
    assert!(inst["descendants"]["label"].get("fontSize").is_none());
    assert_eq!(inst["descendants"]["btn"]["width"], json!(200));
}
