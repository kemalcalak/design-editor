//! The kit buttons install as one Button set, variant × size (#368): the
//! old kit ids keep working, `K()` picks a variant, and a document that
//! already holds the old single master keeps using it.

use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use crate::command::EditorCommand;
use crate::component_set::{find_node, set_summary, variant_masters};
use crate::kit_master::kit_master_key;
use crate::node_id::NodeId;
use crate::page_mutators::COMPONENTS_PAGE_NAME;
use crate::pen_node_ext::PenNodeExt;
use crate::test_support::{frame, state_with};
use crate::EditorState;

const SHADCN: &str = "shadcn-ui";

fn kit(component: &str, overrides: Option<Value>) -> EditorCommand {
    EditorCommand::InstantiateKitComponent {
        kit_id: SHADCN.into(),
        component_id: component.into(),
        doc_x: None,
        doc_y: None,
        target_parent: NodeId::new("screen"),
        page_id: None,
        overrides_json: overrides.map(|o| o.to_string()),
        detach: false,
    }
}

fn screen_state(extra: Vec<PenNode>) -> EditorState {
    let mut roots = vec![frame(
        "screen",
        "Screen",
        0.0,
        0.0,
        400.0,
        300.0,
        Vec::new(),
    )];
    roots.extend(extra);
    state_with(roots)
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

fn instances(s: &EditorState) -> Vec<Value> {
    find_node(&s.doc, "screen")
        .and_then(PenNodeExt::children)
        .into_iter()
        .flatten()
        .map(|node| serde_json::to_value(node).unwrap())
        .collect()
}

#[test]
fn a_kit_button_installs_the_whole_set_once() {
    let mut s = screen_state(Vec::new());
    let depth = s.history.past.len();
    assert!(s.apply(kit("shadcn-btn-primary", Some(json!({"label": "Save"})))));
    assert_eq!(
        s.history.past.len(),
        depth + 1,
        "install and insert are one step"
    );
    assert!(s.apply(kit("shadcn-btn-ghost", None)));
    let page = components_page(&s);
    assert_eq!(page.len(), 1, "one set, installed once");
    let summary = set_summary(&s.doc, "shadcn-button").expect("set");
    assert_eq!(summary.name, "Button");
    assert_eq!(summary.axes[0].1.len(), 6);
    assert_eq!(summary.axes[1].1, ["sm", "md", "lg"]);
    assert_eq!(variant_masters(&page[0]).len(), 18);
    for id in [
        "shadcn-btn-primary",
        "shadcn-btn-ghost-sm",
        "shadcn-btn-link-lg",
    ] {
        assert!(s.components.find_by_id(&NodeId::new(id)).is_some(), "{id}");
    }
    let placed = instances(&s);
    assert_eq!(
        placed[0]["ref"], "shadcn-btn-primary",
        "the old kit id is the md variant"
    );
    assert_eq!(
        placed[0]["variant"],
        json!({"size": "md", "variant": "primary"})
    );
    assert_eq!(placed[1]["ref"], "shadcn-btn-ghost");
    assert!(s.duplicate_ids().is_empty());
}

#[test]
fn a_variant_override_picks_another_size() {
    let mut s = screen_state(Vec::new());
    let overrides = json!({"variant": {"size": "lg"},
        "descendants": {"shadcn-btn-outline-label": {"content": "Next"}}});
    assert!(s.apply(kit("shadcn-btn-outline", Some(overrides))));
    let placed = instances(&s);
    assert_eq!(placed[0]["ref"], "shadcn-btn-outline-lg");
    assert_eq!(
        placed[0]["descendants"],
        json!({"shadcn-btn-outline-label-lg": {"content": "Next"}}),
        "kit-template keys map onto the variant"
    );
    assert_eq!(placed[0]["variant"]["size"], "lg");
}

#[test]
fn taken_ids_install_the_set_fresh_and_record_every_member() {
    let mut s = screen_state(vec![frame(
        "shadcn-btn-primary",
        "Mine",
        500.0,
        0.0,
        10.0,
        10.0,
        Vec::new(),
    )]);
    assert!(s.apply(kit("shadcn-btn-primary", None)));
    assert!(s.apply(kit("shadcn-btn-secondary", None)));
    assert_eq!(
        components_page(&s).len(),
        1,
        "the record finds the set again"
    );
    let primary = s
        .editor_ui
        .kit_masters
        .get(&kit_master_key(SHADCN, "shadcn-btn-primary"))
        .expect("recorded")
        .clone();
    assert_ne!(primary, "shadcn-btn-primary");
    assert!(s
        .editor_ui
        .kit_masters
        .contains_key(&kit_master_key(SHADCN, "shadcn-btn-link")));
    assert_eq!(instances(&s)[0]["ref"], primary.as_str());
    assert!(s.duplicate_ids().is_empty());
}

#[test]
fn an_older_single_master_keeps_serving_its_kit_id() {
    // A document from before the set: the master sits alone on the
    // Components page.
    let mut s = screen_state(Vec::new());
    let kit_template = s
        .ui_kits
        .iter()
        .find(|k| k.id == SHADCN)
        .and_then(|k| k.components.iter().find(|c| c.id == "shadcn-btn-primary"))
        .unwrap()
        .template
        .clone();
    let mut alloc = crate::id_allocator::SequentialIdAllocator::new(1000);
    let mut master = kit_template;
    if let PenNode::Frame(frame) = &mut master {
        frame.reusable = Some(true);
    }
    s.append_components_page_masters_with_allocator(vec![master], &mut alloc)
        .unwrap();
    assert!(s.apply(kit("shadcn-btn-primary", None)));
    assert_eq!(components_page(&s).len(), 1);
    assert!(set_summary(&s.doc, "shadcn-button").is_none());
    let placed = instances(&s);
    assert_eq!(placed[0]["ref"], "shadcn-btn-primary");
    assert!(placed[0].get("variant").is_none());
}
