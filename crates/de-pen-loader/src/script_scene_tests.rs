//! A script node reaches the canvas as a frame holding its generated
//! children (#19).

use crate::editor_state_to_active_page_layout_scene;
use de_editor_core::{EditorState, PenNodeExt};
use jian_scene::layout_scene::NodeKind;

const DOC: &str = r##"{
  "version":"1.0.0",
  "children":[
    {"type":"script","id":"chart","x":10,"y":20,"width":200,"height":100,"clipContent":true,
     "script":"return [];","inputs":{"bars":2},"scriptHash":"h",
     "children":[
       {"type":"rectangle","id":"chart-g0","x":0,"y":50,"width":40,"height":50},
       {"type":"rectangle","id":"chart-g1","x":60,"y":20,"width":40,"height":80}
     ]}
  ]
}"##;

fn state() -> EditorState {
    EditorState::from_document(
        jian_ops_schema::load_str(DOC)
            .expect("fixture parses")
            .value,
    )
}

#[test]
fn script_node_paints_as_a_clipping_frame_with_its_children() {
    let scene = editor_state_to_active_page_layout_scene(&state());
    let page = &scene.pages[scene.active_page_index];
    let chart = &page.children[0];
    assert_eq!(chart.id, "chart");
    assert_eq!(chart.kind, NodeKind::Frame);
    assert!(chart.clip_content);
    assert_eq!(chart.bounds.size.x, 200.0);
    let ids: Vec<&str> = chart.children.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["chart-g0", "chart-g1"]);
    // Children sit at the script's origin plus their own offset.
    assert_eq!(chart.children[1].bounds.origin.x, 70.0);
}

#[test]
fn script_children_are_walked_like_frame_children() {
    let state = state();
    let chart = &state.active_children()[0];
    assert_eq!(chart.children().map(Vec::len), Some(2));
    // Nothing is inserted into a script: its program owns the children.
    assert!(!chart.is_container());
}
