//! The property panel's component-property rows and writes (#367).

use jian_ops_schema::component_props::{BindingField, ComponentPropertyType};
use serde_json::{json, Value};

use super::tests::state;
use super::PanelProperties;
use crate::command::EditorCommand;
use crate::{EditorState, NodeId};

fn select(s: &mut EditorState, page: u32, id: &str) {
    assert!(s.apply(EditorCommand::SetActivePage { index: page }) || page == 0);
    s.set_single_selection(NodeId::new(id));
}

fn props_of(s: &EditorState, id: &str) -> Value {
    let node = crate::component_backing::find_node_in_document(&s.doc, id).unwrap();
    serde_json::to_value(node).unwrap()["props"].clone()
}

fn names(panel: &PanelProperties) -> Vec<String> {
    match panel {
        PanelProperties::Instance(rows) | PanelProperties::Master(rows) => {
            rows.iter().map(|row| row.name.clone()).collect()
        }
        other => panic!("rows expected, got {other:?}"),
    }
}

#[test]
fn an_instance_lists_its_masters_properties_with_values() {
    let mut s = state();
    select(&mut s, 0, "b1");
    let panel = s.panel_properties();
    assert!(matches!(panel, PanelProperties::Instance(_)));
    assert_eq!(
        names(&panel),
        ["icon", "iconTint", "label", "showIcon", "tone"]
    );
    let label = panel.row(2).unwrap();
    assert_eq!((label.display(), label.set), ("Save".to_string(), true));
    let tone = panel.row(4).unwrap();
    assert_eq!((tone.display(), tone.set), ("#111111".to_string(), false));
    assert_eq!(panel.row(0).unwrap().choices, ["icon-arrow", "icon-plus"]);
}

#[test]
fn instance_inputs_write_props() {
    let mut s = state();
    select(&mut s, 0, "b1");
    assert!(s.toggle_selected_instance_prop(3), "showIcon false -> true");
    assert!(s.cycle_selected_instance_prop(0), "icon-arrow -> icon-plus");
    assert!(s.commit_selected_prop_text(2, " Hello "));
    assert!(s.commit_selected_prop_text(4, "#ff00ff"));
    assert_eq!(
        props_of(&s, "b1"),
        json!({"label": "Hello", "showIcon": true, "icon": "icon-plus", "tone": "#ff00ff"})
    );
    assert!(
        !s.commit_selected_prop_text(4, "nope"),
        "an invalid colour is refused"
    );
    assert!(
        s.commit_selected_prop_text(2, ""),
        "blank goes back to the default"
    );
    assert!(props_of(&s, "b1").get("label").is_none());
    assert!(
        !s.toggle_selected_instance_prop(2),
        "label is not a boolean"
    );
}

#[test]
fn a_master_renames_and_removes_from_the_panel() {
    let mut s = state();
    select(&mut s, 2, "btn");
    let panel = s.panel_properties();
    assert!(matches!(panel, PanelProperties::Master(_)));
    assert!(s.commit_selected_prop_text(2, "caption"));
    assert_eq!(props_of(&s, "b1")["caption"], "Save");
    let panel = s.panel_properties();
    let index = names(&panel).iter().position(|n| n == "caption").unwrap();
    assert!(s.remove_selected_component_property(index));
    let b1 = crate::component_backing::find_node_in_document(&s.doc, "b1").unwrap();
    let b1 = serde_json::to_value(b1).unwrap();
    assert_eq!(b1["descendants"]["btn-label"]["content"], "Save");
    assert!(s.apply(EditorCommand::Undo));
    assert_eq!(props_of(&s, "b1")["caption"], "Save", "one undo step");
}

#[test]
fn a_node_inside_a_master_offers_its_unbound_fields() {
    let mut s = state();
    select(&mut s, 2, "btn-label");
    assert_eq!(
        s.panel_properties(),
        PanelProperties::Bindable(vec![BindingField::Visible]),
        "content is bound by label already"
    );
    select(&mut s, 2, "plus-shape");
    assert_eq!(
        s.panel_properties(),
        PanelProperties::Bindable(vec![BindingField::Visible])
    );
    assert!(s.bind_selected_to_new_property(BindingField::Visible));
    select(&mut s, 2, "icon-plus");
    let panel = s.panel_properties();
    assert_eq!(names(&panel), ["Show rectangle"]);
    assert_eq!(panel.row(0).unwrap().kind, ComponentPropertyType::Boolean);
    assert_eq!(panel.row(0).unwrap().value, json!(true));
    // A node outside any master offers nothing.
    select(&mut s, 0, "screen");
    assert_eq!(s.panel_properties(), PanelProperties::None);
}
