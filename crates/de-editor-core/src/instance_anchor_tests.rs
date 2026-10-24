use super::*;
use crate::state::EditorState;
use crate::walkers::find_node;
use serde_json::json;

fn doc(instances: Vec<Value>) -> PenDocument {
    let mut children = vec![
        json!({"type":"frame","id":"card","name":"Card","reusable":true,
               "width":200,"height":120,"children":[
            {"type":"text","id":"title","content":"Hello"},
            {"type":"frame","id":"slot","name":"Content","children":[
                {"type":"text","id":"placeholder","content":"Drop here"}]}]}),
        json!({"type":"frame","id":"button","name":"Button","reusable":true,
               "width":120,"height":40,"children":[
            {"type":"text","id":"label","content":"OK"}]}),
        json!({"type":"frame","id":"toolbar","name":"Toolbar","reusable":true,
               "width":300,"height":48,"children":[
            {"type":"ref","id":"save","ref":"button"}]}),
    ];
    children.extend(instances);
    jian_ops_schema::load_str(&json!({"version":"1.0.0","children":children}).to_string())
        .expect("fixture parses")
        .value
}

fn anchor(doc: &PenDocument, id: &str) -> Option<InstanceDescendant> {
    locate_anchor(doc, &NodeId::new(id))
}

fn patch(key: &str) -> DescendantTarget {
    DescendantTarget::Patch { key: key.into() }
}

fn owned(entry_key: &str, node_id: &str) -> DescendantTarget {
    DescendantTarget::Owned {
        entry_key: entry_key.into(),
        node_id: node_id.into(),
    }
}

#[test]
fn master_children_and_nested_leaves_resolve_to_patch_keys() {
    let doc = doc(vec![json!({"type":"ref","id":"bar","ref":"toolbar"})]);
    assert_eq!(anchor(&doc, "bar__save").unwrap().target, patch("save"));
    let leaf = anchor(&doc, "bar__save__label").expect("nested leaf");
    assert_eq!(leaf.ref_id.as_str(), "bar");
    assert_eq!(leaf.target, patch("save/label"));
    assert!(matches!(leaf.display, PenNode::Text(_)));
    assert!(
        anchor(&doc, "bar__label").is_none(),
        "no flat id into a nested instance"
    );
    let nested = anchor(&doc, "bar__save").unwrap();
    assert!(
        matches!(nested.display, PenNode::Frame(_)),
        "a nested instance shows its merged component root"
    );
}

#[test]
fn replacement_and_slot_content_resolve_as_owned() {
    let doc = doc(vec![
        json!({"type":"ref","id":"inst","ref":"card","descendants":{
        "title":{"type":"frame","id":"hero","children":[{"type":"text","id":"big","content":"Hi"}]},
        "slot":{"children":[{"type":"text","id":"mine","content":"Mine"}]}}}),
    ]);
    assert_eq!(
        anchor(&doc, "inst__hero").unwrap().target,
        owned("title", "hero")
    );
    assert_eq!(
        anchor(&doc, "inst__big").unwrap().target,
        owned("title", "big")
    );
    assert_eq!(anchor(&doc, "inst__slot").unwrap().target, patch("slot"));
    assert_eq!(
        anchor(&doc, "inst__mine").unwrap().target,
        owned("slot", "mine")
    );
    assert!(anchor(&doc, "inst__placeholder").is_none());
    assert!(
        anchor(&doc, "inst__title").is_none(),
        "the replaced node is gone"
    );

    let ref_node = find_node(&doc.children, &NodeId::new("inst")).unwrap();
    assert_eq!(
        locate_path(&doc, ref_node, "big").unwrap().target,
        owned("title", "big")
    );
    assert_eq!(
        locate_path(&doc, ref_node, "title").unwrap().target,
        owned("title", "hero"),
        "a replacement root also answers to the key it replaced"
    );
}

#[test]
fn document_paths_walk_frames_then_the_instance() {
    let doc = doc(vec![json!({"type":"frame","id":"page","children":[
        {"type":"ref","id":"bar","ref":"toolbar"}]})]);
    let hit = locate_document_path(&doc.children, &doc, "page/bar/save/label").expect("hit");
    assert_eq!(hit.ref_id.as_str(), "bar");
    assert_eq!(hit.target, patch("save/label"));
    let hit = locate_document_path(&doc.children, &doc, "bar/save").expect("deep first segment");
    assert_eq!(hit.anchor.as_str(), "bar__save");
    assert!(locate_document_path(&doc.children, &doc, "page/bar").is_none());
    assert!(locate_document_path(&doc.children, &doc, "bar/nope").is_none());
}

#[test]
fn panel_write_on_owned_node_lands_in_the_entry() {
    let doc = doc(vec![
        json!({"type":"ref","id":"inst","ref":"card","descendants":{
        "slot":{"children":[{"type":"text","id":"mine","content":"Mine"}]}}}),
    ]);
    let mut state = EditorState::from_document(doc);
    state.set_single_selection(NodeId::new("inst__mine"));
    assert!(state.set_selected_color(true, "#ff0000"));
    let Some(PenNode::Ref(reference)) = find_node(state.active_children(), &NodeId::new("inst"))
    else {
        panic!("inst stays a Ref");
    };
    let descendants = reference.descendants.as_ref().expect("descendants");
    assert_eq!(descendants.len(), 1, "no extra patch entry");
    assert_eq!(
        descendants["slot"].pointer("/children/0/fill/0/color"),
        Some(&json!("#ff0000"))
    );
}

#[test]
fn panel_write_on_nested_leaf_writes_a_path_key() {
    let doc = doc(vec![json!({"type":"ref","id":"bar","ref":"toolbar"})]);
    let mut state = EditorState::from_document(doc);
    state.set_single_selection(NodeId::new("bar__save__label"));
    assert!(state.set_selected_color(true, "#00ff00"));
    let Some(PenNode::Ref(reference)) = find_node(state.active_children(), &NodeId::new("bar"))
    else {
        panic!("bar stays a Ref");
    };
    let entry = &reference.descendants.as_ref().expect("descendants")["save/label"];
    assert_eq!(entry.pointer("/fill/0/color"), Some(&json!("#00ff00")));
    let resolved = crate::ref_resolve::resolve_refs_for_canvas(&state.doc);
    let label = find_node(&resolved.children, &NodeId::new("bar__save__label")).unwrap();
    assert_eq!(crate::fills::first_solid_fill_hex(label), Some("#00ff00"));
}
