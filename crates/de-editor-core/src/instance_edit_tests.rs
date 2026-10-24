use super::*;
use crate::ref_resolve::resolve_refs_for_canvas;
use crate::EditorCommand;
use jian_ops_schema::node::text::TextContent;
use serde_json::json;

fn state() -> EditorState {
    let doc = json!({"version":"1.0.0","children":[
        {"type":"frame","id":"card","name":"Card","reusable":true,
         "width":200,"height":120,"children":[
            {"type":"text","id":"title","content":"Hello"},
            {"type":"frame","id":"slot","name":"Content","children":[
                {"type":"text","id":"placeholder","content":"Drop here"}]}]},
        {"type":"frame","id":"button","name":"Button","reusable":true,
         "width":120,"height":40,"children":[{"type":"text","id":"label","content":"OK"}]},
        {"type":"frame","id":"toolbar","name":"Toolbar","reusable":true,
         "width":300,"height":48,"children":[{"type":"ref","id":"save","ref":"button"}]},
        {"type":"ref","id":"inst","ref":"card","x":0,"y":200},
        {"type":"ref","id":"bar","ref":"toolbar","x":0,"y":400}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn edit(state: &mut EditorState, instance: &str, path: &str, edit: InstanceEdit) -> bool {
    state.apply(EditorCommand::EditInstanceDescendant {
        instance_id: NodeId::new(instance),
        path: path.into(),
        edit,
        page_id: None,
    })
}

fn node(value: serde_json::Value) -> PenNode {
    serde_json::from_value(value).expect("node")
}

fn rendered_text(state: &EditorState, id: &str) -> Option<String> {
    let resolved = resolve_refs_for_canvas(&state.doc);
    match find_node(&resolved.children, &NodeId::new(id))? {
        PenNode::Text(text) => match &text.content {
            TextContent::Plain(content) => Some(content.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn descendants(state: &EditorState, instance: &str) -> Option<Map<String, Value>> {
    match find_node(state.active_children(), &NodeId::new(instance))? {
        PenNode::Ref(reference) => reference
            .descendants
            .as_ref()
            .map(|d| d.clone().into_iter().collect()),
        _ => None,
    }
}

#[test]
fn replace_then_insert_fills_a_slot_like_pen_dev() {
    let mut state = state();
    let frame = node(json!({"type":"frame","id":"custom","layout":"vertical"}));
    assert!(edit(
        &mut state,
        "inst",
        "slot",
        InstanceEdit::Replace {
            node: Box::new(frame)
        }
    ));
    let item = node(json!({"type":"text","id":"item1","content":"Item 1"}));
    assert!(edit(
        &mut state,
        "inst",
        "custom",
        InstanceEdit::InsertChildren {
            nodes: vec![item],
            index: None,
        }
    ));
    assert_eq!(
        rendered_text(&state, "inst__item1").as_deref(),
        Some("Item 1")
    );
    let entries = descendants(&state, "inst").expect("descendants");
    assert_eq!(entries.len(), 1, "the insert lands inside the replacement");
    assert_eq!(entries["slot"]["children"][0]["id"], "item1");
}

#[test]
fn insert_into_a_master_slot_replaces_its_default_content() {
    let mut state = state();
    let items = vec![
        node(json!({"type":"text","id":"a","content":"A"})),
        node(json!({"type":"text","id":"b","content":"B"})),
    ];
    assert!(edit(
        &mut state,
        "inst",
        "slot",
        InstanceEdit::InsertChildren {
            nodes: items,
            index: None,
        }
    ));
    let first = node(json!({"type":"text","id":"z","content":"Z"}));
    assert!(edit(
        &mut state,
        "inst",
        "slot",
        InstanceEdit::InsertChildren {
            nodes: vec![first],
            index: Some(0),
        }
    ));
    let entries = descendants(&state, "inst").unwrap();
    let ids: Vec<&str> = entries["slot"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["z", "a", "b"]);
    assert_eq!(rendered_text(&state, "inst__placeholder"), None);
    assert!(
        !edit(
            &mut state,
            "inst",
            "title",
            InstanceEdit::InsertChildren {
                nodes: vec![node(json!({"type":"text","id":"t","content":"T"}))],
                index: None,
            }
        ),
        "a text node takes no children"
    );
}

#[test]
fn patch_reaches_nested_instances_and_reset_restores_the_master() {
    let mut state = state();
    assert!(edit(
        &mut state,
        "bar",
        "save/label",
        InstanceEdit::Patch {
            patch_json: r#"{"content":"Store"}"#.into()
        }
    ));
    assert_eq!(
        rendered_text(&state, "bar__save__label").as_deref(),
        Some("Store")
    );
    assert!(edit(&mut state, "bar", "save/label", InstanceEdit::Reset));
    assert_eq!(
        rendered_text(&state, "bar__save__label").as_deref(),
        Some("OK")
    );
    assert!(
        descendants(&state, "bar").is_none(),
        "an empty map is dropped"
    );
    assert!(!edit(&mut state, "bar", "save/label", InstanceEdit::Reset));
}

#[test]
fn reset_of_an_owned_node_removes_its_whole_entry() {
    let mut state = state();
    let text = node(json!({"type":"text","id":"swap","content":"New"}));
    assert!(edit(
        &mut state,
        "inst",
        "title",
        InstanceEdit::Replace {
            node: Box::new(text)
        }
    ));
    assert_eq!(rendered_text(&state, "inst__swap").as_deref(), Some("New"));
    assert!(edit(&mut state, "inst", "swap", InstanceEdit::Reset));
    assert_eq!(
        rendered_text(&state, "inst__title").as_deref(),
        Some("Hello")
    );
}

#[test]
fn unknown_paths_and_locked_instances_are_rejected() {
    let mut state = state();
    let patch = || InstanceEdit::Patch {
        patch_json: r#"{"content":"x"}"#.into(),
    };
    assert!(!edit(&mut state, "inst", "nope", patch()));
    assert!(
        !edit(&mut state, "card", "title", patch()),
        "not an instance"
    );
    if let Some(node) =
        crate::walkers::find_node_mut(state.active_children_mut(), &NodeId::new("inst"))
    {
        node.base_mut().locked = Some(true);
    }
    assert!(!edit(&mut state, "inst", "title", patch()));
}

#[test]
fn reset_of_the_selected_descendant_is_one_undo_step() {
    let mut state = state();
    let frame = node(json!({"type":"frame","id":"hero","children":[
        {"type":"text","id":"big","content":"Big"}]}));
    assert!(edit(
        &mut state,
        "inst",
        "slot",
        InstanceEdit::Replace {
            node: Box::new(frame)
        }
    ));
    state.set_single_selection(NodeId::new("inst__big"));
    assert_eq!(
        state.instance_override_at_anchor(&NodeId::new("inst__big")),
        Some((NodeId::new("inst"), "slot".to_string()))
    );
    assert_eq!(
        state.instance_override_at_anchor(&NodeId::new("inst__title")),
        None
    );
    let depth = state.history.past.len();
    assert!(state.reset_selected_instance_override());
    assert_eq!(state.history.past.len(), depth + 1);
    assert_eq!(
        rendered_text(&state, "inst__placeholder").as_deref(),
        Some("Drop here")
    );
    assert_eq!(
        state.selection.anchor.as_str(),
        "inst",
        "the reset node is gone, so the instance is selected"
    );
    assert!(state.undo());
    assert_eq!(rendered_text(&state, "inst__big").as_deref(), Some("Big"));
}
