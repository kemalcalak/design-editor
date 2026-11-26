//! Variant rows in the property panel (#368): an instance's axis
//! dropdowns above its property rows and buttons, a set's rename / add
//! rows, Combine as variants on selected masters, and their dispatch.

use super::property_panel::{PropertyPanel, PropertyPanelAction};
use super::property_panel_test_support::{state_from, CountingBackend};
use super::property_panel_variants::{self as variants, VariantAction, VariantRowsLayout};
use crate::widgets::property_panel_commit::commit_property_focus;
use crate::widgets::property_panel_dispatch::apply_instance_lifecycle_action;
use crate::widgets::PaintCx;
use crate::widgets::Widget;
use crate::{Point2D, Rect};
use de_editor_core::interaction_edit::InteractionMenuTarget;
use de_editor_core::{EditorState, NodeId, PropertyFocus};

/// Two Button masters (`variant=primary` / `variant=ghost`) with a label
/// property, an instance of the first, all on one page.
const DOC: &str = r##"{
  "version":"1.0.0",
  "children":[
    {"type":"frame","id":"pm","name":"variant=primary, size=md","reusable":true,"width":120,"height":40,
     "componentProperties":{"label":{"type":"text","bind":[{"node":"pm-label","field":"content"}]}},
     "children":[{"type":"text","id":"pm-label","name":"Label","content":"Button"}]},
    {"type":"frame","id":"gm","name":"variant=ghost, size=md","reusable":true,"width":120,"height":40,
     "componentProperties":{"label":{"type":"text","bind":[{"node":"gm-label","field":"content"}]}},
     "children":[{"type":"text","id":"gm-label","name":"Label","content":"Button"}]},
    {"type":"ref","id":"i1","ref":"pm","x":300,"y":0,"props":{"label":"Save"},
     "descendants":{"pm-label":{"fontSize":18}}}
  ]
}"##;

const PANEL: Rect = Rect {
    origin: Point2D { x: 0.0, y: 0.0 },
    size: Point2D {
        x: 280.0,
        y: 1400.0,
    },
};

fn panel_for(state: &EditorState) -> PropertyPanel {
    PropertyPanel::for_selection(state).expect("panel builds")
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
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

/// Combine both masters through the panel and select the instance.
fn combined() -> EditorState {
    let mut state = state_from(DOC);
    state.set_single_selection(NodeId::new("pm"));
    state.toggle_selection(NodeId::new("gm"));
    let panel = panel_for(&state);
    assert_eq!(
        panel.visible_sections().variant_rows,
        VariantRowsLayout::Combine
    );
    let rects = variants::action_rects(
        panel.visible_sections().variant_rows,
        0.0,
        variants::rows_top(
            &panel.visible_sections(),
            super::property_panel_inputs::PANEL_HEADER_HEIGHT,
        ),
        PANEL.size.x,
    );
    let (action, rect) = rects[0].clone();
    assert_eq!(action, PropertyPanelAction::Variant(VariantAction::Combine));
    assert_eq!(
        panel.hit_test_action(PANEL, center(rect)),
        Some(action.clone())
    );
    assert!(paint_texts(&panel)
        .iter()
        .any(|text| text == "Combine as variants"));
    apply_instance_lifecycle_action(&mut state, &action);
    let set_id = state.selection.anchor.as_str().to_string();
    assert!(de_editor_core::component_set::set_summary(&state.doc, &set_id).is_some());
    state.set_single_selection(NodeId::new("i1"));
    state
}

#[test]
fn an_instance_shows_one_dropdown_per_axis_above_its_properties() {
    let state = combined();
    let panel = panel_for(&state);
    let visible = panel.visible_sections();
    assert_eq!(
        visible.variant_rows,
        VariantRowsLayout::Instance {
            axes: 2,
            fallback: false
        }
    );
    let top = variants::rows_top(&visible, super::property_panel_inputs::PANEL_HEADER_HEIGHT);
    let rects = variants::action_rects(visible.variant_rows, 0.0, top, PANEL.size.x);
    assert_eq!(
        rects[0].0,
        PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::VariantAxis(0))
    );
    assert_eq!(
        panel.hit_test_action(PANEL, center(rects[1].1)),
        Some(rects[1].0.clone())
    );
    // The property rows start below the variant rows.
    assert_eq!(
        variants::props_top(&visible, super::property_panel_inputs::PANEL_HEADER_HEIGHT),
        top + visible.variant_rows.height()
    );
    let texts = paint_texts(&panel);
    for expected in ["variant", "primary", "size", "md", "Save"] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected}: {texts:?}"
        );
    }
    // The popover lists the axis's values.
    let rows = variants::axis_menu_rows(&panel.variants, 0);
    assert_eq!(
        rows.iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>(),
        ["primary", "ghost"]
    );
}

#[test]
fn the_axis_dropdown_opens_the_shared_popover_under_its_row() {
    let mut state = combined();
    state.editor_ui.interaction_menu_open = true;
    state.editor_ui.interaction_menu_target = InteractionMenuTarget::VariantAxis(0);
    let panel = panel_for(&state);
    let anchor = panel
        .interaction_menu_anchor_rect(PANEL)
        .expect("the popover hangs off the axis row");
    // The second popover row picks `ghost`.
    let mut hit = None;
    let mut y = anchor.origin.y;
    while y < anchor.origin.y + 200.0 && hit.is_none() {
        if let super::property_panel_interactions::InteractionMenuHit::Row(action) =
            panel.interaction_menu_hit(PANEL, Point2D::new(anchor.origin.x + 20.0, y))
        {
            if action
                != (PropertyPanelAction::Variant(VariantAction::SetInstance { axis: 0, value: 0 }))
            {
                hit = Some(action);
            }
        }
        y += 4.0;
    }
    assert_eq!(
        hit,
        Some(PropertyPanelAction::Variant(VariantAction::SetInstance {
            axis: 0,
            value: 1
        }))
    );
}

#[test]
fn picking_a_value_switches_the_variant_keeping_props_and_overrides() {
    let mut state = combined();
    let panel = panel_for(&state);
    let (action, _) = variants::axis_menu_rows(&panel.variants, 0)[1].clone();
    assert_eq!(
        action,
        PropertyPanelAction::Variant(VariantAction::SetInstance { axis: 0, value: 1 })
    );
    let depth = state.history.past.len();
    apply_instance_lifecycle_action(&mut state, &action);
    assert_eq!(state.history.past.len(), depth + 1, "one undo step");
    let node = serde_json::to_value(state.selected_node().unwrap()).unwrap();
    assert_eq!(node["ref"], "gm");
    assert_eq!(node["props"]["label"], "Save");
    assert_eq!(node["descendants"]["gm-label"]["fontSize"], 18.0);
    assert!(
        state.editor_ui.pending_variant_report.is_none(),
        "nothing lost"
    );
}

#[test]
fn a_set_renames_its_axes_and_adds_variants_and_axes() {
    let mut state = combined();
    let set_id = de_editor_core::component_set::set_containing(&state.doc, "pm")
        .map(|set| de_editor_core::PenNodeExt::id_str(set).to_string())
        .unwrap();
    state.set_single_selection(NodeId::new(set_id.as_str()));
    let panel = panel_for(&state);
    let visible = panel.visible_sections();
    assert_eq!(visible.variant_rows, VariantRowsLayout::Set { axes: 2 });
    let top = variants::rows_top(&visible, super::property_panel_inputs::PANEL_HEADER_HEIGHT);
    let fields = variants::input_rects(visible.variant_rows, 0.0, top, PANEL.size.x);
    assert_eq!(
        panel.hit_test(PANEL, center(fields[1].1)),
        Some(PropertyFocus::VariantText(1))
    );
    // Rename `size` through the field.
    state.ui.property_focus = Some(PropertyFocus::VariantText(1));
    state.ui.property_input.set_text("scale");
    assert!(commit_property_focus(&mut state));
    let summary = de_editor_core::component_set::set_summary(&state.doc, &set_id).unwrap();
    assert_eq!(summary.axes[1].0, "scale");

    let actions: Vec<PropertyPanelAction> =
        variants::action_rects(visible.variant_rows, 0.0, top, PANEL.size.x)
            .into_iter()
            .map(|(action, _)| action)
            .collect();
    assert_eq!(
        actions,
        [
            PropertyPanelAction::Variant(VariantAction::Add),
            PropertyPanelAction::Variant(VariantAction::AddAxis)
        ]
    );
    apply_instance_lifecycle_action(
        &mut state,
        &PropertyPanelAction::Variant(VariantAction::AddAxis),
    );
    state.set_single_selection(NodeId::new(set_id.as_str()));
    apply_instance_lifecycle_action(
        &mut state,
        &PropertyPanelAction::Variant(VariantAction::Add),
    );
    let summary = de_editor_core::component_set::set_summary(&state.doc, &set_id).unwrap();
    assert_eq!(summary.axes.len(), 3);
    assert_eq!(summary.variants.len(), 3);
    assert_eq!(summary.axes[0].1, ["primary", "ghost", "variant3"]);
}
