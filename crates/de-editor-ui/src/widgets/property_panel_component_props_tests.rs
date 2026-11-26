//! Component-property rows in the property panel (#367): an instance's
//! inputs above its buttons, a master's rename / remove rows, the
//! "… property" offers on a node inside a master, and their dispatch.

use super::property_panel::{PropertyPanel, PropertyPanelAction};
use super::property_panel_component_props::{self as props, BindOffer, PropRowsLayout};
use super::property_panel_test_support::{state_from, CountingBackend};
use crate::widgets::property_panel_dispatch::apply_instance_lifecycle_action;
use crate::widgets::PaintCx;
use crate::widgets::Widget;
use crate::{Point2D, Rect};
use de_editor_core::{EditorState, NodeId, PropertyFocus};

const DOC: &str = r##"{
  "version":"1.0.0",
  "children":[
    {"type":"frame","id":"icon-a","name":"Arrow","reusable":true,"width":16,"height":16},
    {"type":"frame","id":"icon-b","name":"Plus","reusable":true,"width":16,"height":16},
    {"type":"frame","id":"btn","name":"Button","reusable":true,"width":120,"height":40,
     "componentProperties":{
       "label":{"type":"text","bind":[{"node":"btn-label","field":"content"}]},
       "showIcon":{"type":"boolean","bind":[{"node":"btn-icon","field":"visible"}]},
       "icon":{"type":"instance","preferred":["icon-a","icon-b"],
               "bind":[{"node":"btn-icon","field":"ref"}]}},
     "children":[
       {"type":"rectangle","id":"btn-bg","name":"Background","width":120,"height":40,
        "fill":[{"type":"solid","color":"#111111"}]},
       {"type":"text","id":"btn-label","name":"Label","content":"Button"},
       {"type":"ref","id":"btn-icon","ref":"icon-a"}]},
    {"type":"ref","id":"i1","ref":"btn","x":300,"y":0,"props":{"label":"Save"}}
  ]
}"##;

const PANEL: Rect = Rect {
    origin: Point2D { x: 0.0, y: 0.0 },
    size: Point2D {
        x: 280.0,
        y: 1400.0,
    },
};

fn select(state: &mut EditorState, id: &str) -> PropertyPanel {
    state.set_single_selection(NodeId::new(id));
    PropertyPanel::for_selection(state).expect("panel builds")
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

fn rows_top(panel: &PropertyPanel) -> f32 {
    props::rows_top(
        panel.visible_sections().component_button,
        super::property_panel_inputs::PANEL_HEADER_HEIGHT,
    )
}

fn row_center(panel: &PropertyPanel, index: usize, right: bool) -> Point2D {
    let row = props::row_rect(0.0, rows_top(panel), PANEL.size.x, index);
    let x = if right {
        row.origin.x + row.size.x - 12.0
    } else {
        row.origin.x + 8.0
    };
    Point2D::new(x, center(row).y)
}

fn paint_texts(panel: &PropertyPanel) -> Vec<String> {
    let mut backend = CountingBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        PANEL,
    );
    backend.texts
}

#[test]
fn an_instance_shows_its_properties_above_its_buttons() {
    let mut state = state_from(DOC);
    let panel = select(&mut state, "i1");
    let visible = panel.visible_sections();
    // icon, label, showIcon (name order).
    assert_eq!(
        visible.component_props,
        PropRowsLayout::Instance {
            count: 3,
            text: 0b010,
            toggle: 0b100
        }
    );
    assert_eq!(
        panel.hit_test(PANEL, row_center(&panel, 1, true)),
        Some(PropertyFocus::ComponentProp(1))
    );
    assert_eq!(
        panel.hit_test_action(PANEL, row_center(&panel, 2, true)),
        Some(PropertyPanelAction::ToggleInstanceProp(2))
    );
    assert_eq!(
        panel.hit_test_action(PANEL, row_center(&panel, 0, true)),
        Some(PropertyPanelAction::CycleInstanceProp(0))
    );
    // The instance buttons moved down by the rows.
    let buttons = super::property_panel_instance::block_rects(
        0.0,
        props::buttons_y(
            visible.component_button,
            visible.component_props,
            super::property_panel_inputs::PANEL_HEADER_HEIGHT,
        ),
        PANEL.size.x,
        visible.component_button,
    );
    assert_eq!(
        panel.hit_test_action(PANEL, center(buttons.go_to_component.unwrap())),
        Some(PropertyPanelAction::GoToComponent)
    );
    let texts = paint_texts(&panel);
    assert!(texts.iter().any(|text| text == "Save"), "{texts:?}");
    assert!(texts.iter().any(|text| text == "showIcon"));
    assert!(texts.iter().any(|text| text == "Arrow"));
}

#[test]
fn instance_controls_dispatch_to_props() {
    let mut state = state_from(DOC);
    select(&mut state, "i1");
    apply_instance_lifecycle_action(&mut state, &PropertyPanelAction::ToggleInstanceProp(2));
    apply_instance_lifecycle_action(&mut state, &PropertyPanelAction::CycleInstanceProp(0));
    let node = state.selected_node().unwrap();
    let props = serde_json::to_value(node).unwrap()["props"].clone();
    assert_eq!(
        props,
        serde_json::json!({"label": "Save", "showIcon": false, "icon": "icon-b"})
    );
    assert!(state.apply(de_editor_core::EditorCommand::Undo));
}

#[test]
fn typed_values_commit_on_the_real_instance() {
    let mut state = state_from(DOC);
    let panel = select(&mut state, "i1");
    assert_eq!(
        super::property_panel_commit::property_focus_initial(
            PropertyFocus::ComponentProp(1),
            &panel
        ),
        "Save"
    );
    state.ui.property_focus = Some(PropertyFocus::ComponentProp(1));
    state.ui.property_input.set_text("Send");
    let depth = state.history.past.len();
    assert!(super::property_panel_commit::commit_property_focus(
        &mut state
    ));
    let node = state.selected_node().unwrap();
    assert_eq!(
        serde_json::to_value(node).unwrap()["props"]["label"],
        "Send"
    );
    assert_eq!(state.history.past.len(), depth + 1, "one undo step");

    // On a master the same field renames the property.
    let panel = select(&mut state, "btn");
    assert_eq!(
        super::property_panel_commit::property_focus_initial(
            PropertyFocus::ComponentProp(1),
            &panel
        ),
        "label"
    );
    state.ui.property_focus = Some(PropertyFocus::ComponentProp(1));
    state.ui.property_input.set_text("caption");
    assert!(super::property_panel_commit::commit_property_focus(
        &mut state
    ));
    let instance = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("i1"));
    assert_eq!(
        serde_json::to_value(instance.unwrap()).unwrap()["props"],
        serde_json::json!({"caption": "Send"})
    );
}

#[test]
fn a_master_lists_rename_fields_and_remove_buttons() {
    let mut state = state_from(DOC);
    let panel = select(&mut state, "btn");
    assert_eq!(
        panel.visible_sections().component_props,
        PropRowsLayout::Master { count: 3 }
    );
    assert_eq!(
        panel.hit_test(PANEL, row_center(&panel, 0, false)),
        Some(PropertyFocus::ComponentProp(0))
    );
    assert_eq!(
        panel.hit_test_action(PANEL, row_center(&panel, 1, true)),
        Some(PropertyPanelAction::RemoveComponentProperty(1))
    );
    let texts = paint_texts(&panel);
    assert!(texts.iter().any(|text| text == "showIcon"), "{texts:?}");
}

#[test]
fn a_node_inside_a_master_offers_new_properties() {
    let mut state = state_from(DOC);
    let panel = select(&mut state, "btn-bg");
    assert_eq!(
        panel.visible_sections().component_props,
        PropRowsLayout::Bind {
            offers: [Some(BindOffer::Fill), Some(BindOffer::Visible), None, None]
        }
    );
    let action = panel.hit_test_action(PANEL, row_center(&panel, 0, false));
    assert_eq!(
        action,
        Some(PropertyPanelAction::BindComponentProperty(BindOffer::Fill))
    );
    let texts = paint_texts(&panel);
    assert!(
        texts.iter().any(|text| text == "Color property"),
        "{texts:?}"
    );
    apply_instance_lifecycle_action(&mut state, &action.unwrap());
    let panel = select(&mut state, "btn");
    assert_eq!(
        panel.visible_sections().component_props,
        PropRowsLayout::Master { count: 4 },
        "Background color joined the master"
    );
}
