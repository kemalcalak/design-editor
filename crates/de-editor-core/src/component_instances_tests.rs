//! Linked component instances (#365): insert, cycle refusal, detach,
//! swap and list, with masters on another page.

use crate::command::EditorCommand;
use crate::instance_command::InstanceCommand;
use crate::instance_error::InstanceError;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::{instance_child_virtual_id, resolve_refs_for_canvas};
use crate::test_support::{frame, state_with, text};
use crate::walkers::find_node;
use crate::EditorState;
use jian_ops_schema::node::PenNode;

fn id(s: &str) -> NodeId {
    NodeId::new(s)
}

/// Page 1 holds `screen` (a frame); page 2 holds the master `btn`
/// (`btn-label` inside). Page 1 is active.
fn two_page_state() -> EditorState {
    let mut s = state_with(vec![frame(
        "screen",
        "Screen",
        0.0,
        0.0,
        400.0,
        300.0,
        Vec::new(),
    )]);
    let label = text("btn-label", "Label", 8.0, 8.0, 80.0, 20.0, "Button");
    let master = frame("btn", "Button", 0.0, 0.0, 120.0, 40.0, vec![label]);
    // Pushed as-is: page and subtree inserts re-mint authored ids.
    let page = s.add_page_with_name(Some("Kit".into())).expect("second page");
    assert_eq!(page, 1);
    assert!(s.apply(EditorCommand::SetActivePage { index: 1 }));
    s.active_children_mut().clear();
    s.active_children_mut().push(master);
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("btn"),
        name: "Button".into(),
    }));
    assert!(s.apply(EditorCommand::SetActivePage { index: 0 }));
    s
}

fn insert(component: &str, parent: &str, descendants: Option<&str>) -> EditorCommand {
    EditorCommand::Instance(InstanceCommand::Insert {
        component_id: id(component),
        parent: if parent.is_empty() {
            NodeId::NONE
        } else {
            id(parent)
        },
        x: None,
        y: None,
        descendants_json: descendants.map(str::to_string),
        page_id: None,
    })
}

/// The content an instance's copy of `child` renders.
fn rendered_text(s: &EditorState, instance: &str, child: &str) -> String {
    let resolved = resolve_refs_for_canvas(&s.doc);
    let virtual_id = id(&instance_child_virtual_id(instance, child));
    let node = resolved
        .pages
        .iter()
        .flatten()
        .find_map(|page| find_node(&page.children, &virtual_id))
        .expect("rendered instance child");
    match node {
        PenNode::Text(text) => serde_json::to_value(&text.content)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default(),
        other => panic!("expected text, got {other:?}"),
    }
}

fn set_master_label(s: &mut EditorState, content: &str) {
    let master = s.doc.pages.as_mut().unwrap()[1]
        .children
        .iter_mut()
        .find(|node| node.id_str() == "btn")
        .unwrap();
    let PenNode::Frame(frame) = master else {
        panic!("master frame")
    };
    let label = &mut frame.children.as_mut().unwrap()[0];
    let PenNode::Text(label) = label else {
        panic!("label text")
    };
    label.content = serde_json::from_value(serde_json::json!(content)).unwrap();
}

#[test]
fn instance_of_a_master_on_another_page_is_a_ref_that_follows_the_master() {
    let mut s = two_page_state();
    assert!(s.apply(insert("btn", "screen", Some(r#"{"btn-label":{"content":"Save"}}"#))));
    assert!(s.apply(insert("btn", "screen", None)));
    let screen = find_node(s.active_children(), &id("screen")).unwrap();
    let instances: Vec<String> = screen
        .children()
        .unwrap()
        .iter()
        .map(|node| {
            let PenNode::Ref(reference) = node else {
                panic!("expected a ref, got {node:?}")
            };
            assert_eq!(reference.target, "btn");
            node.id_str().to_string()
        })
        .collect();
    assert_eq!(instances.len(), 2);
    assert_eq!(s.selection.anchor.as_str(), instances[1]);
    assert_eq!(rendered_text(&s, &instances[0], "btn-label"), "Save");
    assert_eq!(rendered_text(&s, &instances[1], "btn-label"), "Button");

    set_master_label(&mut s, "Go");
    assert_eq!(rendered_text(&s, &instances[0], "btn-label"), "Save");
    assert_eq!(rendered_text(&s, &instances[1], "btn-label"), "Go");
    assert!(s.duplicate_ids().is_empty());
}

#[test]
fn page_root_instance_defaults_next_to_the_master_origin_and_undoes_in_one_step() {
    let mut s = two_page_state();
    let depth = s.history.past.len();
    assert!(s.apply(insert("btn", "", None)));
    let instance = s.active_children().last().unwrap();
    assert!(matches!(instance, PenNode::Ref(_)));
    assert_eq!((instance.base().x, instance.base().y), (Some(20.0), Some(20.0)));
    assert_eq!(s.history.past.len(), depth + 1);
    assert!(s.undo());
    assert_eq!(s.active_children().len(), 1);
}

#[test]
fn an_instance_cannot_go_inside_its_own_master() {
    let mut s = two_page_state();
    let page = s.instance_view().on_page(1);
    assert!(matches!(
        page.check_instance_insert(&id("btn"), &id("btn")),
        Err(InstanceError::Cycle { .. })
    ));
    assert!(s.apply(EditorCommand::SetActivePage { index: 1 }));
    assert!(!s.apply(insert("btn", "btn", None)));
    assert!(!s.apply(insert("btn", "btn-label", None)));

    // Indirect: card shows btn; btn may not then hold a card instance.
    s.active_children_mut().push(frame("card", "Card", 200.0, 0.0, 200.0, 100.0, Vec::new()));
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("card"),
        name: "Card".into(),
    }));
    assert!(s.apply(insert("btn", "card", None)));
    assert!(matches!(
        s.instance_view().check_instance_insert(&id("card"), &id("btn")),
        Err(InstanceError::Cycle { .. })
    ));
}

#[test]
fn unknown_or_plain_nodes_are_not_instantiable() {
    let s = two_page_state();
    let view = s.instance_view();
    assert_eq!(
        view.check_instance_insert(&id("nope"), &NodeId::NONE).unwrap_err(),
        InstanceError::ComponentNotFound("nope".into())
    );
    assert_eq!(
        view.check_instance_insert(&id("screen"), &NodeId::NONE).unwrap_err(),
        InstanceError::NotAComponent("screen".into())
    );
}

#[test]
fn detach_keeps_the_instance_id_and_gives_inner_nodes_fresh_ids() {
    let mut s = two_page_state();
    assert!(s.apply(insert("btn", "screen", Some(r#"{"btn-label":{"content":"Save"}}"#))));
    let instance = s.selection.anchor.clone();
    assert!(s.apply(EditorCommand::SetActivePage { index: 1 }));
    // Detach finds the instance on page 1 while page 2 is active.
    assert!(s.apply(EditorCommand::Instance(InstanceCommand::Detach {
        node_id: instance.clone(),
        page_id: None,
    })));
    assert_eq!(s.ui.active_page_index, 1);
    let page = &s.doc.pages.as_ref().unwrap()[0].children;
    let detached = find_node(page, &instance).expect("same id");
    let PenNode::Frame(frame) = detached else {
        panic!("detached into a frame, got {detached:?}")
    };
    assert_ne!(frame.reusable, Some(true));
    let label = &frame.children.as_ref().unwrap()[0];
    assert_ne!(label.id_str(), "btn-label");
    let PenNode::Text(label) = label else {
        panic!("label")
    };
    assert_eq!(
        serde_json::to_value(&label.content).unwrap(),
        serde_json::json!("Save")
    );
    assert!(s.duplicate_ids().is_empty());
}

#[test]
fn swap_keeps_overrides_and_list_instances_reports_each_use() {
    let mut s = two_page_state();
    assert!(s.apply(insert("btn", "screen", Some(r#"{"btn-label":{"content":"Save"}}"#))));
    let first = s.selection.anchor.clone();
    assert!(s.apply(insert("btn", "", None)));
    let uses = s.instance_view().list_instances("btn");
    assert_eq!(uses.len(), 2);
    assert_eq!(uses[0].instance_id, first.as_str());
    assert_eq!(uses[0].override_count, 1);
    assert_eq!(uses[1].override_count, 0);

    assert!(s.apply(EditorCommand::SetActivePage { index: 1 }));
    s.active_children_mut().push(frame("alt", "Alt", 200.0, 0.0, 100.0, 40.0, Vec::new()));
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("alt"),
        name: "Alt".into(),
    }));
    assert!(s.apply(EditorCommand::Instance(InstanceCommand::Swap {
        node_id: first.clone(),
        component_id: id("alt"),
        page_id: None,
    })));
    let page = &s.doc.pages.as_ref().unwrap()[0].children;
    let PenNode::Ref(reference) = find_node(page, &first).unwrap() else {
        panic!("still a ref")
    };
    assert_eq!(reference.target, "alt");
    assert!(reference.descendants.as_ref().unwrap().contains_key("btn-label"));
    assert_eq!(s.instance_view().list_instances("btn").len(), 1);
    assert_eq!(s.instance_view().list_instances("alt").len(), 1);
}

#[test]
fn override_targets_name_the_masters_nodes() {
    let s = two_page_state();
    let targets = s.instance_view().override_targets("btn");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, "btn-label");
    assert_eq!(targets[0].name.as_deref(), Some("Label"));
    assert_eq!(targets[0].kind, "text");
}

#[test]
fn copy_keeps_the_old_detached_clone() {
    let mut s = two_page_state();
    assert!(s.apply(EditorCommand::Instance(InstanceCommand::Copy {
        component_id: id("btn"),
    })));
    let clone = s.active_children().last().unwrap();
    assert!(matches!(clone, PenNode::Frame(_)));
    assert_ne!(clone.id_str(), "btn");
}
