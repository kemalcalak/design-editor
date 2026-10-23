use super::*;
use jian_ops_schema::document::PenDocument;

fn state_with(json: &str) -> EditorState {
    let doc: PenDocument = jian_ops_schema::load_str(json)
        .expect("fixture parses")
        .value;
    EditorState::from_document(doc)
}

const DOC: &str = r#"{"version":"1.0.0","children":[
  {"type":"frame","id":"f","name":"Hero","x":0,"y":0,"width":400,"height":300,"children":[
    {"type":"image","id":"a","name":"Photo","src":"data:image/png;base64,AA==","width":10,"height":10}
  ]},
  {"type":"rectangle","id":"r","x":0,"y":0,"width":10,"height":10},
  {"type":"image","id":"b","src":"https://example.com/b.png","width":10,"height":10}
]}"#;

#[test]
fn lists_image_nodes_depth_first() {
    let images = document_images(&state_with(DOC));
    let ids: Vec<_> = images
        .iter()
        .map(|i| i.node_id.as_str().to_string())
        .collect();
    assert_eq!(ids, ["a", "b"]);
    assert_eq!(images[0].name, "Photo");
    assert_eq!(images[1].name, "");
    assert_eq!(images[1].src, "https://example.com/b.png");
}

#[test]
fn search_on_a_selected_image_reuses_it() {
    let mut state = state_with(DOC);
    state.set_single_selection(NodeId::new("a"));
    let inserted = apply_images_tab_action(&mut state, ImagesTabAction::Search, 1);
    assert!(!inserted);
    assert_eq!(state.selection.anchor.as_str(), "a");
    assert!(state.editor_ui.image_panel.search_open);
    assert_eq!(state.editor_ui.image_panel.search_query.text(), "Photo");
}

#[test]
fn generate_without_an_image_selected_inserts_a_placeholder() {
    let mut state = state_with(DOC);
    state.set_single_selection(NodeId::new("r"));
    let before = document_images(&state).len();
    let inserted = apply_images_tab_action(&mut state, ImagesTabAction::Generate, 1);
    assert!(inserted);
    assert_eq!(document_images(&state).len(), before + 1);
    assert!(matches!(state.selected_node(), Some(PenNode::Image(_))));
    let panel = &state.editor_ui.image_panel;
    assert!(panel.generate_open && !panel.search_open);
    assert_eq!(panel.generate_prompt.text(), "");
}
