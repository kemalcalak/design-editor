//! Planning and applying a conversion (#369).

use jian_ops_schema::node::PenNode;
use serde_json::{json, Value};

use super::tests::{page, row, state, table};
use super::*;
use crate::page_mutators::is_components_page;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::resolve_refs_for_canvas;
use crate::{walkers, EditorCommand, EditorState};

fn request(ids: &[&str]) -> ConvertRequest {
    ConvertRequest {
        node_ids: ids.iter().map(|id| NodeId::new(*id)).collect(),
        ..Default::default()
    }
}

fn three_rows() -> EditorState {
    state(json!([page(
        "p1",
        "Page",
        vec![table(
            "t",
            vec![
                row("r1", "Rent", "$900"),
                row("r2", "Food", "$300"),
                row("r3", "Fun", "$80")
            ]
        )]
    )]))
}

/// The active page as the canvas renders it, with instance ids and the
/// virtual `ref__child` ids erased, so a converted page compares with the
/// hand-built one.
fn rendered(s: &EditorState) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for key in ["id", "reusable", "x", "y", "name"] {
                    map.remove(key);
                }
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let resolved = resolve_refs_for_canvas(&s.doc);
    let mut value = serde_json::to_value(&resolved.pages.as_ref().unwrap()[0].children).unwrap();
    strip(&mut value);
    value
}

fn node<'a>(s: &'a EditorState, id: &str) -> &'a PenNode {
    s.doc
        .pages
        .iter()
        .flatten()
        .find_map(|page| walkers::find_node(&page.children, &NodeId::new(id)))
        .unwrap_or_else(|| panic!("node {id}"))
}

#[test]
fn rows_become_instances_of_a_components_page_master() {
    let mut s = three_rows();
    let before = rendered(&s);
    let plan = s
        .plan_instance_conversion(&request(&["r1", "r2", "r3"]))
        .expect("plan");
    assert_eq!(plan.converted.len(), 3);
    assert!(plan.skipped.is_empty());
    assert!(
        s.apply(EditorCommand::ConvertToInstances(Box::new(request(&[
            "r1", "r2", "r3"
        ]))))
    );

    // Each row is an instance at its slot, keeping its id and name.
    let PenNode::Frame(table) = node(&s, "t") else {
        panic!("table")
    };
    let rows = table.children.as_ref().unwrap();
    assert_eq!(rows.len(), 3);
    for (row, id) in rows.iter().zip(["r1", "r2", "r3"]) {
        let PenNode::Ref(instance) = row else {
            panic!("{id} is not an instance")
        };
        assert_eq!(instance.base.id, id);
        assert_eq!(instance.target, plan.master_id.as_str());
        assert_eq!(
            instance.base.name.as_deref(),
            Some(format!("Row {id}").as_str())
        );
    }
    // The master sits on the Components page, registered.
    let components = s
        .doc
        .pages
        .as_ref()
        .unwrap()
        .iter()
        .find(|p| is_components_page(p));
    let master = components
        .expect("Components page")
        .children
        .last()
        .unwrap();
    assert_eq!(master.id_str(), plan.master_id.as_str());
    assert!(matches!(master, PenNode::Frame(f) if f.reusable == Some(true)));
    assert!(s.components.find_by_id(&plan.master_id).is_some());
    // Rendered exactly as before; the selection is the instances.
    assert_eq!(rendered(&s), before);
    assert_eq!(s.selection.set.len(), 3);

    // One undo step.
    assert!(s.apply(EditorCommand::Undo));
    assert!(matches!(node(&s, "r2"), PenNode::Frame(_)));
    assert!(s.components.find_by_id(&plan.master_id).is_none());
}

#[test]
fn text_differences_are_descendant_overrides() {
    let s = three_rows();
    let plan = s
        .plan_instance_conversion(&request(&["r1", "r2", "r3"]))
        .expect("plan");
    let PenNode::Ref(second) = &plan.converted[1].instance else {
        panic!("instance")
    };
    let descendants = serde_json::to_value(second.descendants.as_ref().unwrap()).unwrap();
    assert_eq!(descendants["r1-name"], json!({"content": "Food"}));
    assert_eq!(descendants["r1-amount"], json!({"content": "$300"}));
    assert_eq!(plan.converted[1].overrides, 2);
    // The master source's own instance needs no override (it is in flow,
    // so the master's Components page position does not reach it).
    let PenNode::Ref(first) = &plan.converted[0].instance else {
        panic!("instance")
    };
    assert!(first.descendants.is_none(), "{:?}", first.descendants);
    assert!(plan.bytes_after < plan.bytes_before);
}

#[test]
fn in_place_keeps_the_source_as_master() {
    let mut s = three_rows();
    let mut req = request(&["r1", "r2", "r3"]);
    req.master_from = Some(NodeId::new("r2"));
    req.placement = MasterPlacement::InPlace;
    let before = rendered(&s);
    assert!(s.apply(EditorCommand::ConvertToInstances(Box::new(req))));
    assert!(matches!(node(&s, "r2"), PenNode::Frame(f) if f.reusable == Some(true)));
    assert!(matches!(node(&s, "r1"), PenNode::Ref(r) if r.target == "r2"));
    assert!(matches!(node(&s, "r3"), PenNode::Ref(r) if r.target == "r2"));
    assert!(s.components.find_by_id(&NodeId::new("r2")).is_some());
    assert!(!s.doc.pages.as_ref().unwrap().iter().any(is_components_page));
    assert_eq!(rendered(&s), before);
}

#[test]
fn a_structural_difference_is_skipped_and_reported() {
    let mut odd = row("r3", "Fun", "$80");
    odd["children"][1]["fontSize"] = json!(16);
    let s = state(json!([page(
        "p1",
        "Page",
        vec![table(
            "t",
            vec![row("r1", "A", "1"), row("r2", "B", "2"), odd]
        )]
    )]));
    let plan = s
        .plan_instance_conversion(&request(&["r1", "r2", "r3"]))
        .expect("plan");
    assert_eq!(plan.converted.len(), 2);
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.skipped[0].id.as_str(), "r3");
    assert!(
        plan.skipped[0].reason.contains("r3-amount") && plan.skipped[0].reason.contains("fontSize"),
        "{}",
        plan.skipped[0].reason
    );
    // Only differences: nothing converts, which is an error, not a no-op.
    let error = s
        .plan_instance_conversion(&request(&["r1", "r3"]))
        .unwrap_err();
    assert!(matches!(error, ConvertError::NothingToConvert(ref skipped) if skipped.len() == 1));
    assert!(error.to_string().contains("r3"), "{error}");
}

#[test]
fn free_positions_and_links_stay_on_each_instance() {
    let mut a = row("a", "A", "1");
    a["x"] = json!(10);
    a["y"] = json!(20);
    a["events"] = json!({"onTap": [{"navigate": {"to": "/a"}}]});
    let mut b = row("b", "B", "2");
    b["x"] = json!(10);
    b["y"] = json!(80);
    let mut s = state(json!([page("p1", "Page", vec![a, b])]));
    let before = rendered(&s);
    let plan = s
        .plan_instance_conversion(&request(&["a", "b"]))
        .expect("plan");
    // The master leaves out a link only one copy had.
    let master = serde_json::to_value(&plan.master).unwrap();
    assert!(master.get("events").is_none());
    assert!(
        s.apply(EditorCommand::ConvertToInstances(Box::new(request(&[
            "a", "b"
        ]))))
    );
    let PenNode::Ref(a) = node(&s, "a") else {
        panic!()
    };
    assert_eq!((a.base.x, a.base.y), (Some(10.0), Some(20.0)));
    assert!(a.events.is_some());
    let PenNode::Ref(b) = node(&s, "b") else {
        panic!()
    };
    assert_eq!((b.base.x, b.base.y), (Some(10.0), Some(80.0)));
    assert!(b.events.is_none());
    let resolved = resolve_refs_for_canvas(&s.doc);
    let page = &resolved.pages.as_ref().unwrap()[0];
    assert_eq!(page.children[0].base().x, Some(10.0));
    assert_eq!(page.children[1].base().y, Some(80.0));
    assert_eq!(rendered(&s), before);
}

#[test]
fn a_copy_linking_to_its_own_inner_node_is_skipped() {
    // The link would land on the master's node: an instance's content
    // renders under `instance__child` ids.
    let mut linked = row("r2", "B", "2");
    linked["events"] = json!({"onTap": [{"scroll_to": {"target": "r2-amount"}}]});
    let s = state(json!([page(
        "p1",
        "Page",
        vec![table(
            "t",
            vec![row("r1", "A", "1"), linked, row("r3", "C", "3")]
        )]
    )]));
    for placement in [MasterPlacement::ComponentsPage, MasterPlacement::InPlace] {
        let mut req = request(&["r1", "r2", "r3"]);
        req.placement = placement;
        let plan = s.plan_instance_conversion(&req).expect("plan");
        assert_eq!(plan.skipped.len(), 1, "{placement:?}");
        assert_eq!(plan.skipped[0].id.as_str(), "r2");
        assert!(
            plan.skipped[0]
                .reason
                .contains("inner node r2-amount is linked from inside the copy"),
            "{}",
            plan.skipped[0].reason
        );
        assert_eq!(
            plan.converted
                .iter()
                .filter(|m| m.id.as_str() != "r1")
                .count(),
            1
        );
    }
    // As the master source on the Components page it is refused.
    let mut req = request(&["r1", "r2"]);
    req.master_from = Some(NodeId::new("r2"));
    assert!(matches!(
        s.plan_instance_conversion(&req),
        Err(ConvertError::MasterUnusable { .. })
    ));
}

#[test]
fn an_inner_node_linked_from_elsewhere_is_skipped() {
    let button = json!({"type": "frame", "id": "go", "name": "Go", "width": 40, "height": 20,
        "events": {"onTap": [{"scroll_to": {"target": "r2-amount"}}]}, "children": []});
    let s = state(json!([page(
        "p1",
        "Page",
        vec![
            table(
                "t",
                vec![
                    row("r1", "A", "1"),
                    row("r2", "B", "2"),
                    row("r3", "C", "3")
                ]
            ),
            button
        ]
    )]));
    let plan = s
        .plan_instance_conversion(&request(&["r1", "r2", "r3"]))
        .expect("plan");
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(plan.skipped[0].id.as_str(), "r2");
    assert!(
        plan.skipped[0].reason.contains("go"),
        "{}",
        plan.skipped[0].reason
    );
    // The master source cannot be one.
    let mut req = request(&["r1", "r2"]);
    req.master_from = Some(NodeId::new("r2"));
    assert!(matches!(
        s.plan_instance_conversion(&req),
        Err(ConvertError::MasterUnusable { .. })
    ));
}

#[test]
fn bad_requests_are_refused_before_anything_changes() {
    let mut master = row("m", "M", "0");
    master["reusable"] = json!(true);
    let s = state(json!([
        page(
            "p1",
            "Page",
            vec![
                table("t", vec![row("r1", "A", "1"), row("r2", "B", "2")]),
                master
            ]
        ),
        page("p2", "Components", vec![row("c1", "C", "3")]),
    ]));
    let error = |ids: &[&str]| s.plan_instance_conversion(&request(ids)).unwrap_err();
    assert_eq!(error(&["r1"]), ConvertError::TooFewNodes(1));
    assert_eq!(
        error(&["r1", "r1"]),
        ConvertError::DuplicateNode("r1".into())
    );
    assert_eq!(
        error(&["r1", "zz"]),
        ConvertError::NodeNotFound("zz".into())
    );
    assert!(error(&["r1", "r1-name"])
        .to_string()
        .contains("only frames"));
    assert!(error(&["r1", "m"])
        .to_string()
        .contains("already a component master"));
    assert!(error(&["r1", "m-name"])
        .to_string()
        .contains("inside a component master"));
    assert!(error(&["r1", "c1"]).to_string().contains("Components page"));
    assert_eq!(
        error(&["t", "r1"]),
        ConvertError::NestedNodes {
            outer: "t".into(),
            inner: "r1".into()
        }
    );
    let mut req = request(&["r1", "r2"]);
    req.master_from = Some(NodeId::new("t"));
    assert_eq!(
        s.plan_instance_conversion(&req).unwrap_err(),
        ConvertError::MasterNotListed("t".into())
    );
    // A refused command changes nothing and pushes no undo step.
    let mut live = s.clone();
    let depth = live.history.past.len();
    assert!(
        !live.apply(EditorCommand::ConvertToInstances(Box::new(request(&[
            "r1", "m"
        ]))))
    );
    assert_eq!(live.history.past.len(), depth);
    assert_eq!(live.doc, s.doc);
}

#[test]
fn a_document_without_pages_gets_its_components_page() {
    let mut s = EditorState::new();
    s.doc.pages = None;
    s.doc.children = vec![
        serde_json::from_value(row("r1", "A", "1")).unwrap(),
        serde_json::from_value(row("r2", "B", "2")).unwrap(),
    ];
    let plan = s
        .convert_to_instances(&request(&["r1", "r2"]))
        .expect("converted");
    let pages = s.doc.pages.as_ref().expect("pages");
    assert!(pages.iter().any(is_components_page));
    assert!(s.components.find_by_id(&plan.master_id).is_some());
    assert!(
        matches!(&pages[0].children[0], PenNode::Ref(r) if r.target == plan.master_id.as_str())
    );
}
