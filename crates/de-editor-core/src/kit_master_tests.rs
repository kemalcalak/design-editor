//! UIKit components insert as linked instances of one installed master
//! (#365).

use crate::command::EditorCommand;
use crate::kit_master::kit_master_key;
use crate::node_id::NodeId;
use crate::page_mutators::COMPONENTS_PAGE_NAME;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::{instance_child_virtual_id, resolve_refs_for_canvas};
use crate::test_support::{frame, state_with};
use crate::walkers::find_node;
use crate::EditorState;
use jian_ops_schema::node::PenNode;

const SHADCN: &str = "shadcn-ui";
const BUTTON: &str = "shadcn-btn-primary";
const LABEL: &str = "shadcn-btn-primary-label";

fn kit(kit_id: &str, component: &str, parent: &str, overrides: Option<&str>) -> EditorCommand {
    EditorCommand::InstantiateKitComponent {
        kit_id: kit_id.into(),
        component_id: component.into(),
        doc_x: None,
        doc_y: None,
        target_parent: if parent.is_empty() {
            NodeId::NONE
        } else {
            NodeId::new(parent)
        },
        page_id: None,
        overrides_json: overrides.map(str::to_string),
        detach: false,
    }
}

fn screen_state() -> EditorState {
    state_with(vec![frame(
        "screen",
        "Screen",
        0.0,
        0.0,
        400.0,
        300.0,
        Vec::new(),
    )])
}

fn components_page(s: &EditorState) -> &[PenNode] {
    s.doc
        .pages
        .iter()
        .flatten()
        .find(|page| page.name == COMPONENTS_PAGE_NAME)
        .map(|page| page.children.as_slice())
        .unwrap_or_default()
}

fn screen_instances(s: &EditorState) -> Vec<(String, String)> {
    find_node(s.active_children(), &NodeId::new("screen"))
        .and_then(PenNodeExt::children)
        .into_iter()
        .flatten()
        .map(|node| match node {
            PenNode::Ref(reference) => (node.id_str().to_string(), reference.target.clone()),
            other => panic!("expected a ref, got {other:?}"),
        })
        .collect()
}

fn rendered_text(s: &EditorState, instance: &str, child: &str) -> String {
    let resolved = resolve_refs_for_canvas(&s.doc);
    let virtual_id = NodeId::new(instance_child_virtual_id(instance, child));
    let node = resolved
        .pages
        .iter()
        .flatten()
        .find_map(|page| find_node(&page.children, &virtual_id))
        .or_else(|| find_node(&resolved.children, &virtual_id))
        .expect("rendered instance child");
    let PenNode::Text(text) = node else {
        panic!("expected text, got {node:?}")
    };
    serde_json::to_value(&text.content).unwrap().as_str().unwrap().to_string()
}

fn set_text(s: &mut EditorState, id: &str, content: &str) {
    for page in s.doc.pages.as_mut().unwrap() {
        if let Some(PenNode::Text(text)) = crate::walkers::find_node_mut(&mut page.children, &NodeId::new(id)) {
            text.content = serde_json::from_value(serde_json::json!(content)).unwrap();
            return;
        }
    }
    panic!("no text {id}");
}

#[test]
fn first_kit_insert_installs_one_master_and_every_use_links_to_it() {
    let mut s = screen_state();
    assert!(s.apply(kit(SHADCN, BUTTON, "screen", Some(r#"{"label":"Save"}"#))));
    assert!(s.apply(kit(SHADCN, BUTTON, "screen", None)));

    let masters = components_page(&s);
    assert_eq!(masters.len(), 1, "installed once");
    let PenNode::Frame(master) = &masters[0] else {
        panic!("master frame")
    };
    assert_eq!(master.base.id, BUTTON, "ids kept verbatim");
    assert_eq!(master.reusable, Some(true));
    assert!(s.components.find_by_id(&NodeId::new(BUTTON)).is_some());
    assert!(s.editor_ui.kit_masters.is_empty());
    assert_eq!(s.ui.active_page_index, 0, "the design page stays active");

    let instances = screen_instances(&s);
    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|(_, target)| target == BUTTON));
    assert_eq!(s.selection.anchor.as_str(), instances[1].0);
    assert_eq!(rendered_text(&s, &instances[0].0, LABEL), "Save");
    assert_eq!(rendered_text(&s, &instances[1].0, LABEL), "Button");

    set_text(&mut s, LABEL, "Go");
    assert_eq!(rendered_text(&s, &instances[0].0, LABEL), "Save");
    assert_eq!(rendered_text(&s, &instances[1].0, LABEL), "Go");
    assert!(s.duplicate_ids().is_empty());
}

#[test]
fn installing_the_master_and_inserting_is_one_undo_step() {
    let mut s = screen_state();
    let before = s.doc.clone();
    let depth = s.history.past.len();
    assert!(s.apply(kit(SHADCN, BUTTON, "screen", None)));
    assert_eq!(s.history.past.len(), depth + 1);
    assert!(s.undo());
    assert_eq!(s.doc, before);
    assert!(s.components.find_by_id(&NodeId::new(BUTTON)).is_none());
    assert!(s.redo());
    assert_eq!(components_page(&s).len(), 1);
    assert_eq!(screen_instances(&s).len(), 1);
}

#[test]
fn colliding_kit_ids_install_under_fresh_ids_and_later_inserts_reuse_them() {
    let mut s = state_with(vec![
        frame("screen", "Screen", 0.0, 0.0, 400.0, 300.0, Vec::new()),
        frame(BUTTON, "Mine", 500.0, 0.0, 10.0, 10.0, Vec::new()),
    ]);
    let overrides = format!(r#"{{"descendants":{{"{LABEL}":{{"content":"OK"}}}}}}"#);
    assert!(s.apply(kit(SHADCN, BUTTON, "screen", Some(&overrides))));
    assert!(s.apply(kit(SHADCN, BUTTON, "screen", None)));

    let masters = components_page(&s);
    assert_eq!(masters.len(), 1);
    let master_id = masters[0].id_str().to_string();
    assert_ne!(master_id, BUTTON);
    assert_eq!(
        s.editor_ui.kit_masters.get(&kit_master_key(SHADCN, BUTTON)),
        Some(&master_id)
    );
    let label = masters[0].children().unwrap()[0].id_str().to_string();
    assert_ne!(label, LABEL);
    let instances = screen_instances(&s);
    assert!(instances.iter().all(|(_, target)| *target == master_id));
    assert_eq!(rendered_text(&s, &instances[0].0, &label), "OK");
    assert!(s.duplicate_ids().is_empty());

    // Undo both: the record is stale, so the next insert installs again.
    assert!(s.undo() && s.undo());
    assert!(components_page(&s).is_empty());
    assert!(s.apply(kit(SHADCN, BUTTON, "screen", None)));
    assert_eq!(components_page(&s).len(), 1);
    assert!(s.duplicate_ids().is_empty());
}

#[test]
fn starter_kit_ids_with_path_separators_are_reminted_so_overrides_work() {
    let mut s = screen_state();
    let overrides = r#"{"descendants":{"kit/btn-primary/label":{"content":"Buy"}}}"#;
    assert!(s.apply(kit("design-editor-starter", "btn-primary", "screen", Some(overrides))));
    let master = &components_page(&s)[0];
    assert!(!master.id_str().contains('/'));
    let label = master.children().unwrap()[0].id_str().to_string();
    let (instance, _) = &screen_instances(&s)[0];
    assert_eq!(rendered_text(&s, instance, &label), "Buy");
}

#[test]
fn detach_inserts_an_independent_copy() {
    let mut s = screen_state();
    let mut cmd = kit(SHADCN, BUTTON, "screen", None);
    if let EditorCommand::InstantiateKitComponent { detach, .. } = &mut cmd {
        *detach = true;
    }
    assert!(s.apply(cmd));
    assert!(components_page(&s).is_empty());
    let screen = find_node(s.active_children(), &NodeId::new("screen")).unwrap();
    let copy = &screen.children().unwrap()[0];
    assert!(matches!(copy, PenNode::Frame(_)));
    assert_ne!(copy.id_str(), BUTTON);
}

#[test]
fn masters_sit_side_by_side_on_the_components_page() {
    let mut s = screen_state();
    assert!(s.apply(kit(SHADCN, BUTTON, "", None)));
    assert!(s.apply(kit(SHADCN, "shadcn-btn-secondary", "", None)));
    let masters = components_page(&s);
    assert_eq!(masters.len(), 2);
    let right = masters[0].base().x.unwrap() + masters[0].width_px().unwrap();
    assert_eq!(masters[1].base().x, Some(right + 40.0));
}

#[test]
fn the_panel_insert_is_linked() {
    let mut s = screen_state();
    let id = s.instantiate_kit_component(SHADCN, BUTTON, 30.0, 40.0).unwrap();
    let instance = find_node(s.active_children(), &id).unwrap();
    assert!(matches!(instance, PenNode::Ref(_)));
    assert_eq!((instance.base().x, instance.base().y), (Some(30.0), Some(40.0)));
}
