use std::collections::BTreeMap;

use de_editor_core::{EditorCommand, EditorState, NodeId, PenNodeExt};

use crate::root_placement::{place_new_roots, places_new_roots};

/// A page holding one non-empty 1200×800 frame at the origin — the shape
/// of the issue's "existing design" (an empty starter would be replaced
/// instead of joined).
fn seeded_state() -> EditorState {
    let src = r#"{"version":"1.0.0","children":[
      {"type":"frame","id":"seed","name":"Frame","x":0,"y":0,"width":1200,"height":800,
       "children":[{"type":"text","id":"seed-text","x":40,"y":40,"content":"Existing"}]}
    ]}"#;
    let parsed = jian_ops_schema::load_str(src).expect("parse fixture");
    EditorState::from_document(parsed.value)
}

fn rect_at(x: i32, y: i32) -> EditorCommand {
    rect_under(NodeId::NONE, x, y)
}

fn rect_under(target_parent: NodeId, x: i32, y: i32) -> EditorCommand {
    EditorCommand::InsertNode {
        kind: "rect".into(),
        name: "New".into(),
        x,
        y,
        width: 400,
        height: 300,
        fill_hex: None,
        target_parent,
        page_id: None,
    }
}

fn unpositioned_frame(name: &str) -> EditorCommand {
    let node: jian_ops_schema::node::PenNode = serde_json::from_value(serde_json::json!({
        "type": "frame", "id": name, "name": name, "width": 390, "height": 844,
        // Not empty: an empty root frame counts as a blank starter, which
        // the next root insert replaces instead of joining.
        "children": [{"type": "text", "id": format!("{name}-title"), "content": name}]
    }))
    .expect("frame node");
    EditorCommand::InsertSubtree {
        nodes: vec![node],
        parent_id: NodeId::NONE,
        page_id: None,
    }
}

fn no_args() -> BTreeMap<String, String> {
    BTreeMap::new()
}

/// Apply what the host would apply: the placed command when there is one.
fn apply_placed(
    state: &mut EditorState,
    tool: &str,
    args: &BTreeMap<String, String>,
    cmd: EditorCommand,
) {
    let placed = place_new_roots(state, tool, args, &cmd).unwrap_or(cmd);
    assert!(state.apply(placed), "placed command applies");
}

fn root_names_and_origins(state: &EditorState) -> Vec<(String, Option<f64>, Option<f64>)> {
    state
        .active_children()
        .iter()
        .map(|node| {
            let base = node.base();
            (base.name.clone().unwrap_or_default(), base.x, base.y)
        })
        .collect()
}

#[test]
fn content_tools_are_in_scope_and_offset_tools_are_not() {
    for tool in [
        "insert_node",
        "insert_button",
        "batch_design",
        "design_skeleton",
        "design_content",
        "import_svg",
        "import_html",
        "import_pen_dev",
        "import_pptx",
        "import_html_url",
        "import_web_snapshot",
        "upsert_screen",
        "upsert_component",
        "instantiate_component",
        "copy_node",
    ] {
        assert!(places_new_roots(tool), "{tool} should place new roots");
    }
    for tool in [
        "duplicate_selected",
        "paste_clipboard",
        "use_scene_template",
        "add_page",
        "duplicate_page",
        "run_design_agent",
        "update_node",
        "move_node",
    ] {
        assert!(!places_new_roots(tool), "{tool} should be left alone");
    }
}

#[test]
fn overlapping_insert_moves_right_of_content_and_to_the_front() {
    let mut state = seeded_state();
    apply_placed(&mut state, "insert_node", &no_args(), rect_at(200, 200));
    assert_eq!(
        root_names_and_origins(&state),
        vec![
            ("New".into(), Some(1280.0), Some(0.0)),
            ("Frame".into(), Some(0.0), Some(0.0)),
        ]
    );
}

#[test]
fn insert_in_free_space_keeps_its_position_but_comes_to_the_front() {
    let mut state = seeded_state();
    apply_placed(&mut state, "insert_node", &no_args(), rect_at(0, 900));
    assert_eq!(
        root_names_and_origins(&state),
        vec![
            ("New".into(), Some(0.0), Some(900.0)),
            ("Frame".into(), Some(0.0), Some(0.0)),
        ]
    );
}

#[test]
fn allow_overlap_keeps_the_position_and_still_comes_to_the_front() {
    let mut state = seeded_state();
    let args = BTreeMap::from([("allowOverlap".to_string(), "true".to_string())]);
    apply_placed(&mut state, "insert_node", &args, rect_at(200, 200));
    assert_eq!(
        root_names_and_origins(&state),
        vec![
            ("New".into(), Some(200.0), Some(200.0)),
            ("Frame".into(), Some(0.0), Some(0.0)),
        ]
    );
}

#[test]
fn unpositioned_frames_line_up_right_of_content() {
    let mut state = seeded_state();
    apply_placed(
        &mut state,
        "insert_node",
        &no_args(),
        unpositioned_frame("B"),
    );
    apply_placed(
        &mut state,
        "insert_node",
        &no_args(),
        unpositioned_frame("C"),
    );
    assert_eq!(
        root_names_and_origins(&state),
        vec![
            ("C".into(), Some(1750.0), Some(0.0)),
            ("B".into(), Some(1280.0), Some(0.0)),
            ("Frame".into(), Some(0.0), Some(0.0)),
        ]
    );
}

#[test]
fn parented_insert_is_left_alone() {
    let mut state = seeded_state();
    let cmd = rect_under(NodeId::new("seed".to_string()), 200, 200);
    assert!(place_new_roots(&mut state, "insert_node", &no_args(), &cmd).is_none());
}

#[test]
fn out_of_scope_tool_is_left_alone() {
    let mut state = seeded_state();
    assert!(place_new_roots(&mut state, "update_node", &no_args(), &rect_at(200, 200)).is_none());
}

#[test]
fn first_root_on_an_empty_page_needs_no_change() {
    let mut state = EditorState::from_document(
        jian_ops_schema::load_str(r#"{"version":"1.0.0","children":[]}"#)
            .expect("empty doc")
            .value,
    );
    assert!(place_new_roots(
        &mut state,
        "insert_node",
        &no_args(),
        &unpositioned_frame("A")
    )
    .is_none());
}

#[test]
fn placement_and_insert_undo_as_one_step() {
    let mut state = seeded_state();
    apply_placed(&mut state, "insert_node", &no_args(), rect_at(200, 200));
    assert!(state.apply(EditorCommand::Undo));
    assert_eq!(
        root_names_and_origins(&state),
        vec![("Frame".into(), Some(0.0), Some(0.0))]
    );
}

#[test]
fn rejected_command_is_returned_unplaced() {
    let mut state = seeded_state();
    let cmd = rect_under(NodeId::new("missing".to_string()), 200, 200);
    assert!(place_new_roots(&mut state, "insert_node", &no_args(), &cmd).is_none());
}

#[test]
fn insert_on_a_page_that_is_not_active_is_placed_on_that_page() {
    let src = r#"{"version":"1.0.0","children":[],"pages":[
      {"id":"p1","name":"One","children":[]},
      {"id":"p2","name":"Two","children":[
        {"type":"frame","id":"seed","name":"Frame","x":0,"y":0,"width":1200,"height":800,
         "children":[{"type":"text","id":"seed-text","x":40,"y":40,"content":"Existing"}]}
      ]}
    ]}"#;
    let mut state =
        EditorState::from_document(jian_ops_schema::load_str(src).expect("parse").value);
    let cmd = match rect_at(200, 200) {
        EditorCommand::InsertNode {
            kind,
            name,
            x,
            y,
            width,
            height,
            fill_hex,
            target_parent,
            ..
        } => EditorCommand::InsertNode {
            kind,
            name,
            x,
            y,
            width,
            height,
            fill_hex,
            target_parent,
            page_id: Some("p2".into()),
        },
        _ => unreachable!(),
    };
    apply_placed(&mut state, "insert_node", &no_args(), cmd);
    let p2 = &state.doc.pages.as_ref().expect("pages")[1].children;
    let got: Vec<(Option<String>, Option<f64>, Option<f64>)> = p2
        .iter()
        .map(|n| (n.base().name.clone(), n.base().x, n.base().y))
        .collect();
    assert_eq!(
        got,
        vec![
            (Some("New".into()), Some(1280.0), Some(0.0)),
            (Some("Frame".into()), Some(0.0), Some(0.0)),
        ]
    );
    assert!(state.doc.pages.as_ref().expect("pages")[0]
        .children
        .is_empty());
}

#[test]
fn kit_insert_without_position_lands_beside_content_even_when_origin_is_free() {
    use crate::McpTool;
    let src = r#"{"version":"1.0.0","children":[
      {"type":"frame","id":"seed","name":"Frame","x":600,"y":40,"width":390,"height":844,
       "children":[{"type":"text","id":"seed-text","x":10,"y":10,"content":"Existing"}]}
    ]}"#;
    let mut state =
        EditorState::from_document(jian_ops_schema::load_str(src).expect("parse").value);
    let kit_tool = crate::element_tools::insert_kit_component_tools(&state)
        .into_iter()
        .next()
        .expect("a kit component");
    let crate::ToolOutcome::OkWithCommand(_, cmd) = kit_tool.call(&no_args()) else {
        panic!("kit insert returns a command");
    };
    apply_placed(&mut state, kit_tool.name(), &no_args(), cmd);
    let front = &state.active_children()[0];
    assert_ne!(front.id_str(), "seed", "the kit root comes to the front");
    assert_eq!((front.base().x, front.base().y), (Some(1070.0), Some(40.0)));
}
