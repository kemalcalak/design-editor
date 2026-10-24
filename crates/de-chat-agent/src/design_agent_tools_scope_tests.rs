//! Per-batch repairs touch only what the batch wrote (#116).

use super::*;

/// A hand-made phone board the per-batch repairs would rewrite if they saw
/// it: a canonical status bar plus a nested bar-looking frame (the duplicate
/// sweep), and an icon-shaped path (icon normalization).
const HAND_MADE_BOARD: &str = r##"{ "version": "1.0", "children": [{
    "type": "frame", "id": "board", "name": "Music Home", "x": 0, "y": 0,
    "width": 402, "height": 874, "layout": "vertical",
    "children": [
        { "type": "frame", "id": "sb", "name": "Status Bar", "role": "status-bar",
          "width": "fill_container", "height": 62,
          "children": [
            { "type": "text", "id": "time", "name": "Time", "content": "9:41",
              "width": 54, "height": 22 },
            { "type": "frame", "id": "lv", "name": "Levels", "width": 70, "height": 22 }
          ] },
        { "type": "frame", "id": "hdr", "name": "Header",
          "width": "fill_container", "height": "fit_content",
          "children": [
            { "type": "frame", "id": "fake", "name": "Status Bar 2",
              "width": "fill_container", "height": 44 },
            { "type": "path", "id": "chevron", "name": "Chevron", "d": "M6 9l6 6 6-6",
              "width": 14, "height": 14,
              "stroke": { "thickness": 2, "fill": [{ "type": "solid", "color": "#111111" }] } }
          ] }
    ]
}] }"##;

fn hand_made_state() -> EditorState {
    EditorState::from_document(serde_json::from_str(HAND_MADE_BOARD).expect("document"))
}

fn find<'a>(nodes: &'a [PenNode], name: &str) -> Option<&'a PenNode> {
    nodes.iter().find_map(|node| {
        if node.base().name.as_deref() == Some(name) {
            Some(node)
        } else {
            find(node.children().map(Vec::as_slice).unwrap_or(&[]), name)
        }
    })
}

fn board_json(state: &EditorState) -> serde_json::Value {
    let board = state
        .active_children()
        .iter()
        .find(|root| root.id_str() == "board")
        .expect("board");
    serde_json::to_value(board).expect("serialize")
}

fn batch(state: &mut EditorState, operations: &str) {
    let args = serde_json::json!({ "operations": operations }).to_string();
    let (result, mutated) = execute_design_tool_with_reveals(state, "batch_design", &args, None);
    assert!(mutated && !result.is_error, "{}", result.content);
}

#[test]
fn a_batch_next_to_a_hand_made_board_leaves_it_unchanged() {
    let mut state = hand_made_state();
    let before = board_json(&state);

    batch(
        &mut state,
        r##"I(null, {type:'frame', name:'Generated', x:600, y:0, width:375, height:812, layout:'vertical', children:[{type:'path', name:'Generated Chevron', d:'M6 9l6 6 6-6', width:14, height:14, stroke:{thickness:2, fill:[{type:'solid', color:'#111111'}]}}]})"##,
    );

    assert_eq!(board_json(&state), before, "the batch rewrote the board");
    let generated = find(state.active_children(), "Generated Chevron").expect("generated");
    assert!(
        matches!(generated, PenNode::IconFont(_)),
        "the batch's own icon path is normalized: {generated:?}"
    );
}

#[test]
fn a_batch_into_the_board_repairs_only_the_nodes_it_added() {
    let mut state = hand_made_state();

    batch(
        &mut state,
        r##"I("hdr", {type:'path', name:'Added Chevron', d:'M6 9l6 6 6-6', width:14, height:14, stroke:{thickness:2, fill:[{type:'solid', color:'#111111'}]}})
I("hdr", {type:'frame', name:'Status Bar 3', width:'fill_container', height:44})"##,
    );

    let roots = state.active_children();
    assert!(
        matches!(find(roots, "Added Chevron"), Some(PenNode::IconFont(_))),
        "the added icon path is normalized"
    );
    assert!(
        find(roots, "Status Bar 3").is_none(),
        "the duplicate bar the batch built is removed"
    );
    assert!(
        matches!(find(roots, "Chevron"), Some(PenNode::Path(_))),
        "the board's own icon path stays a path"
    );
    assert!(
        find(roots, "Status Bar 2").is_some(),
        "a bar-looking frame already on the board stays"
    );
}
