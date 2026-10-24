use crate::widgets::layer_panel::LayerPanel;
use de_editor_core::{EditorState, NodeId};

fn state() -> EditorState {
    let src = serde_json::json!({"version":"1.0.0","children":[
        {"type":"frame","id":"card","name":"Card","reusable":true,
         "width":200,"height":120,"children":[
            {"type":"text","id":"title","content":"Hello"},
            {"type":"frame","id":"slot","name":"Content","children":[
                {"type":"text","id":"hint","content":"Drop here"}]}]},
        {"type":"ref","id":"inst","ref":"card","x":300,"y":0,"descendants":{
            "title":{"type":"frame","id":"hero","name":"Hero","children":[
                {"type":"text","id":"big","name":"Big","content":"Hi"}]},
            "slot":{"children":[{"type":"text","id":"mine","name":"Mine","content":"Mine"}]}}},
        {"type":"ref","id":"plain","ref":"card","x":600,"y":0}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&src.to_string())
            .expect("fixture")
            .value,
    )
}

fn rows(state: &EditorState) -> Vec<(String, usize, String)> {
    LayerPanel::from_editor(state)
        .items
        .iter()
        .map(|item| {
            (
                item.node_id.as_str().to_string(),
                item.depth,
                item.label.clone(),
            )
        })
        .collect()
}

#[test]
fn an_instance_lists_its_replacements_and_slot_content() {
    let rows = rows(&state());
    let ids: Vec<&str> = rows.iter().map(|(id, _, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "card",
            "title",
            "slot",
            "hint",
            "inst",
            "inst__slot",
            "inst__mine",
            "inst__hero",
            "inst__big",
            "plain"
        ]
    );
    let depth = |id: &str| rows.iter().find(|(row, _, _)| row == id).unwrap().1;
    assert_eq!(depth("inst__slot"), depth("inst") + 1);
    assert_eq!(depth("inst__mine"), depth("inst") + 2);
    let label = |id: &str| rows.iter().find(|(row, _, _)| row == id).unwrap().2.clone();
    assert_eq!(
        label("inst__slot"),
        "Content",
        "the slot row names the master slot"
    );
}

#[test]
fn owned_rows_collapse_with_the_instance_and_resolve_in_the_inspector() {
    let mut state = state();
    for (id, _, _) in rows(&state) {
        if id.starts_with("inst__") {
            assert!(
                de_editor_core::resolve_instance_display_node_for_anchor(
                    &state.doc,
                    &NodeId::new(&id)
                )
                .is_some(),
                "{id} is an anchor the inspector can show"
            );
        }
    }
    state.editor_ui.collapsed_layers.insert(NodeId::new("inst"));
    assert!(!rows(&state)
        .iter()
        .any(|(id, _, _)| id.starts_with("inst__")));
}

#[test]
fn eye_on_an_owned_row_hides_it_and_stale_entries_get_no_row() {
    let mut state = state();
    assert!(state.toggle_node_hidden(&NodeId::new("inst__mine")));
    let panel = LayerPanel::from_editor(&state);
    let mine = panel
        .items
        .iter()
        .find(|item| item.node_id.as_str() == "inst__mine")
        .expect("row");
    assert!(mine.hidden, "the flag is stored in the slot's entry");

    if let Some(jian_ops_schema::node::PenNode::Ref(reference)) =
        de_editor_core::walkers::find_node_mut(state.active_children_mut(), &NodeId::new("plain"))
    {
        reference.descendants = Some(std::collections::BTreeMap::from([(
            "gone".to_string(),
            serde_json::json!({"children":[{"type":"text","id":"orphan","content":"x"}]}),
        )]));
    }
    assert!(!rows(&state)
        .iter()
        .any(|(id, _, _)| id.starts_with("plain__")));
}
