use super::*;
use crate::id_allocator::{collect_document_ids, SequentialIdAllocator};
use crate::instance_edit::InstanceEdit;
use crate::{EditorCommand, EditorState};
use serde_json::json;

fn doc(value: Value) -> jian_ops_schema::PenDocument {
    jian_ops_schema::load_str(&value.to_string())
        .expect("fixture parses")
        .value
}

/// A card component with a title and an empty slot, a button component,
/// and an instance of the card whose slot holds a frame with a text and a
/// nested button instance that replaces its label.
fn state_with_slot_content() -> EditorState {
    EditorState::from_document(doc(json!({"version":"1.0.0","children":[
        {"type":"frame","id":"n1","name":"Card","reusable":true,"width":200,"height":120,
         "children":[
            {"type":"text","id":"n2","name":"Title","content":"Hello"},
            {"type":"frame","id":"n3","name":"Slot","width":200,"height":80}]},
        {"type":"frame","id":"n4","name":"Button","reusable":true,"width":80,"height":30,
         "children":[{"type":"text","id":"n5","name":"Label","content":"OK"}]},
        {"type":"ref","id":"n6","ref":"n1","x":300,"descendants":{
            "n3":{"children":[
                {"type":"frame","id":"n50","name":"Body","children":[
                    {"type":"text","id":"n51","content":"Inside"},
                    {"type":"ref","id":"n52","ref":"n4","descendants":{
                        "n5":{"type":"text","id":"n53","content":"Go"}}}]}]},
            "n2":{"type":"text","content":"Kept slot id"}}}
    ]})))
}

#[test]
fn id_walks_see_instance_content_at_any_depth() {
    let state = state_with_slot_content();
    assert_eq!(state.max_node_id(), 53);
    assert_eq!(state.next_node_id_seed(), Some(54));
    let ids = state.collect_node_ids();
    for id in ["n50", "n51", "n52", "n53"] {
        assert!(ids.contains(&NodeId::new(id)), "{id} is collected");
    }
    assert_eq!(collect_document_ids(&state.doc), ids);
    let allocator = SequentialIdAllocator::for_document(&state.doc, 1).expect("allocator");
    assert_eq!(allocator.next_counter(), 54);
}

#[test]
fn a_replacement_root_that_keeps_the_slot_id_is_no_node_of_its_own() {
    let state = state_with_slot_content();
    let PenNode::Ref(reference) = &state.doc.children[2] else {
        panic!("instance");
    };
    let mut own = Vec::new();
    for_each_owned_node(
        reference.descendants.as_ref().expect("map"),
        &mut |node, keeps| {
            own.push((json_node_id(node).map(str::to_owned), keeps));
        },
    );
    assert!(own.contains(&(None, true)), "{own:?}");
    assert!(own.contains(&(Some("n53".into()), false)), "{own:?}");
}

fn insert_text(id: &str) -> PenNode {
    serde_json::from_value(json!({"type":"text","id":id,"content":"x"})).expect("node")
}

#[test]
fn ids_stay_unique_after_insert_and_replace_inside_an_instance() {
    let mut state = state_with_slot_content();
    // `batch_design` mints from the seed it reads off the document.
    let first = format!("n{}", state.next_node_id_seed().expect("seed"));
    assert!(state.apply(EditorCommand::EditInstanceDescendant {
        instance_id: NodeId::new("n6"),
        path: "n50".into(),
        edit: InstanceEdit::InsertChildren {
            nodes: vec![insert_text(&first)],
            index: None,
        },
        page_id: None,
    }));
    let second = format!("n{}", state.next_node_id_seed().expect("seed"));
    assert_ne!(first, second, "the insert advanced the allocator");
    assert!(state.apply(EditorCommand::EditInstanceDescendant {
        instance_id: NodeId::new("n6"),
        path: "n51".into(),
        edit: InstanceEdit::Replace {
            node: Box::new(insert_text(&second)),
        },
        page_id: None,
    }));
    let third = format!("n{}", state.next_node_id_seed().expect("seed"));
    assert!(![first.as_str(), second.as_str()].contains(&third.as_str()));
    let mut allocator = SequentialIdAllocator::for_document(&state.doc, 1).expect("allocator");
    for _ in 0..3 {
        assert!(state
            .apply_with_allocator(
                EditorCommand::InsertNode {
                    kind: "rect".into(),
                    name: "New".into(),
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                    fill_hex: None,
                    target_parent: NodeId::NONE,
                    page_id: None,
                },
                &mut allocator,
            )
            .expect("allocates"));
    }
    assert!(
        state.duplicate_ids().is_empty(),
        "{:?}",
        state.duplicate_ids()
    );
}

#[test]
fn a_copied_instance_gets_fresh_ids_for_its_own_content() {
    let mut state = state_with_slot_content();
    let PenNode::Ref(reference) = &mut state.doc.children[2] else {
        panic!("instance");
    };
    // A key that addresses a node inside the nested instance of the slot.
    reference.descendants.as_mut().expect("map").insert(
        "n52/n5".into(),
        json!({"fill":[{"type":"solid","color":"#ff0000"}]}),
    );
    state.set_single_selection(NodeId::new("n6"));
    let mut next = state.next_node_id_seed().expect("seed");
    let copy = state
        .duplicate_selected(&mut next, 10.0)
        .expect("duplicated");
    assert!(
        state.duplicate_ids().is_empty(),
        "{:?}",
        state.duplicate_ids()
    );
    let Some(PenNode::Ref(copy)) = crate::walkers::find_node(&state.doc.children, &copy) else {
        panic!("copy is an instance");
    };
    let descendants = copy.descendants.as_ref().expect("map");
    assert!(descendants.contains_key("n2"), "master keys stay");
    assert!(descendants.contains_key("n3"), "master keys stay");
    assert!(!descendants.contains_key("n52/n5"), "{descendants:?}");
    let nested = descendants["n3"].pointer("/children/0/children/1/id");
    let nested = nested.and_then(Value::as_str).expect("nested id");
    assert_ne!(nested, "n52");
    assert!(
        descendants.contains_key(&format!("{nested}/n5")),
        "{descendants:?}"
    );
}

#[test]
fn both_walks_visit_instance_content_in_the_same_order() {
    let state = state_with_slot_content();
    let PenNode::Ref(reference) = &state.doc.children[2] else {
        panic!("instance");
    };
    let mut descendants = reference.descendants.clone().expect("map");
    let mut shared = Vec::new();
    for_each_owned_node(&descendants, &mut |node, keeps| {
        shared.push((json_node_id(node).map(str::to_owned), keeps));
    });
    let mut exclusive = Vec::new();
    for_each_owned_node_mut(descendants.iter_mut(), &mut |node, keeps| {
        exclusive.push((json_node_id(node).map(str::to_owned), keeps));
    });
    assert_eq!(shared, exclusive);
    assert_eq!(shared.len(), 5, "{shared:?}");
}
