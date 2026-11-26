//! Finding repeated structures (#369).

use serde_json::{json, Value};

use super::*;
use crate::{ComponentLibrary, EditorState};

/// A state whose pages are `pages` (JSON), first page active.
pub(super) fn state(pages: Value) -> EditorState {
    let mut s = EditorState::new();
    let mut doc = serde_json::to_value(&s.doc).expect("doc json");
    doc["children"] = json!([]);
    doc["pages"] = pages;
    s.doc = serde_json::from_value(doc).expect("document");
    s.components = ComponentLibrary::from_document(&s.doc);
    s
}

/// A hand-built table row: a horizontal frame with a name cell and an
/// amount cell.
pub(super) fn row(id: &str, name: &str, amount: &str) -> Value {
    json!({
        "type": "frame", "id": id, "name": format!("Row {id}"),
        "layout": "horizontal", "gap": 12, "padding": 8,
        "width": "fill_container", "height": 40,
        "fill": [{"type": "solid", "color": "#ffffff"}],
        "children": [
            {"type": "text", "id": format!("{id}-name"), "content": name,
             "fontSize": 14, "fill": [{"type": "solid", "color": "#111111"}],
             "width": 200, "height": 20},
            {"type": "text", "id": format!("{id}-amount"), "content": amount,
             "fontSize": 14, "fontWeight": "600",
             "fill": [{"type": "solid", "color": "#333333"}],
             "width": 80, "height": 20}
        ]
    })
}

pub(super) fn table(id: &str, rows: Vec<Value>) -> Value {
    json!({"type": "frame", "id": id, "name": "Table", "x": 0, "y": 0,
           "layout": "vertical", "width": 480, "height": "fit_content",
           "children": rows})
}

pub(super) fn page(id: &str, name: &str, children: Vec<Value>) -> Value {
    json!({"id": id, "name": name, "children": children})
}

fn ids(group: &RepeatedGroup) -> Vec<&str> {
    group.node_ids.iter().map(NodeId::as_str).collect()
}

#[test]
fn rows_with_different_text_form_one_group() {
    let s = state(json!([page(
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
    )]));
    let groups = find_repeated_structures(&s.doc, &s.components, None, DEFAULT_MIN_NODES);
    assert_eq!(groups.len(), 1, "{groups:?}");
    assert_eq!(ids(&groups[0]), ["r1", "r2", "r3"]);
    assert_eq!(groups[0].nodes_per_member, 3);
    assert!(groups[0].group_id.starts_with(GROUP_ID_PREFIX));
    assert_eq!(groups[0].pages, vec![Some(0); 3]);
}

#[test]
fn a_style_difference_splits_the_group() {
    let mut odd = row("r3", "Fun", "$80");
    odd["fill"] = json!([{"type": "solid", "color": "#f5f5f5"}]);
    let s = state(json!([page(
        "p1",
        "Page",
        vec![table(
            "t",
            vec![row("r1", "A", "1"), row("r2", "B", "2"), odd]
        )]
    )]));
    let groups = find_repeated_structures(&s.doc, &s.components, None, DEFAULT_MIN_NODES);
    assert_eq!(groups.len(), 1);
    assert_eq!(ids(&groups[0]), ["r1", "r2"]);
}

#[test]
fn min_nodes_filters_small_units() {
    let s = state(json!([page(
        "p1",
        "Page",
        vec![table("t", vec![row("r1", "A", "1"), row("r2", "B", "2")])]
    )]));
    assert!(find_repeated_structures(&s.doc, &s.components, None, 4).is_empty());
    assert_eq!(
        find_repeated_structures(&s.doc, &s.components, None, 3).len(),
        1
    );
}

#[test]
fn the_largest_unit_claims_its_inner_copies() {
    // Two identical tables: the tables group, not their rows.
    let s = state(json!([page(
        "p1",
        "Page",
        vec![
            table("t1", vec![row("a1", "A", "1"), row("a2", "B", "2")]),
            table("t2", vec![row("b1", "C", "3"), row("b2", "D", "4")]),
        ]
    )]));
    let groups = find_repeated_structures(&s.doc, &s.components, None, DEFAULT_MIN_NODES);
    assert_eq!(groups.len(), 1, "{groups:?}");
    assert_eq!(ids(&groups[0]), ["t1", "t2"]);
}

#[test]
fn an_inner_unit_that_also_repeats_outside_is_reported_with_those_copies() {
    let s = state(json!([page(
        "p1",
        "Page",
        vec![
            table("t1", vec![row("a1", "A", "1")]),
            table("t2", vec![row("b1", "C", "3")]),
            row("loose1", "E", "5"),
            row("loose2", "F", "6"),
        ]
    )]));
    let groups = find_repeated_structures(&s.doc, &s.components, None, DEFAULT_MIN_NODES);
    assert_eq!(groups.len(), 2, "{groups:?}");
    let rows = groups
        .iter()
        .find(|group| group.node_ids.len() == 2 && group.nodes_per_member == 3)
        .expect("row group");
    assert_eq!(ids(rows), ["loose1", "loose2"]);
}

#[test]
fn page_scope_masters_and_instances_are_left_out() {
    let mut master = row("m", "Master", "0");
    master["reusable"] = json!(true);
    let s = state(json!([
        page("p1", "Page", vec![row("r1", "A", "1")]),
        page(
            "p2",
            "Other",
            vec![
                row("r2", "B", "2"),
                json!({"type": "ref", "id": "i1", "ref": "m"})
            ]
        ),
        page(
            "p3",
            "Components",
            vec![master, row("c1", "C", "3"), row("c2", "D", "4")]
        ),
    ]));
    // Across pages (Components page excluded): r1 + r2.
    let all = find_repeated_structures(&s.doc, &s.components, None, DEFAULT_MIN_NODES);
    assert_eq!(all.len(), 1);
    assert_eq!(ids(&all[0]), ["r1", "r2"]);
    assert_eq!(all[0].pages, vec![Some(0), Some(1)]);
    // One page alone has no copy.
    assert!(find_repeated_structures(&s.doc, &s.components, Some(0), DEFAULT_MIN_NODES).is_empty());
    // The group id finds the same members.
    assert_eq!(
        group_members(&s.doc, &s.components, None, &all[0].group_id),
        Some(vec![NodeId::new("r1"), NodeId::new("r2")])
    );
    assert_eq!(group_members(&s.doc, &s.components, None, "rs-0"), None);
}

#[test]
fn a_frame_other_instances_use_as_master_is_not_a_member() {
    let s = state(json!([page(
        "p1",
        "Page",
        vec![
            row("r1", "A", "1"),
            row("r2", "B", "2"),
            row("r3", "C", "3"),
            json!({"type": "ref", "id": "i1", "ref": "r3"}),
        ]
    )]));
    let groups = find_repeated_structures(&s.doc, &s.components, None, DEFAULT_MIN_NODES);
    assert_eq!(ids(&groups[0]), ["r1", "r2"]);
}

#[test]
fn similar_layers_and_the_menu_query() {
    let mut s = state(json!([page(
        "p1",
        "Page",
        vec![table(
            "t",
            vec![
                row("r1", "A", "1"),
                row("r2", "B", "2"),
                row("r3", "C", "3")
            ]
        )]
    )]));
    assert_eq!(
        similar_layers(&s.doc, &s.components, Some(0), &NodeId::new("r2")),
        ["r1", "r2", "r3"].map(NodeId::new).to_vec()
    );
    // The right-clicked layer comes first: it becomes the master.
    assert_eq!(
        s.similar_layer_ids(&NodeId::new("r2")),
        ["r2", "r1", "r3"].map(NodeId::new).to_vec()
    );
    assert!(s.similar_layer_ids(&NodeId::new("t")).is_empty());
    assert!(s.similar_layer_ids(&NodeId::new("r1-name")).is_empty());
    // A multi-selection of copies is the set.
    s.set_single_selection(NodeId::new("r3"));
    s.toggle_selection(NodeId::new("r1"));
    assert_eq!(
        s.similar_layer_ids(&NodeId::new("r1")),
        ["r1", "r3"].map(NodeId::new).to_vec()
    );
    // A selection that mixes structures offers nothing.
    s.toggle_selection(NodeId::new("t"));
    assert!(s.similar_layer_ids(&NodeId::new("r1")).is_empty());
}
