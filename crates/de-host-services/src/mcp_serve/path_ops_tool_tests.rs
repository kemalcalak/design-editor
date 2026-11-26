//! The MCP path tools (#388): `boolean_op` group mode, `flatten`,
//! `outline_stroke`, `offset_path`, `simplify_path`, `reverse_path`.

use super::*;
use de_editor_core::PenNodeExt;
use jian_ops_schema::node::PenNode;
use serde_json::json;

fn state() -> EditorState {
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(json!({
        "version": "1.0",
        "children": [
            {"type":"ellipse", "id":"c1", "x":0, "y":0, "width":100, "height":100,
             "fill":[{"type":"solid","color":"#ff0000"}]},
            {"type":"ellipse", "id":"c2", "x":60, "y":0, "width":100, "height":100},
            {"type":"rectangle", "id":"r", "x":200, "y":0, "width":40, "height":40,
             "stroke":{"thickness":4, "join":"round", "fill":[{"type":"solid","color":"#0000ff"}]}},
            {"type":"path", "id":"p", "x":0, "y":200, "width":30, "height":10, "closed":false,
             "anchors":[{"x":0,"y":200},{"x":10,"y":200},{"x":20,"y":200},{"x":30,"y":210}]},
            {"type":"frame", "id":"f", "x":300, "y":0, "width":50, "height":50, "children":[
                {"type":"rectangle", "id":"inner", "x":0, "y":0, "width":10, "height":10}]}
        ]
    }))
    .expect("document");
    EditorState::from_document(doc)
}

fn call(state: &EditorState, tool: PathTool, args: &[(&str, &str)]) -> ToolOutcome {
    let args: BTreeMap<String, String> = args
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    path_op_snapshot(state, tool).call(&args)
}

fn apply(
    state: &mut EditorState,
    tool: PathTool,
    args: &[(&str, &str)],
) -> BTreeMap<String, String> {
    let outcome = call(state, tool, args);
    let ToolOutcome::OkWithCommand(out, command) = outcome else {
        panic!("expected a write, got {outcome:?}");
    };
    assert!(state.apply(command), "the write applies");
    out
}

fn code(outcome: ToolOutcome) -> ToolErrorCode {
    match outcome {
        ToolOutcome::Err(code, _) => code,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn find<'a>(state: &'a EditorState, id: &str) -> &'a PenNode {
    de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id)).expect(id)
}

#[test]
fn boolean_op_defaults_to_an_editable_group() {
    let mut s = state();
    let depth = s.history.past.len();
    let out = apply(
        &mut s,
        PathTool::Boolean,
        &[("nodeIds", r#"["c2","c1"]"#), ("operation", "subtract")],
    );
    assert_eq!(out.get("mode").map(String::as_str), Some("group"));
    let id = out["id"].clone();
    let PenNode::Boolean(group) = find(&s, &id) else {
        panic!("a boolean group")
    };
    assert_eq!(group.op, BooleanOp::Subtract);
    let kids: Vec<&str> = find(&s, &id)
        .children()
        .unwrap()
        .iter()
        .map(|c| c.id_str())
        .collect();
    assert_eq!(kids, ["c1", "c2"], "the bottom layer is the base");
    assert_eq!(s.history.past.len(), depth + 1);
    // The children remain editable nodes.
    assert!(matches!(find(&s, "c2"), PenNode::Ellipse(_)));
}

#[test]
fn boolean_group_mode_needs_siblings() {
    let s = state();
    assert_eq!(
        code(call(
            &s,
            PathTool::Boolean,
            &[("nodeIds", r#"["c1","inner"]"#), ("operation", "union")]
        )),
        ToolErrorCode::InvalidArgument
    );
}

#[test]
fn flatten_bakes_curves_into_one_path() {
    let mut s = state();
    let out = apply(&mut s, PathTool::Flatten, &[("nodeIds", r#"["c1","c2"]"#)]);
    let PenNode::Path(path) = find(&s, &out["id"]) else {
        panic!("a path")
    };
    let d = path.d.as_deref().unwrap();
    let cubics = d.matches('C').count();
    assert!((1..=8).contains(&cubics), "{cubics} cubics: {d}");
    assert!(de_editor_core::walkers::find_node(s.active_children(), &NodeId::new("c2")).is_none());
}

#[test]
fn flatten_a_boolean_group_made_over_mcp() {
    let mut s = state();
    let group = apply(
        &mut s,
        PathTool::Boolean,
        &[("nodeIds", r#"["c1","c2"]"#), ("operation", "union")],
    )["id"]
        .clone();
    let ids = format!(r#"["{group}"]"#);
    let out = apply(&mut s, PathTool::Flatten, &[("nodeIds", &ids)]);
    let PenNode::Path(path) = find(&s, &out["id"]) else {
        panic!()
    };
    assert_eq!(path.base.name.as_deref(), Some("Union"));
    assert!((find(&s, &out["id"]).width_px().unwrap() - 160.0).abs() < 0.01);
}

#[test]
fn flatten_refuses_a_frame_with_layers() {
    let s = state();
    assert_eq!(
        code(call(&s, PathTool::Flatten, &[("nodeIds", r#"["f"]"#)])),
        ToolErrorCode::InvalidArgument
    );
}

#[test]
fn outline_stroke_fills_with_the_stroke_paint() {
    let mut s = state();
    let out = apply(&mut s, PathTool::OutlineStroke, &[("nodeId", "r")]);
    let PenNode::Path(path) = find(&s, &out["id"]) else {
        panic!()
    };
    assert!(path.stroke.is_none() && path.fill.is_some());
    // A centred 4 px stroke on a 40 px square spans 44 px.
    assert!((find(&s, &out["id"]).width_px().unwrap() - 44.0).abs() < 0.01);
    assert_eq!(
        code(call(&s, PathTool::OutlineStroke, &[("nodeId", "c2")])),
        ToolErrorCode::ToolFailed,
        "no stroke"
    );
}

#[test]
fn offset_path_grows_and_shrinks() {
    let mut s = state();
    let out = apply(
        &mut s,
        PathTool::Offset,
        &[("nodeId", "r"), ("offset", "5"), ("join", "miter")],
    );
    let node = find(&s, &out["id"]);
    assert_eq!(node.base().x, Some(195.0));
    assert!((node.width_px().unwrap() - 50.0).abs() < 0.01);
    let s = state();
    assert_eq!(
        code(call(
            &s,
            PathTool::Offset,
            &[("nodeId", "r"), ("offset", "-30")]
        )),
        ToolErrorCode::ToolFailed
    );
    assert_eq!(
        code(call(&s, PathTool::Offset, &[("nodeId", "r")])),
        ToolErrorCode::MissingArgument
    );
    assert_eq!(
        code(call(
            &s,
            PathTool::Offset,
            &[("nodeId", "r"), ("offset", "2"), ("join", "x")]
        )),
        ToolErrorCode::InvalidArgument
    );
}

#[test]
fn simplify_and_reverse_edit_a_path_in_place() {
    let mut s = state();
    apply(&mut s, PathTool::Simplify, &[("nodeId", "p")]);
    let PenNode::Path(path) = find(&s, "p") else {
        panic!()
    };
    assert_eq!(path.anchors.as_ref().map(Vec::len), Some(3));
    apply(&mut s, PathTool::Reverse, &[("nodeId", "p")]);
    let PenNode::Path(path) = find(&s, "p") else {
        panic!()
    };
    assert_eq!(path.anchors.as_ref().unwrap()[0].x, 30.0);
    assert_eq!(
        code(call(&s, PathTool::Simplify, &[("nodeId", "c1")])),
        ToolErrorCode::InvalidArgument,
        "not a path"
    );
    assert_eq!(
        code(call(
            &s,
            PathTool::Simplify,
            &[("nodeId", "p"), ("tolerance", "-1")]
        )),
        ToolErrorCode::InvalidArgument
    );
}

#[test]
fn the_tools_carry_their_names() {
    let s = state();
    for (name, tool) in PathTool::NAMES {
        assert_eq!(path_op_snapshot(&s, tool).name(), name);
    }
}
